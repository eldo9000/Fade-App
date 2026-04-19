//! Remove Video — drop the video track(s), keep audio (and subs) stream-copied.
//!
//! `-vn` drops video; `-c copy` preserves the audio codec. Useful for
//! turning a video file into an audio-only container (m4a / mka) without
//! re-encoding.

use super::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vn".to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
