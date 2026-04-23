//! Black-interval detection via the `blackdetect` filter.
//! Parses stderr lines of the form:
//!   [blackdetect @ 0x...] black_start:3.00 black_end:5.04 black_duration:2.04

use super::run_ffmpeg_capture_registered;
use crate::AppState;
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct BlackInterval {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
}

#[derive(Serialize, Clone)]
pub struct BlackDetectResult {
    pub job_id: String,
    pub intervals: Option<Vec<BlackInterval>>,
    pub error: Option<String>,
    pub cancelled: bool,
}

fn parse_intervals(stderr: &str) -> Vec<BlackInterval> {
    let mut out = Vec::new();
    for line in stderr.lines() {
        if !line.contains("blackdetect") || !line.contains("black_start") {
            continue;
        }
        let parse = |key: &str| -> Option<f64> {
            let idx = line.find(key)? + key.len();
            line[idx..]
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok())
        };
        if let (Some(s), Some(e), Some(d)) = (
            parse("black_start:"),
            parse("black_end:"),
            parse("black_duration:"),
        ) {
            out.push(BlackInterval {
                start: s,
                end: e,
                duration: d,
            });
        }
    }
    out
}

#[tauri::command]
pub fn analyze_black_detect(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    input_path: String,
    min_duration: f64, // d=
    pix_th: f64,       // pix_th=
    pic_th: f64,       // pic_th=
) -> Result<(), String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let filter = format!(
            "blackdetect=d={:.3}:pix_th={:.3}:pic_th={:.3}",
            min_duration, pix_th, pic_th,
        );
        let args = vec![
            "-hide_banner".to_string(),
            "-nostats".to_string(),
            "-i".to_string(),
            input_path,
            "-vf".to_string(),
            filter,
            "-an".to_string(),
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
            Ok(stderr) => BlackDetectResult {
                job_id: job_id.clone(),
                intervals: Some(parse_intervals(&stderr)),
                error: None,
                cancelled: false,
            },
            Err(msg) if msg == "CANCELLED" => BlackDetectResult {
                job_id: job_id.clone(),
                intervals: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => BlackDetectResult {
                job_id: job_id.clone(),
                intervals: None,
                error: Some(msg),
                cancelled: false,
            },
        };

        let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
    });

    Ok(())
}
