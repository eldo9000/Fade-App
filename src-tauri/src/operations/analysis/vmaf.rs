//! VMAF — Netflix perceptual quality score.
//!
//! Uses `libvmaf` with a JSON log. Both inputs are forced to the model's
//! native resolution so the filter graph is always valid; fps is re-sampled
//! on the distorted input to match the reference.

use super::run_ffmpeg_capture_registered;
use crate::{validate_input_path, AppState};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct VmafScores {
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub harmonic_mean: f64,
}

#[derive(Serialize, Clone)]
pub struct VmafResult {
    pub job_id: String,
    pub data: Option<VmafScores>,
    pub error: Option<String>,
    pub cancelled: bool,
}

fn parse_vmaf_log(log_path: &str) -> Result<VmafScores, String> {
    let body = std::fs::read_to_string(log_path).map_err(|e| format!("read vmaf log: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("vmaf json parse: {e}"))?;

    // libvmaf v2+ shape: { "pooled_metrics": { "vmaf": { "mean": ..., ... } } }
    let pooled = &v["pooled_metrics"]["vmaf"];
    Ok(VmafScores {
        mean: pooled["mean"].as_f64().unwrap_or(0.0),
        min: pooled["min"].as_f64().unwrap_or(0.0),
        max: pooled["max"].as_f64().unwrap_or(0.0),
        harmonic_mean: pooled["harmonic_mean"].as_f64().unwrap_or(0.0),
    })
}

#[tauri::command]
pub fn analyze_vmaf(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    reference_path: String,
    distorted_path: String,
    model: String, // "hd" | "4k" | "phone"
    subsample: u32,
) -> Result<(), String> {
    validate_input_path(&reference_path)?;
    validate_input_path(&distorted_path)?;
    // Register cancellation flag before spawning the thread.
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let (res, model_name) = match model.as_str() {
            "4k" => ("3840x2160", "vmaf_4k_v0.6.1"),
            "phone" => ("1920x1080", "vmaf_v0.6.1"),
            _ => ("1920x1080", "vmaf_v0.6.1"),
        };
        let (w, h) = res.split_once('x').unwrap_or(("1920", "1080"));

        #[cfg(unix)]
        let sandbox = {
            use std::os::unix::fs::PermissionsExt;
            match tempfile::Builder::new()
                .permissions(std::fs::Permissions::from_mode(0o700))
                .tempdir_in(std::env::temp_dir())
            {
                Ok(d) => d,
                Err(e) => {
                    let payload = VmafResult {
                        job_id: job_id.clone(),
                        data: None,
                        error: Some(format!("failed to create temp sandbox: {e}")),
                        cancelled: false,
                    };
                    let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
                    return;
                }
            }
        };
        #[cfg(not(unix))]
        let sandbox = match tempfile::TempDir::new_in(std::env::temp_dir()) {
            Ok(d) => d,
            Err(e) => {
                let payload = VmafResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(format!("failed to create temp sandbox: {e}")),
                    cancelled: false,
                };
                let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
                return;
            }
        };
        let log_path = sandbox.path().join("vmaf.json");
        let log_path_str = log_path.to_string_lossy().to_string();

        // Scale both inputs to the model's native size; fps-match distorted to ref.
        // The `phone` flag is a libvmaf toggle, not a model — map accordingly.
        let phone_flag = if model == "phone" {
            ":phone_model=1"
        } else {
            ""
        };

        let filter = format!(
            "[0:v]scale={w}:{h}:flags=bicubic,setpts=PTS-STARTPTS[ref];\
             [1:v]scale={w}:{h}:flags=bicubic,setpts=PTS-STARTPTS[dist];\
             [dist][ref]libvmaf=model=version={model_name}:log_fmt=json:log_path={log}:n_subsample={ns}{phone}",
            w = w,
            h = h,
            model_name = model_name,
            log = log_path_str.replace(':', "\\:"),
            ns = subsample.max(1),
            phone = phone_flag,
        );

        let args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            reference_path,
            "-i".to_string(),
            distorted_path,
            "-lavfi".to_string(),
            filter,
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ];

        let result = run_ffmpeg_capture_registered(
            &args,
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        // Clean up cancellation registry entry.
        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let payload = match result {
            Ok(_) => match parse_vmaf_log(&log_path_str) {
                Ok(scores) => VmafResult {
                    job_id: job_id.clone(),
                    data: Some(scores),
                    error: None,
                    cancelled: false,
                },
                Err(msg) => VmafResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(msg),
                    cancelled: false,
                },
            },
            Err(msg) if msg == "CANCELLED" => VmafResult {
                job_id: job_id.clone(),
                data: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => VmafResult {
                job_id: job_id.clone(),
                data: None,
                error: Some(msg),
                cancelled: false,
            },
        };

        // sandbox drops here → auto-cleanup (removes vmaf.json and sandbox dir)
        drop(sandbox);
        let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
    });

    Ok(())
}
