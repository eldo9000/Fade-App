//! Video-processing filter ops that re-encode the video track.
//!
//! All ops here build a single `-vf` / `-af` graph and call `run_ffmpeg`.
//! H.264 + AAC is the default re-encode target because the Fade UI writes
//! MP4 by default. Callers may pick a different extension on the output
//! path and ffmpeg will muxer-infer, but the codecs stay h264/aac.

use super::run_ffmpeg;
use crate::probe_duration;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

/// Common output encoder args for filter ops (H.264 + AAC).
fn enc_args() -> Vec<String> {
    vec![
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "18".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
    ]
}

// ── rotate / flip ─────────────────────────────────────────────────────────

pub fn run_rotate_flip(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    mode: &str, // cw90 · ccw90 · 180 · hflip · vflip
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let vf = match mode {
        "cw90" => "transpose=1",
        "ccw90" => "transpose=2",
        "180" => "transpose=1,transpose=1",
        "hflip" => "hflip",
        "vflip" => "vflip",
        _ => return Err(format!("unknown rotate mode: {mode}")),
    };

    let duration = probe_duration(input_path);
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        vf.to_string(),
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── reverse ───────────────────────────────────────────────────────────────

pub fn run_reverse(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        "reverse".to_string(),
        "-af".to_string(),
        "areverse".to_string(),
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── speed ─────────────────────────────────────────────────────────────────

/// Build an `atempo` chain that covers any ratio by splitting the factor
/// into 0.5..=2.0 chunks (atempo's hard limits).
fn atempo_chain(rate: f64) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut r = rate;
    while r > 2.0 {
        parts.push("atempo=2.0".to_string());
        r /= 2.0;
    }
    while r < 0.5 {
        parts.push("atempo=0.5".to_string());
        r /= 0.5;
    }
    parts.push(format!("atempo={:.4}", r));
    parts.join(",")
}

pub fn run_speed(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    rate: f64,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let rate = rate.clamp(0.1, 10.0);
    let vf = format!("setpts=PTS/{:.6}", rate);
    let af = atempo_chain(rate);

    let duration = probe_duration(input_path).map(|d| d / rate);

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        vf,
        "-af".to_string(),
        af,
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── fade ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_fade(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    fade_in: f64,
    fade_out: f64,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration =
        probe_duration(input_path).ok_or_else(|| "couldn't probe duration".to_string())?;
    let fi = fade_in.clamp(0.0, 10.0);
    let fo = fade_out.clamp(0.0, 10.0);
    let out_start = (duration - fo).max(0.0);

    let mut v_parts: Vec<String> = Vec::new();
    let mut a_parts: Vec<String> = Vec::new();
    if fi > 0.0 {
        v_parts.push(format!("fade=t=in:st=0:d={:.3}", fi));
        a_parts.push(format!("afade=t=in:st=0:d={:.3}", fi));
    }
    if fo > 0.0 {
        v_parts.push(format!("fade=t=out:st={:.3}:d={:.3}", out_start, fo));
        a_parts.push(format!("afade=t=out:st={:.3}:d={:.3}", out_start, fo));
    }
    if v_parts.is_empty() {
        return Err("No fade set (both in and out are 0)".to_string());
    }

    let vf = v_parts.join(",");
    let af = a_parts.join(",");

    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        vf,
        "-af".to_string(),
        af,
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, Some(duration), processes, cancelled)
}

// ── deinterlace ───────────────────────────────────────────────────────────

pub fn run_deinterlace(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    mode: &str, // yadif · yadif_double · bwdif
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let vf = match mode {
        "yadif" => "yadif=0:-1:0",
        "yadif_double" => "yadif=1:-1:0",
        "bwdif" => "bwdif=0:-1:0",
        _ => return Err(format!("unknown deinterlace mode: {mode}")),
    };

    let duration = probe_duration(input_path);
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        vf.to_string(),
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── denoise ───────────────────────────────────────────────────────────────

pub fn run_denoise(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    preset: &str, // light · medium · strong
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // hqdn3d=luma_spatial:chroma_spatial:luma_tmp:chroma_tmp
    let vf = match preset {
        "light" => "hqdn3d=2:1:2:3",
        "medium" => "hqdn3d=4:3:6:4.5",
        "strong" => "hqdn3d=7:5:9:6",
        _ => return Err(format!("unknown denoise preset: {preset}")),
    };

    let duration = probe_duration(input_path);
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        vf.to_string(),
    ];
    args.extend(enc_args());
    args.push("-progress".to_string());
    args.push("pipe:1".to_string());
    args.push(output_path.to_string());
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
