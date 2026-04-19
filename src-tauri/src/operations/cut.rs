//! Cut: trim a clip to a time range using stream-copy (no re-encode).
//!
//! The `-ss` / `-to` flags are placed before `-i` so FFmpeg seeks quickly.
//! Output will be snapped to the nearest keyframe — this is expected behaviour
//! for a stream-copy cut; document it, don't work around it.

use super::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    start_secs: Option<f64>,
    end_secs: Option<f64>,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // Duration for progress calculation: clamp to [start, end] range
    let file_duration = probe_duration(input_path);
    let duration = match (start_secs, end_secs, file_duration) {
        (Some(s), Some(e), _) => Some(e - s),
        (Some(s), None, Some(d)) => Some(d - s),
        (Some(_), None, None) => None,
        (None, Some(e), _) => Some(e),
        (None, None, d) => d,
    };

    let mut args: Vec<String> = vec!["-y".to_string()];

    // Seek flags before input for fast keyframe seek
    if let Some(s) = start_secs {
        args.extend(["-ss".to_string(), format!("{:.6}", s)]);
    }
    if let Some(e) = end_secs {
        args.extend(["-to".to_string(), format!("{:.6}", e)]);
    }

    args.extend(["-i".to_string(), input_path.to_string()]);
    args.extend([
        "-c".to_string(),
        "copy".to_string(),
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ]);

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
