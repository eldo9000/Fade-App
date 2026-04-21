//! Metadata Strip — remove container-level metadata with `-map_metadata -1`.
//!
//! Two modes:
//!   * `all`   — strip every tag.
//!   * `title` — strip everything, then re-write only the container `title`
//!     tag from a user-supplied string.
//!
//! Stream-copies all tracks so this is effectively a metadata-only rewrap.

use super::run_ffmpeg;
use crate::probe_duration;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    mode: &str,
    title: Option<&str>,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-map_metadata".to_string(),
        "-1".to_string(),
        "-map_chapters".to_string(),
        "-1".to_string(),
    ];

    if mode == "title" {
        let t = title.unwrap_or("").to_string();
        args.push("-metadata".to_string());
        args.push(format!("title={}", t));
    }

    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
