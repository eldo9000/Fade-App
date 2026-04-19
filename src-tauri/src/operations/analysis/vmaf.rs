//! VMAF — Netflix perceptual quality score.
//!
//! Uses `libvmaf` with a JSON log. Both inputs are forced to the model's
//! native resolution so the filter graph is always valid; fps is re-sampled
//! on the distorted input to match the reference.

use super::run_ffmpeg_capture;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct VmafResult {
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub harmonic_mean: f64,
}

#[tauri::command]
pub fn analyze_vmaf(
    reference_path: String,
    distorted_path: String,
    model: String, // "hd" | "4k" | "phone"
    subsample: u32,
) -> Result<VmafResult, String> {
    let (res, model_name) = match model.as_str() {
        "4k" => ("3840x2160", "vmaf_4k_v0.6.1"),
        "phone" => ("1920x1080", "vmaf_v0.6.1"),
        _ => ("1920x1080", "vmaf_v0.6.1"),
    };
    let (w, h) = res.split_once('x').unwrap_or(("1920", "1080"));

    let log_path = std::env::temp_dir()
        .join(format!("fade_vmaf_{}.json", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();

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
        log = log_path.replace(':', "\\:"),
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
    run_ffmpeg_capture(&args)?;

    let body = std::fs::read_to_string(&log_path).map_err(|e| format!("read vmaf log: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("vmaf json parse: {e}"))?;

    // libvmaf v2+ shape: { "pooled_metrics": { "vmaf": { "mean": ..., ... } } }
    let pooled = &v["pooled_metrics"]["vmaf"];
    let out = VmafResult {
        mean: pooled["mean"].as_f64().unwrap_or(0.0),
        min: pooled["min"].as_f64().unwrap_or(0.0),
        max: pooled["max"].as_f64().unwrap_or(0.0),
        harmonic_mean: pooled["harmonic_mean"].as_f64().unwrap_or(0.0),
    };
    let _ = std::fs::remove_file(&log_path);
    Ok(out)
}
