//! Chroma key (FFmpeg tier) — background removal via the built-in
//! `chromakey` / `colorkey` / `hsvkey` filters, optional `despill`, and
//! an alpha-capable output codec.
//!
//! No new deps — all work happens in FFmpeg. Also exposes a one-frame
//! preview command that writes a PNG to temp and returns the path.

use super::run_ffmpeg;
use crate::probe_duration;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{command, Window};

/// Keying algorithm selector — maps 1:1 to an ffmpeg filter name.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChromaAlgo {
    Chromakey,
    Colorkey,
    Hsvkey,
}

/// Alpha-capable output target. Each variant pins codec + pix_fmt + ext.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChromaOutput {
    MovQtrle,
    MovProres4444,
    WebmVp9,
    PngSequence,
    MkvFfv1,
}

/// Parse a `#RRGGBB` / `RRGGBB` string into `(r, g, b)` u8 components.
pub(crate) fn parse_hex_rgb(hex: &str) -> Result<(u8, u8, u8), String> {
    let s = hex.trim().trim_start_matches('#');
    if s.len() != 6 {
        return Err(format!("bad color '{hex}' — expected #RRGGBB"));
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
    Ok((r, g, b))
}

/// Convert RGB (0..=255) → HSV with hue in degrees (0..360), sat/val in 0..1.
pub(crate) fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let h = if delta.abs() < 1e-9 {
        0.0
    } else if (max - rf).abs() < 1e-9 {
        60.0 * (((gf - bf) / delta).rem_euclid(6.0))
    } else if (max - gf).abs() < 1e-9 {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };
    let s = if max < 1e-9 { 0.0 } else { delta / max };
    (h, s, max)
}

/// Return "green" when the picked color's G dominates, else "blue". Used as
/// the `despill=type=` argument. Skipped for non-green/blue shades where
/// despill would over-correct.
pub(crate) fn despill_type_for(hex: &str) -> Option<&'static str> {
    let (r, g, b) = parse_hex_rgb(hex).ok()?;
    if g > r && g > b {
        Some("green")
    } else if b > r && b > g {
        Some("blue")
    } else {
        None
    }
}

/// Build the `-vf` filter chain for the chroma op.
/// `for_preview=true` appends `,format=rgba` so stdout PNGs render on a
/// checkerboard without an intermediate alpha-capable container.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_vf(
    algo: ChromaAlgo,
    color_hex: &str,
    similarity: f64,
    blend: f64,
    despill: bool,
    despill_mix: f64,
    upsample: bool,
    for_preview: bool,
) -> Result<String, String> {
    let (r, g, b) = parse_hex_rgb(color_hex)?;
    let sim = similarity.clamp(0.01, 0.40);
    let bl = blend.clamp(0.0, 0.5);

    let mut parts: Vec<String> = Vec::new();
    if upsample {
        parts.push("format=yuv444p".to_string());
    }

    match algo {
        ChromaAlgo::Chromakey => {
            parts.push(format!(
                "chromakey=color=0x{:02X}{:02X}{:02X}:similarity={sim}:blend={bl}",
                r, g, b
            ));
        }
        ChromaAlgo::Colorkey => {
            parts.push(format!(
                "colorkey=color=0x{:02X}{:02X}{:02X}:similarity={sim}:blend={bl}",
                r, g, b
            ));
        }
        ChromaAlgo::Hsvkey => {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            parts.push(format!(
                "hsvkey=hue={h:.2}:sat={s:.4}:val={v:.4}:similarity={sim}:blend={bl}"
            ));
        }
    }

    // Despill cleans up the colored light spill on the subject's edges.
    // Skipped for colorkey (hard cuts don't have soft-edge spill) and when
    // the picked color isn't a clear green/blue (magenta/red keys etc.).
    if despill && algo != ChromaAlgo::Colorkey {
        if let Some(kind) = despill_type_for(color_hex) {
            let mix = despill_mix.clamp(0.0, 1.0);
            parts.push(format!("despill=type={kind}:mix={mix}:expand=0"));
        }
    }

    if for_preview {
        parts.push("format=rgba".to_string());
    }
    Ok(parts.join(","))
}

/// Output codec/muxer args for the selected alpha container.
fn enc_args_for(target: ChromaOutput) -> Vec<String> {
    match target {
        ChromaOutput::MovQtrle => vec![
            "-c:v".into(),
            "qtrle".into(),
            "-pix_fmt".into(),
            "argb".into(),
        ],
        ChromaOutput::MovProres4444 => vec![
            "-c:v".into(),
            "prores_ks".into(),
            "-profile:v".into(),
            "4444".into(),
            "-pix_fmt".into(),
            "yuva444p10le".into(),
        ],
        ChromaOutput::WebmVp9 => vec![
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-pix_fmt".into(),
            "yuva420p".into(),
            "-b:v".into(),
            "0".into(),
            "-crf".into(),
            "20".into(),
        ],
        ChromaOutput::MkvFfv1 => vec![
            "-c:v".into(),
            "ffv1".into(),
            "-level".into(),
            "3".into(),
            "-pix_fmt".into(),
            "yuva420p".into(),
        ],
        // PNG sequence handled separately (muxer image2, pattern output).
        ChromaOutput::PngSequence => vec![
            "-c:v".into(),
            "png".into(),
            "-pix_fmt".into(),
            "rgba".into(),
        ],
    }
}

/// Run the chroma-key op end-to-end.
#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    algo: ChromaAlgo,
    color_hex: &str,
    similarity: f64,
    blend: f64,
    despill: bool,
    despill_mix: f64,
    upsample: bool,
    output_target: ChromaOutput,
    trim_start: Option<f64>,
    trim_end: Option<f64>,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let vf = build_vf(
        algo,
        color_hex,
        similarity,
        blend,
        despill,
        despill_mix,
        upsample,
        false,
    )?;

    let duration = probe_duration(input_path);

    let mut args: Vec<String> = vec!["-y".into()];
    if let Some(ts) = trim_start {
        if ts > 0.0 {
            args.push("-ss".into());
            args.push(format!("{ts:.3}"));
        }
    }
    if let Some(te) = trim_end {
        let start = trim_start.unwrap_or(0.0);
        if te > start {
            args.push("-to".into());
            args.push(format!("{te:.3}"));
        }
    }
    args.push("-i".into());
    args.push(input_path.into());
    args.push("-vf".into());
    args.push(vf);
    // Alpha outputs are video-only; existing audio is dropped because most
    // alpha codecs here don't mux audio cleanly and the typical use-case is
    // a matte asset feeding a compositor.
    args.push("-an".into());
    args.extend(enc_args_for(output_target));
    args.push("-progress".into());
    args.push("pipe:1".into());

    // PNG sequence writes `<output_path>/%05d.png` where output_path is
    // a directory the frontend pre-creates.
    if matches!(output_target, ChromaOutput::PngSequence) {
        std::fs::create_dir_all(output_path).map_err(|e| format!("mkdir {output_path}: {e}"))?;
        args.push(format!("{}/%05d.png", output_path.trim_end_matches('/')));
    } else {
        args.push(output_path.to_string());
    }

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── Preview command ────────────────────────────────────────────────────────

/// Render a single frame at time `time_s` through the chroma filter chain
/// and write it to a PNG in the OS temp dir. Returns the path so the
/// frontend can load it via `convertFileSrc`.
#[allow(clippy::too_many_arguments)]
#[command]
pub fn chroma_key_preview(
    input_path: String,
    time_s: f64,
    algo: ChromaAlgo,
    color_hex: String,
    similarity: f64,
    blend: f64,
    despill: bool,
    despill_mix: f64,
    upsample: bool,
) -> Result<String, String> {
    let vf_core = build_vf(
        algo,
        &color_hex,
        similarity,
        blend,
        despill,
        despill_mix,
        upsample,
        true,
    )?;
    // Downscale to max 960px wide for preview speed. `-1` keeps aspect.
    let vf = format!("{vf_core},scale='min(960,iw)':-2");

    let ts = time_s.max(0.0);
    let job_id = uuid::Uuid::new_v4().to_string();
    let out = std::env::temp_dir().join(format!("fade-chroma-preview-{job_id}.png"));

    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss",
            &format!("{ts:.3}"),
            "-i",
            &input_path,
            "-vf",
            &vf,
            "-frames:v",
            "1",
            out.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(crate::truncate_stderr(&stderr));
    }

    Ok(out.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_rgb_ok() {
        assert_eq!(parse_hex_rgb("#00FF00").unwrap(), (0, 255, 0));
        assert_eq!(parse_hex_rgb("0000ff").unwrap(), (0, 0, 255));
    }

    #[test]
    fn parse_hex_rgb_bad() {
        assert!(parse_hex_rgb("#fff").is_err());
        assert!(parse_hex_rgb("notahex").is_err());
    }

    #[test]
    fn rgb_to_hsv_green() {
        let (h, s, v) = rgb_to_hsv(0, 255, 0);
        assert!((h - 120.0).abs() < 0.1);
        assert!((s - 1.0).abs() < 1e-6);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rgb_to_hsv_blue() {
        let (h, _, _) = rgb_to_hsv(0, 0, 255);
        assert!((h - 240.0).abs() < 0.1);
    }

    #[test]
    fn despill_type_picks_green() {
        assert_eq!(despill_type_for("#00FF00"), Some("green"));
        assert_eq!(despill_type_for("#0000FF"), Some("blue"));
        assert_eq!(despill_type_for("#FF0000"), None);
    }

    #[test]
    fn build_vf_chromakey_emits_hex() {
        let vf = build_vf(
            ChromaAlgo::Chromakey,
            "#00FF00",
            0.10,
            0.10,
            false,
            0.5,
            true,
            false,
        )
        .unwrap();
        assert!(vf.starts_with("format=yuv444p,"));
        assert!(vf.contains("chromakey=color=0x00FF00"));
        assert!(vf.contains("similarity=0.1"));
    }

    #[test]
    fn build_vf_despill_on_green() {
        let vf = build_vf(
            ChromaAlgo::Chromakey,
            "#00FF00",
            0.10,
            0.10,
            true,
            0.5,
            false,
            false,
        )
        .unwrap();
        assert!(vf.contains("despill=type=green"));
    }

    #[test]
    fn build_vf_despill_skipped_for_colorkey() {
        let vf = build_vf(
            ChromaAlgo::Colorkey,
            "#00FF00",
            0.10,
            0.10,
            true,
            0.5,
            false,
            false,
        )
        .unwrap();
        assert!(!vf.contains("despill"));
    }

    #[test]
    fn build_vf_hsvkey() {
        let vf = build_vf(
            ChromaAlgo::Hsvkey,
            "#00FF00",
            0.10,
            0.10,
            false,
            0.5,
            false,
            false,
        )
        .unwrap();
        assert!(vf.starts_with("hsvkey=hue="));
        assert!(vf.contains("sat="));
        assert!(vf.contains("val="));
    }

    #[test]
    fn build_vf_preview_appends_rgba() {
        let vf = build_vf(
            ChromaAlgo::Chromakey,
            "#00FF00",
            0.10,
            0.10,
            false,
            0.5,
            false,
            true,
        )
        .unwrap();
        assert!(vf.ends_with(",format=rgba"));
    }
}
