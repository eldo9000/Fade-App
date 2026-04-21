//! Replace Audio: swap the audio track in a video with a new audio file.
//!
//! If the new audio codec is incompatible with the output container (e.g. PCM
//! WAV into MP4), the audio is re-encoded to AAC 192k.  Otherwise stream-copy
//! is used for both video and audio.  Output container matches the video input.

use super::{ext_of, parse_streams, run_ffmpeg, run_ffprobe};
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::Window;

pub fn run(
    window: &Window,
    job_id: &str,
    video_path: &str,
    audio_path: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(video_path);
    let out_ext = ext_of(output_path);

    // Probe the audio file to decide whether to re-encode
    let audio_probe = run_ffprobe(audio_path)?;
    let audio_streams = parse_streams(&audio_probe);
    let audio_codec = audio_streams
        .iter()
        .find(|s| s.stream_type == "audio")
        .map(|s| s.codec.as_str())
        .unwrap_or("aac");

    // PCM codecs are not valid in MP4/M4V/MOV containers
    let needs_transcode =
        matches!(out_ext.as_str(), "mp4" | "m4v" | "mov") && audio_codec.starts_with("pcm_");

    let audio_codec_args: Vec<String> = if needs_transcode {
        vec![
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
        ]
    } else {
        vec!["-c:a".to_string(), "copy".to_string()]
    };

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        video_path.to_string(),
        "-i".to_string(),
        audio_path.to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
    ];
    args.extend(audio_codec_args);
    args.extend([
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "1:a".to_string(),
        "-shortest".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ]);

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
