//! Audio-only filter ops. Video stream is copied where present.
//!
//! * `run_volume` — `volume=NdB` gain
//! * `run_channel_tools` — `pan=` mix matrix for stereo/mono tricks
//! * `run_pad_silence` — `adelay` head + `apad=pad_dur` tail

use super::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

fn audio_only_args(input_path: &str, af: &str, output_path: &str) -> Vec<String> {
    vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-af".to_string(),
        af.to_string(),
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
}

// ── volume ────────────────────────────────────────────────────────────────

pub fn run_volume(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    gain_db: f64,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let db = gain_db.clamp(-30.0, 20.0);
    let af = format!("volume={:.2}dB", db);
    let duration = probe_duration(input_path);
    let args = audio_only_args(input_path, &af, output_path);
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── channel tools ─────────────────────────────────────────────────────────

pub fn run_channel_tools(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    mode: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let af: &str = match mode {
        "stereo_to_mono" => "pan=mono|c0=0.5*c0+0.5*c1",
        "swap" => "pan=stereo|c0=c1|c1=c0",
        "mute_l" => "pan=stereo|c0=0*c0|c1=c1",
        "mute_r" => "pan=stereo|c0=c0|c1=0*c1",
        "mono_to_stereo" => "pan=stereo|c0=c0|c1=c0",
        _ => return Err(format!("unknown channel-tools mode: {mode}")),
    };
    let duration = probe_duration(input_path);
    let args = audio_only_args(input_path, af, output_path);
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── pad silence ───────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_pad_silence(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    head_s: f64,
    tail_s: f64,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let head = head_s.clamp(0.0, 60.0);
    let tail = tail_s.clamp(0.0, 60.0);

    // adelay expects milliseconds per channel (|-separated). We use `all=1`
    // to cover any channel count in one go. apad uses `pad_dur` in seconds.
    let mut parts: Vec<String> = Vec::new();
    if head > 0.0 {
        let ms = (head * 1000.0) as u64;
        parts.push(format!("adelay={ms}:all=1"));
    }
    if tail > 0.0 {
        parts.push(format!("apad=pad_dur={:.3}", tail));
    }
    if parts.is_empty() {
        return Err("Both head and tail are 0".to_string());
    }
    let af = parts.join(",");
    let duration = probe_duration(input_path).map(|d| d + head + tail);

    let args = audio_only_args(input_path, &af, output_path);
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
