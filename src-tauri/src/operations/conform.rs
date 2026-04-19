//! Conform: re-encode a video to match a target fps / resolution / pixel format.
//!
//! Always re-encodes (no way around it once fps or res changes).  Exposes
//! multiple algorithm choices because the quality spread between drop/dup,
//! frame-blend, and motion-compensated interpolation is enormous on anything
//! that isn't an integer ratio.
//!
//! Filter chain order:  minterpolate/framerate/fps  →  scale  →  zscale/format
//!
//! Video codec: libx264 CRF 17 preset=slow (visually lossless-ish intermediate).
//! Audio: stream-copy (conform touches video only).

use super::{run_ffmpeg, run_ffprobe};
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum FpsAlgo {
    /// `fps` filter — drop/duplicate frames. Fast, deterministic, judder on
    /// non-integer ratios.
    Drop,
    /// `framerate` filter — linear blend of neighbouring frames. Smoother, but
    /// ghosts motion.
    Blend,
    /// `minterpolate` — motion-compensated frame interpolation. Highest
    /// quality, 5–30× slower. Can warp on complex motion / occlusions.
    Mci,
}

#[derive(serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ScaleAlgo {
    Bilinear,
    Bicubic,
    Lanczos,
    Spline,
}

impl ScaleAlgo {
    fn as_flag(&self) -> &'static str {
        match self {
            ScaleAlgo::Bilinear => "bilinear",
            ScaleAlgo::Bicubic => "bicubic",
            ScaleAlgo::Lanczos => "lanczos",
            ScaleAlgo::Spline => "spline",
        }
    }
}

/// Map a UI-friendly fps string to an ffmpeg-safe rational.  Broadcast rates
/// become their exact rationals (24000/1001, 30000/1001, 60000/1001) so we
/// don't accumulate drift.
fn fps_to_rational(s: &str) -> Option<String> {
    match s {
        "23.976" => Some("24000/1001".to_string()),
        "24" => Some("24".to_string()),
        "25" => Some("25".to_string()),
        "29.97" => Some("30000/1001".to_string()),
        "30" => Some("30".to_string()),
        "50" => Some("50".to_string()),
        "59.94" => Some("60000/1001".to_string()),
        "60" => Some("60".to_string()),
        _ => None,
    }
}

/// Source video stream metadata we need to decide which filter stages to emit.
struct SourceSpec {
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    // 10-bit depth detection — used to decide whether to apply dither.
    source_is_10bit: bool,
}

fn probe_source(input_path: &str) -> Result<SourceSpec, String> {
    let json = run_ffprobe(input_path)?;
    let streams = json["streams"].as_array().cloned().unwrap_or_default();
    let v = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("video"))
        .ok_or_else(|| "No video stream found in input".to_string())?;

    let pix_fmt = v["pix_fmt"].as_str().map(|s| s.to_string());
    let source_is_10bit = pix_fmt
        .as_deref()
        .map(|p| p.contains("10le") || p.contains("10be") || p.contains("p010"))
        .unwrap_or(false);

    Ok(SourceSpec {
        width: v["width"].as_u64().map(|v| v as u32),
        height: v["height"].as_u64().map(|v| v as u32),
        pix_fmt,
        source_is_10bit,
    })
}

fn target_is_8bit(pix_fmt: &str) -> bool {
    !(pix_fmt.contains("10") || pix_fmt.contains("p010") || pix_fmt.contains("12"))
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    fps: Option<String>,      // None = source; else UI string ("23.976", "60", ...)
    resolution: Option<String>, // None = source; else "1920x1080"
    pix_fmt: Option<String>,    // None = source
    fps_algo: FpsAlgo,
    scale_algo: ScaleAlgo,
    dither: bool,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let src = probe_source(input_path)?;
    let duration = probe_duration(input_path);

    // ── Build filter chain ────────────────────────────────────────────────
    let mut filters: Vec<String> = Vec::new();

    // 1) Framerate stage — only when target fps is set AND differs (we can't
    //    cheaply detect "differs" without another probe; emit unconditionally
    //    when target specified — ffmpeg no-ops if already at target).
    if let Some(fps_str) = fps.as_deref() {
        let rational = fps_to_rational(fps_str)
            .ok_or_else(|| format!("Unsupported fps value: {}", fps_str))?;
        match fps_algo {
            FpsAlgo::Drop => filters.push(format!("fps={}", rational)),
            FpsAlgo::Blend => filters.push(format!("framerate=fps={}", rational)),
            FpsAlgo::Mci => filters.push(format!(
                "minterpolate=fps={}:mi_mode=mci:mc_mode=aobmc:me_mode=bidir:vsbmc=1:scd=fdiff:scd_threshold=5",
                rational
            )),
        }
    }

    // 2) Scale stage — only when target resolution is set AND differs from source.
    if let Some(res) = resolution.as_deref() {
        let differs = match (src.width, src.height) {
            (Some(w), Some(h)) => format!("{}x{}", w, h) != res,
            _ => true,
        };
        if differs {
            // Validate format "WxH".
            if !res.contains('x') {
                return Err(format!("Invalid resolution: {}", res));
            }
            filters.push(format!(
                "scale={}:flags={}",
                res.replace('x', ":"),
                scale_algo.as_flag()
            ));
        }
    }

    // 3) Pixel format / dither stage.
    if let Some(target_pf) = pix_fmt.as_deref() {
        let differs = src
            .pix_fmt
            .as_deref()
            .map(|p| p != target_pf)
            .unwrap_or(true);
        if differs {
            let need_dither = dither && src.source_is_10bit && target_is_8bit(target_pf);
            if need_dither {
                // zscale provides proper error-diffusion dither; format= re-caps
                // the pix_fmt for the encoder.
                filters.push(format!(
                    "zscale=dither=error_diffusion,format={}",
                    target_pf
                ));
            } else {
                filters.push(format!("format={}", target_pf));
            }
        }
    } else if dither && src.source_is_10bit {
        // User didn't pick a target pix_fmt but kept dither on — this is a
        // no-op in practice (encoder will keep source depth). Skip.
    }

    // ── Build ffmpeg args ─────────────────────────────────────────────────
    let mut args: Vec<String> = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
    ];

    if !filters.is_empty() {
        args.push("-vf".to_string());
        args.push(filters.join(","));
    }

    // Video encoder — high-quality intermediate.
    args.extend([
        "-c:v", "libx264",
        "-crf", "17",
        "-preset", "slow",
        "-pix_fmt", pix_fmt.as_deref().unwrap_or("yuv420p"),
        // Audio: stream-copy (conform touches video only).
        "-c:a", "copy",
        // Keep subtitles / data streams untouched.
        "-map", "0",
        "-c:s", "copy",
        "-c:d", "copy",
        // Progress reporting to stdout.
        "-progress", "pipe:1",
    ].into_iter().map(String::from));

    args.push(output_path.to_string());

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
