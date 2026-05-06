//! EBU R128 loudness measurement via `loudnorm=print_format=json`.
//!
//! Analyze-only: writes no output file. Parses the JSON block that
//! loudnorm prints to stderr after processing finishes.

use super::run_ffmpeg_capture_registered;
use crate::{validate_input_path, AppState};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, State, Window};

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

#[derive(Serialize, Clone)]
pub struct LoudnessJobResult {
    pub job_id: String,
    pub data: Option<LoudnessResult>,
    pub error: Option<String>,
    pub cancelled: bool,
}

fn parse_loudness(stderr: &str) -> Result<LoudnessResult, String> {
    let start = stderr
        .rfind('{')
        .ok_or_else(|| "loudnorm produced no JSON block".to_string())?;
    let tail = &stderr[start..];
    let end = tail
        .find('}')
        .ok_or_else(|| "loudnorm JSON block not closed".to_string())?;
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

#[tauri::command]
pub fn analyze_loudness(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    input_path: String,
    target_i: f64,
    target_tp: f64,
    true_peak: bool,
) -> Result<(), String> {
    validate_input_path(&input_path)?;
    // `loudnorm` prints a JSON summary when print_format=json.
    // loudnorm always measures true-peak with 4x oversampling; the UI toggle
    // is advisory only (kept in the payload for forward compat).
    let _ = true_peak;

    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
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

        let result = run_ffmpeg_capture_registered(
            &args,
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let payload = match result {
            Ok(stderr) => match parse_loudness(&stderr) {
                Ok(data) => LoudnessJobResult {
                    job_id: job_id.clone(),
                    data: Some(data),
                    error: None,
                    cancelled: false,
                },
                Err(msg) => LoudnessJobResult {
                    job_id: job_id.clone(),
                    data: None,
                    error: Some(msg),
                    cancelled: false,
                },
            },
            Err(msg) if msg == "CANCELLED" => LoudnessJobResult {
                job_id: job_id.clone(),
                data: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => LoudnessJobResult {
                job_id: job_id.clone(),
                data: None,
                error: Some(msg),
                cancelled: false,
            },
        };

        let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
    });

    Ok(())
}
