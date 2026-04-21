//! Remove Audio — strip the audio track(s) while stream-copying the video.
//!
//! Uses `-an` to drop audio and `-c copy` so the video track is not
//! re-encoded. Subtitles / metadata / attachments are carried through with
//! `-map 0` plus negative mapping of audio.

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
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-map".to_string(),
        "-0:a".to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
