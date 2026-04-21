//! Rewrap: change container without re-encoding.
//!
//! Pre-flight: check that all streams in the input are compatible with the
//! target container.  If any are incompatible, return an error before touching
//! anything on disk.

use super::{check_codec_container_compat, duration_from_probe, ext_of, parse_streams, run_ffmpeg, run_ffprobe};
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::Window;

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // Pre-flight: check codec/container compatibility
    let probe = run_ffprobe(input_path)?;
    let streams = parse_streams(&probe);
    let out_ext = ext_of(output_path);

    for stream in &streams {
        if let Some(err) =
            check_codec_container_compat(&stream.codec, &stream.stream_type, &out_ext)
        {
            return Err(err);
        }
    }

    let duration = duration_from_probe(&probe);

    // -c copy remuxes all streams; -map 0 keeps every stream
    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-c".to_string(),
        "copy".to_string(),
        "-map".to_string(),
        "0".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
