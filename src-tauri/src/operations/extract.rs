//! Extract: pull individual streams out of a container without re-encoding.

use super::{parse_streams, run_ffmpeg, run_ffprobe, StreamInfo};
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

/// Return all streams found in `input_path`.
pub fn get_streams(input_path: &str) -> Result<Vec<StreamInfo>, String> {
    let probe = run_ffprobe(input_path)?;
    Ok(parse_streams(&probe))
}

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    stream_index: u32,
    stream_type: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    // Map selector: e.g. 0:v:0, 0:a:1, 0:s:0
    // stream_index is the absolute stream index in the file.
    let map_sel = format!("0:{}", stream_index);

    // Codec flags — copy all; FFmpeg will infer the right codec for the
    // output container from the stream type.
    let codec_flag = match stream_type {
        "video" => vec!["-c:v".to_string(), "copy".to_string()],
        "audio" => vec!["-c:a".to_string(), "copy".to_string()],
        "subtitle" => vec!["-c:s".to_string(), "copy".to_string()],
        _ => vec!["-c".to_string(), "copy".to_string()],
    };

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-map".to_string(),
        map_sel,
    ];
    args.extend(codec_flag);
    args.extend(["-progress".to_string(), "pipe:1".to_string()]);
    args.push(output_path.to_string());

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
