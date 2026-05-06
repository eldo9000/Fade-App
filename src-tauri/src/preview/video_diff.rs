use crate::args::{ffmpeg_video_codec_args, resolution_to_scale};
use crate::operations::analysis::run_ffmpeg_capture_registered;
use crate::{truncate_stderr, validate_input_path, AppState};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{command, Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct DiffPreview {
    path: String,
    note: String,
}

#[derive(Serialize, Clone)]
pub struct DiffPreviewResult {
    pub job_id: String,
    pub data: Option<DiffPreview>,
    pub error: Option<String>,
    pub cancelled: bool,
}

/// Encode a short snippet of the source with the requested codec/resolution and
/// return a clip showing the per-pixel difference between the original and the
/// re-encoded version (amplified for visibility). The snippet is padded with
/// `handle_secs` of runway on each side so the rate-controller and GOP layout
/// reach steady state before the target region is compared.
#[allow(clippy::too_many_arguments)]
#[command]
pub fn preview_diff(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    path: String,
    codec: String,
    resolution: Option<String>,
    at_secs: f64,
    duration_secs: Option<f64>,
    handle_secs: Option<f64>,
    amplify: Option<f64>,
) -> Result<(), String> {
    validate_input_path(&path)?;
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }

    // Register cancellation flag before spawning the thread.
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let dur = duration_secs.unwrap_or(1.0).clamp(0.1, 10.0);
        let handle = handle_secs.unwrap_or(3.0).clamp(0.0, 10.0);
        let amp = amplify.unwrap_or(8.0).clamp(1.0, 32.0);

        let start = (at_secs - handle).max(0.0);
        // How far into the encoded snippet the target region begins.
        let mid_offset = at_secs - start;
        let total = dur + 2.0 * handle;

        #[cfg(unix)]
        let make_sandbox = || -> Result<tempfile::TempDir, String> {
            use std::os::unix::fs::PermissionsExt;
            tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(std::env::temp_dir())
                .map_err(|e| format!("failed to create temp sandbox: {e}"))
        };
        #[cfg(not(unix))]
        let make_sandbox = || -> Result<tempfile::TempDir, String> {
            tempfile::TempDir::new_in(std::env::temp_dir())
                .map_err(|e| format!("failed to create temp sandbox: {e}"))
        };

        let enc_sandbox = match make_sandbox() {
            Ok(d) => d,
            Err(e) => {
                let payload = DiffPreviewResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(e),
                    cancelled: false,
                };
                let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
                return;
            }
        };
        let encoded = enc_sandbox.path().join("encoded.mp4");
        let diff_sandbox = match make_sandbox() {
            Ok(d) => d,
            Err(e) => {
                let payload = DiffPreviewResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(e),
                    cancelled: false,
                };
                let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
                return;
            }
        };
        let diff = diff_sandbox.path().join("diff.mp4");

        // Optional spatial scaling — must be applied to BOTH inputs during the diff
        // pass so resolutions match for blend.
        let scale_filter = match resolution.as_deref() {
            Some(r) if r != "original" && !r.is_empty() => Some(resolution_to_scale(r)),
            _ => None,
        };

        // ── Pass 1: encode snippet with handles ──
        let mut enc_args: Vec<String> = vec![
            "-y".to_string(),
            "-ss".to_string(),
            format!("{start:.3}"),
            "-i".to_string(),
            path.clone(),
            "-t".to_string(),
            format!("{total:.3}"),
            "-an".to_string(),
        ];
        if let Some(sf) = scale_filter.as_deref() {
            enc_args.push("-vf".to_string());
            enc_args.push(sf.to_string());
        }
        enc_args.extend(ffmpeg_video_codec_args(&codec));
        enc_args.push(encoded.to_string_lossy().to_string());

        let enc_result = run_ffmpeg_capture_registered(
            &enc_args,
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        // If encode failed or was cancelled, emit and exit early.
        let encode_err = match enc_result {
            Ok(_) => None,
            Err(msg) if msg == "CANCELLED" => {
                let _ = std::fs::remove_file(&encoded);
                {
                    let mut map = cancellations.lock();
                    map.remove(&job_id);
                }
                let payload = DiffPreviewResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: None,
                    cancelled: true,
                };
                let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
                return;
            }
            Err(msg) => Some(format!("encode failed: {}", truncate_stderr(&msg))),
        };

        if let Some(msg) = encode_err {
            let _ = std::fs::remove_file(&encoded);
            {
                let mut map = cancellations.lock();
                map.remove(&job_id);
            }
            let payload = DiffPreviewResult {
                job_id: job_id.clone(),
                data: None,
                error: Some(msg),
                cancelled: false,
            };
            let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
            return;
        }

        // ── Pass 2: blend=difference, amplified, grayscale output ──
        //
        // Alignment is the critical detail here. `-ss` before `-i` snaps to the
        // nearest keyframe, which for a short encoded snippet (one GOP) is always
        // frame 0 — so the two streams end up several seconds out of phase and the
        // blend produces black.
        //
        // Fix: use `trim=start=` filters (which operate on decoded PTS, not seeks)
        // so both inputs are cut to the exact same temporal window before blending.
        //
        //   Input 0 (source): seek close with -ss for efficiency; trim=start uses
        //     the absolute source PTS to land on exactly at_secs.
        //   Input 1 (encoded snippet): no seek at all; trim=start={mid_offset}
        //     advances through the decoded clip to the target region.
        let pre_seek = (at_secs - 5.0).max(0.0);
        let src_trim = format!("trim=start={at_secs:.3}:duration={dur:.3},setpts=PTS-STARTPTS");
        let enc_trim = format!("trim=start={mid_offset:.3}:duration={dur:.3},setpts=PTS-STARTPTS");

        let src_chain = match scale_filter.as_deref() {
            Some(sf) => format!("[0:v]{src_trim},{sf},format=yuv420p[a]"),
            None => format!("[0:v]{src_trim},format=yuv420p[a]"),
        };
        let filter = format!(
            "{src_chain};[1:v]{enc_trim},format=yuv420p[b];[a][b]blend=all_mode=difference,lutyuv=y=val*{amp}:u=128:v=128[o]"
        );

        let diff_args: Vec<String> = vec![
            "-y".to_string(),
            "-ss".to_string(),
            format!("{pre_seek:.3}"),
            "-i".to_string(),
            path.clone(),
            "-i".to_string(),
            encoded.to_string_lossy().to_string(),
            "-filter_complex".to_string(),
            filter,
            "-map".to_string(),
            "[o]".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "ultrafast".to_string(),
            "-crf".to_string(),
            "16".to_string(),
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            "-an".to_string(),
            diff.to_string_lossy().to_string(),
        ];

        let diff_result = run_ffmpeg_capture_registered(
            &diff_args,
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        // Intermediate encoded file no longer needed; enc_sandbox drops here via RAII.
        let _ = std::fs::remove_file(&encoded);
        drop(enc_sandbox);

        // Keep diff sandbox alive for the renderer; caller is responsible for cleanup.
        let kept_diff_dir = diff_sandbox.keep();
        let kept_diff = kept_diff_dir.join("diff.mp4");

        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let payload = match diff_result {
            Ok(_) => DiffPreviewResult {
                job_id: job_id.clone(),
                data: Some(DiffPreview {
                    path: kept_diff.to_string_lossy().to_string(),
                    note: format!("codec={codec} handles={handle:.1}s amp={amp:.0}×"),
                }),
                error: None,
                cancelled: false,
            },
            Err(msg) if msg == "CANCELLED" => DiffPreviewResult {
                job_id: job_id.clone(),
                data: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => DiffPreviewResult {
                job_id: job_id.clone(),
                data: None,
                error: Some(format!("diff failed: {}", truncate_stderr(&msg))),
                cancelled: false,
            },
        };

        let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_preview_serializes() {
        let d = DiffPreview {
            path: "/tmp/out.mp4".to_string(),
            note: "codec=h264 handles=3.0s amp=8×".to_string(),
        };
        let v: serde_json::Value = serde_json::to_value(&d).unwrap();
        assert_eq!(v["path"], "/tmp/out.mp4");
        assert!(v["note"].as_str().unwrap().contains("codec=h264"));
    }
}
