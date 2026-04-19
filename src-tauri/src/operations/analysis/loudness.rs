//! EBU R128 loudness measurement via `loudnorm=print_format=json`.
//!
//! Analyze-only: writes no output file. Parses the JSON block that
//! loudnorm prints to stderr after processing finishes.

use super::run_ffmpeg_capture;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct LoudnessResult {
    #[serde(rename = "I")]
    pub i: String,
    #[serde(rename = "LRA")]
    pub lra: String,
    #[serde(rename = "TP")]
    pub tp: String,
    pub threshold: String,
}

#[tauri::command]
pub fn analyze_loudness(
    input_path: String,
    target_i: f64,
    target_tp: f64,
    true_peak: bool,
) -> Result<LoudnessResult, String> {
    // `loudnorm` prints a JSON summary when print_format=json.
    // loudnorm always measures true-peak with 4x oversampling; the UI toggle
    // is advisory only (kept in the payload for forward compat).
    let _ = true_peak;
    let filter = format!(
        "loudnorm=I={i}:TP={tp}:LRA=11:print_format=json",
        i = target_i,
        tp = target_tp,
    );
    let args = vec![
        "-hide_banner".to_string(),
        "-nostats".to_string(),
        "-i".to_string(),
        input_path,
        "-af".to_string(),
        filter,
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];
    let stderr = run_ffmpeg_capture(&args)?;

    // The JSON block starts at the last '{' and ends at the matching '}'.
    let start = stderr
        .rfind('{')
        .ok_or_else(|| "loudnorm produced no JSON block".to_string())?;
    // Find the matching closing brace by scanning forward.
    let tail = &stderr[start..];
    let end = tail
        .find('}')
        .ok_or_else(|| "loudnorm JSON block not closed".to_string())?;
    // loudnorm outputs a single flat object — rfind('{') + first '}' is safe.
    let json = &tail[..=end];
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("loudnorm JSON parse: {e}"))?;

    Ok(LoudnessResult {
        i: v["input_i"].as_str().unwrap_or("").to_string(),
        lra: v["input_lra"].as_str().unwrap_or("").to_string(),
        tp: v["input_tp"].as_str().unwrap_or("").to_string(),
        threshold: v["input_thresh"].as_str().unwrap_or("").to_string(),
    })
}
