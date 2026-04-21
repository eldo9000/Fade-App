use crate::args::{ffmpeg_video_codec_args, resolution_to_scale};
use crate::truncate_stderr;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[derive(Serialize, Clone)]
pub struct DiffPreview {
    path: String,
    note: String,
}

/// Encode a short snippet of the source with the requested codec/resolution and
/// return a clip showing the per-pixel difference between the original and the
/// re-encoded version (amplified for visibility). The snippet is padded with
/// `handle_secs` of runway on each side so the rate-controller and GOP layout
/// reach steady state before the target region is compared.
#[allow(clippy::too_many_arguments)]
#[command]
pub fn preview_diff(
    path: String,
    codec: String,
    resolution: Option<String>,
    at_secs: f64,
    duration_secs: Option<f64>,
    handle_secs: Option<f64>,
    amplify: Option<f64>,
) -> Result<DiffPreview, String> {
    crate::validate_no_traversal(&path)?;
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }

    let dur = duration_secs.unwrap_or(1.0).clamp(0.1, 10.0);
    let handle = handle_secs.unwrap_or(3.0).clamp(0.0, 10.0);
    let amp = amplify.unwrap_or(8.0).clamp(1.0, 32.0);

    let start = (at_secs - handle).max(0.0);
    // How far into the encoded snippet the target region begins.
    let mid_offset = at_secs - start;
    let total = dur + 2.0 * handle;

    let tmp_dir = std::env::temp_dir();
    let job_id = uuid::Uuid::new_v4().to_string();
    let encoded = tmp_dir.join(format!("fade-diff-enc-{job_id}.mp4"));
    let diff = tmp_dir.join(format!("fade-diff-out-{job_id}.mp4"));

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

    let enc_out = Command::new("ffmpeg")
        .args(&enc_args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;
    if !enc_out.status.success() {
        return Err(format!(
            "encode failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&enc_out.stderr))
        ));
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

    let diff_out = Command::new("ffmpeg")
        .args(&diff_args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    // Intermediate encoded file no longer needed.
    let _ = std::fs::remove_file(&encoded);

    if !diff_out.status.success() {
        return Err(format!(
            "diff failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&diff_out.stderr))
        ));
    }

    Ok(DiffPreview {
        path: diff.to_string_lossy().to_string(),
        note: format!("codec={codec} handles={handle:.1}s amp={amp:.0}×"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_when_file_missing() {
        let missing = std::env::temp_dir().join(format!(
            "fade-diff-missing-{}-{}.mp4",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let res = preview_diff(
            missing.to_string_lossy().to_string(),
            "h264".to_string(),
            None,
            5.0,
            Some(1.0),
            Some(3.0),
            Some(8.0),
        );
        let err = match res {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        };
        assert!(err.starts_with("File not found"), "got: {err}");
    }

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
