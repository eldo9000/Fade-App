//! Loop — concatenate the input against itself N times via `-stream_loop`.
//!
//! `-stream_loop N` replays the input N+1 times (N additional loops after
//! the first pass). We interpret the UI's `count` as the total number of
//! playthroughs (2..=50) and pass `count - 1` to ffmpeg. Streams are copied
//! end-to-end — no re-encode.

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
    count: u32,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let count = count.clamp(2, 50);
    let extra = count - 1;

    // Scale the reported duration by total loops so the progress bar fills
    // once over the final output rather than once per loop.
    let duration = probe_duration(input_path).map(|d| d * count as f64);

    let args = vec![
        "-y".to_string(),
        "-stream_loop".to_string(),
        extra.to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
