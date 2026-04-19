//! Audio Offset: shift the audio track relative to the video.
//!
//! Positive offset_ms — delay audio (audio plays later than video).
//!   Uses `-itsoffset` on the second input to delay the audio source.
//!
//! Negative offset_ms — advance audio (audio plays earlier).
//!   Trims the beginning of the audio track with `atrim` + `asetpts`;
//!   audio is re-encoded to AAC because atrim requires a decode/encode pass.

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
    offset_ms: i64,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    let args = if offset_ms >= 0 {
        // Positive: delay audio — second input read with -itsoffset
        let offset_secs = offset_ms as f64 / 1000.0;
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input_path.to_string(),
            "-itsoffset".to_string(),
            format!("{:.6}", offset_secs),
            "-i".to_string(),
            input_path.to_string(),
            "-map".to_string(),
            "0:v".to_string(),
            "-map".to_string(),
            "1:a".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
            "-progress".to_string(),
            "pipe:1".to_string(),
            output_path.to_string(),
        ]
    } else {
        // Negative: advance audio — trim audio start
        let abs_secs = (-offset_ms) as f64 / 1000.0;
        let filter = format!(
            "[0:a]atrim=start={:.6},asetpts=PTS-STARTPTS[aout]",
            abs_secs
        );
        vec![
            "-y".to_string(),
            "-i".to_string(),
            input_path.to_string(),
            "-filter_complex".to_string(),
            filter,
            "-map".to_string(),
            "0:v".to_string(),
            "-map".to_string(),
            "[aout]".to_string(),
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
            "-progress".to_string(),
            "pipe:1".to_string(),
            output_path.to_string(),
        ]
    };

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
