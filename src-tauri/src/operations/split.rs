//! Split: divide a file at one or more timecodes, producing N+1 output files.
//!
//! Uses FFmpeg's segment muxer for stream-copy splitting.  Output files are
//! named `{stem}_part000.ext`, `{stem}_part001.ext`, … in the same directory
//! as the input file (or `output_dir` if provided).

use super::run_ffmpeg;
use crate::probe_duration;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    timecodes_secs: &[f64],
    output_dir: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if timecodes_secs.is_empty() {
        return Err("At least one timecode is required to split".to_string());
    }

    let duration = probe_duration(input_path);

    // Build the output pattern: {output_dir}/{stem}_part%03d.{ext}
    let ext = std::path::Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "mp4".into());
    let stem = std::path::Path::new(input_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    let pattern = format!("{}/{}_part%03d.{}", output_dir, stem, ext);

    // Segment times as comma-separated list of seconds
    let segment_times = timecodes_secs
        .iter()
        .map(|t| format!("{:.6}", t))
        .collect::<Vec<_>>()
        .join(",");

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-f".to_string(),
        "segment".to_string(),
        "-segment_times".to_string(),
        segment_times,
        "-segment_time_delta".to_string(),
        "0.05".to_string(),
        "-reset_timestamps".to_string(),
        "1".to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        pattern,
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
