//! Audio normalization — three modes:
//!   · EBU R128 two-pass loudnorm (measure, then apply with measured values)
//!   · Peak: volumedetect → scale to target dBFS
//!   · ReplayGain tag-only: write `replaygain_*` tags with measured gain
//!
//! Emits a real file via the same job-progress pipeline as other mechanical
//! ops. Unlike the one-shot analysis filters, this is a producer op, so it
//! goes through the `run_operation` dispatch.

use crate::operations::analysis::run_ffmpeg_capture;
use crate::operations::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::Window;

#[derive(serde::Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum NormMode {
    Ebu,
    Peak,
    Rg,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    mode: NormMode,
    target_i: f64,
    target_tp: f64,
    target_lra: f64,
    linear: bool,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    match mode {
        NormMode::Ebu => {
            // Pass 1 — measure
            let pass1_filter = format!(
                "loudnorm=I={i}:TP={tp}:LRA={lra}:print_format=json",
                i = target_i,
                tp = target_tp,
                lra = target_lra,
            );
            let pass1_args = vec![
                "-hide_banner".to_string(),
                "-nostats".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-af".to_string(),
                pass1_filter,
                "-f".to_string(),
                "null".to_string(),
                "-".to_string(),
            ];
            let stderr = run_ffmpeg_capture(&pass1_args)?;
            let start = stderr
                .rfind('{')
                .ok_or_else(|| "loudnorm pass1 produced no JSON block".to_string())?;
            let tail = &stderr[start..];
            let end = tail
                .find('}')
                .ok_or_else(|| "loudnorm pass1 JSON block not closed".to_string())?;
            let v: serde_json::Value = serde_json::from_str(&tail[..=end])
                .map_err(|e| format!("loudnorm pass1 JSON parse: {e}"))?;

            let measured_i = v["input_i"].as_str().unwrap_or("-23.0");
            let measured_tp = v["input_tp"].as_str().unwrap_or("-2.0");
            let measured_lra = v["input_lra"].as_str().unwrap_or("7.0");
            let measured_thresh = v["input_thresh"].as_str().unwrap_or("-34.0");
            let offset = v["target_offset"].as_str().unwrap_or("0.0");

            // Pass 2 — apply with measured values and optional linear mode
            let pass2_filter = format!(
                "loudnorm=I={i}:TP={tp}:LRA={lra}:measured_I={mi}:measured_TP={mtp}:measured_LRA={mlra}:measured_thresh={mth}:offset={off}{lin}",
                i = target_i,
                tp = target_tp,
                lra = target_lra,
                mi = measured_i,
                mtp = measured_tp,
                mlra = measured_lra,
                mth = measured_thresh,
                off = offset,
                lin = if linear { ":linear=true" } else { "" },
            );
            let args = vec![
                "-y".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-af".to_string(),
                pass2_filter,
                "-progress".to_string(),
                "pipe:1".to_string(),
                output_path.to_string(),
            ];
            run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
        }
        NormMode::Peak => {
            // Peak: measure max_volume with volumedetect, then apply a gain.
            let detect_args = vec![
                "-hide_banner".to_string(),
                "-nostats".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-af".to_string(),
                "volumedetect".to_string(),
                "-f".to_string(),
                "null".to_string(),
                "-".to_string(),
            ];
            let stderr = run_ffmpeg_capture(&detect_args)?;
            // "max_volume: -3.5 dB"
            let max_db = stderr
                .lines()
                .find_map(|l| l.find("max_volume:").map(|i| l[i + 11..].trim().to_string()))
                .and_then(|s| s.trim_end_matches(" dB").trim().parse::<f64>().ok())
                .ok_or_else(|| "volumedetect produced no max_volume".to_string())?;
            let gain_db = target_i - max_db; // target_i is the dBFS ceiling in peak mode
            let args = vec![
                "-y".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-af".to_string(),
                format!("volume={:.3}dB", gain_db),
                "-progress".to_string(),
                "pipe:1".to_string(),
                output_path.to_string(),
            ];
            run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
        }
        NormMode::Rg => {
            // ReplayGain: stream-copy and write track_gain tag based on measured LUFS.
            // We measure with a quick loudnorm pass and write REPLAYGAIN_TRACK_GAIN.
            let pass1_filter = format!("loudnorm=I={:.1}:print_format=json", target_i);
            let pass1_args = vec![
                "-hide_banner".to_string(),
                "-nostats".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-af".to_string(),
                pass1_filter,
                "-f".to_string(),
                "null".to_string(),
                "-".to_string(),
            ];
            let stderr = run_ffmpeg_capture(&pass1_args)?;
            let start = stderr
                .rfind('{')
                .ok_or_else(|| "loudnorm produced no JSON block".to_string())?;
            let tail = &stderr[start..];
            let end = tail
                .find('}')
                .ok_or_else(|| "loudnorm JSON block not closed".to_string())?;
            let v: serde_json::Value = serde_json::from_str(&tail[..=end])
                .map_err(|e| format!("loudnorm JSON parse: {e}"))?;
            let measured_i: f64 = v["input_i"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(-23.0);
            let gain_db = target_i - measured_i;

            let args = vec![
                "-y".to_string(),
                "-i".to_string(),
                input_path.to_string(),
                "-c".to_string(),
                "copy".to_string(),
                "-metadata".to_string(),
                format!("REPLAYGAIN_TRACK_GAIN={:.2} dB", gain_db),
                "-progress".to_string(),
                "pipe:1".to_string(),
                output_path.to_string(),
            ];
            run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
        }
    }
}
