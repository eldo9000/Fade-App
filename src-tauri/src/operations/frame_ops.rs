//! Single-frame / image-sequence / watermark ops.
//!
//! * `run_thumbnail` — seek to a timestamp and write one image.
//! * `run_contact_sheet` — select N frames and tile them into one PNG.
//! * `run_frame_export` — emit an image sequence to a sibling `<name>_frames/`
//!   folder, either at a fixed fps or a fixed interval.
//! * `run_watermark` — overlay a PNG in a corner with opacity + % width scale.
//!
//! Progress for the one-shot thumbnail and contact sheet is not emitted via
//! `-progress` because those jobs finish almost instantly; the call still
//! goes through `run_ffmpeg` so it shares cancellation & the process map.

use super::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

fn image_codec_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "png",
        "webp" => "libwebp",
        _ => "mjpeg",
    }
}

// ── thumbnail ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_thumbnail(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    time_spec: &str, // HH:MM:SS[.ms] or seconds
    format: &str,    // jpeg · png · webp
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let codec = image_codec_for_ext(format);
    let args = vec![
        "-y".to_string(),
        "-ss".to_string(),
        time_spec.to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
        "-c:v".to_string(),
        codec.to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, None, processes, cancelled)
}

// ── contact sheet ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn run_contact_sheet(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    cols: u32,
    rows: u32,
    frames: u32,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let frames = frames.max(cols * rows);

    // Determine interval: sample `frames` frames uniformly across the clip.
    // Requires knowing total frame count — approximate via duration * typical fps.
    // We'll instead use the simpler `select='not(mod(n,INTERVAL))'` where
    // INTERVAL = total_frames / frames. If duration is unknown, fall back
    // to every 30th frame.
    let duration = probe_duration(input_path).unwrap_or(60.0);
    // Assume source ~30 fps to derive a reasonable interval — if wrong the
    // output still makes sense, just sampled a bit uneven.
    let total_frames = (duration * 30.0) as u64;
    let interval = (total_frames / frames as u64).max(1);

    // Thumbnail width — tile will then be cols*tile_w wide.
    let tile_w = 320;
    let filter = format!(
        "select='not(mod(n,{interval}))',scale={w}:-1,tile={c}x{r}",
        interval = interval,
        w = tile_w,
        c = cols,
        r = rows,
    );

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        filter,
        "-frames:v".to_string(),
        "1".to_string(),
        "-vsync".to_string(),
        "vfr".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, None, processes, cancelled)
}

// ── frame export ──────────────────────────────────────────────────────────

/// `mode`: "fps" → `value` is frames-per-second; "interval" → `value` is
/// seconds between frames (converted internally to 1/value fps).
#[allow(clippy::too_many_arguments)]
pub fn run_frame_export(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_dir: &str,
    mode: &str,
    value: f64,
    format: &str, // jpeg · png · webp
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    std::fs::create_dir_all(output_dir).map_err(|e| format!("create output dir: {e}"))?;

    let fps_filter = match mode {
        "fps" => format!("fps={:.6}", value.max(0.01)),
        "interval" => format!("fps=1/{:.6}", value.max(0.01)),
        _ => return Err(format!("unknown frame-export mode: {mode}")),
    };

    let out_ext = match format {
        "png" => "png",
        "webp" => "webp",
        _ => "jpg",
    };
    let pattern = format!(
        "{}/frame_%06d.{}",
        output_dir.trim_end_matches('/'),
        out_ext
    );

    let codec = image_codec_for_ext(format);
    let duration = probe_duration(input_path);

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-vf".to_string(),
        fps_filter,
        "-c:v".to_string(),
        codec.to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        pattern,
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── watermark ─────────────────────────────────────────────────────────────

fn corner_overlay_pos(corner: &str) -> &'static str {
    // Margin of 16px from each edge.
    match corner {
        "tl" => "16:16",
        "tr" => "main_w-overlay_w-16:16",
        "bl" => "16:main_h-overlay_h-16",
        "br" => "main_w-overlay_w-16:main_h-overlay_h-16",
        "center" => "(main_w-overlay_w)/2:(main_h-overlay_h)/2",
        _ => "main_w-overlay_w-16:main_h-overlay_h-16",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run_watermark(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    watermark_path: &str,
    corner: &str,
    opacity: f64,
    scale_pct: f64, // percent of video width (0..=100)
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let op = opacity.clamp(0.0, 1.0);
    let sp = scale_pct.clamp(1.0, 100.0) / 100.0;
    let pos = corner_overlay_pos(corner);

    // Scale watermark to sp * main_w (preserve aspect), normalise alpha, set opacity.
    // scale2ref syntax: [wm][ref]scale2ref=... -> emits scaled wm + untouched ref.
    let filter = format!(
        "[1:v]format=rgba,colorchannelmixer=aa={op:.3}[wmalpha];\
         [wmalpha][0:v]scale2ref=w=main_w*{sp:.4}:h=ow/mdar[wmsc][mv];\
         [mv][wmsc]overlay={pos}",
        op = op,
        sp = sp,
        pos = pos,
    );

    let duration = probe_duration(input_path);
    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-i".to_string(),
        watermark_path.to_string(),
        "-filter_complex".to_string(),
        filter,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "18".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "copy".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
