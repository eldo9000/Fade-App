use serde::Serialize;
use std::process::Command;
use tauri::command;

#[derive(Serialize)]
pub struct WaveformData {
    pub amplitudes: Vec<f32>,
    /// HSL hue (0-360) for each bar, derived from per-chunk dominant frequency.
    pub hues: Vec<u32>,
}

/// Zero-crossing rate → HSL hue.
/// At 8 000 Hz sample rate, ZCR × 4 000 ≈ dominant frequency in Hz.
/// Bass (red/orange) → mids (yellow/green) → hi-hats (cyan/blue).
fn zcr_to_hue(zcr: f32) -> u32 {
    // 0.0 (DC / sub-bass) → hue 0   (red)
    // 0.5 (2 kHz mids)    → hue 120 (green)
    // 1.0 (4 kHz hi-hats) → hue 240 (blue)
    let clamped = zcr.clamp(0.0, 1.0);
    (clamped * 240.0) as u32
}

/// Extract a 500-point RMS waveform plus per-bar frequency hue.
/// Uses zero-crossing rate at 8 000 Hz — fast, no extra deps, works well
/// for distinguishing bass kicks from hi-hats visually.
#[command]
pub fn get_waveform(
    path: String,
    draft: bool,
    buckets: Option<usize>,
) -> Result<WaveformData, String> {
    let ar = if draft { "2000" } else { "8000" };
    let output = Command::new("ffmpeg")
        .args(["-i", &path, "-ac", "1", "-ar", ar, "-f", "f32le", "-"])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if output.stdout.is_empty() {
        return Ok(WaveformData {
            amplitudes: vec![],
            hues: vec![],
        });
    }

    let samples: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .filter_map(|c| c.try_into().ok().map(f32::from_le_bytes))
        .collect();

    let n = buckets.unwrap_or(500).clamp(100, 8000);
    let chunk_size = (samples.len() / n).max(1);
    let mut amplitudes = Vec::with_capacity(n);
    let mut hues = Vec::with_capacity(n);

    for chunk in samples.chunks(chunk_size).take(n) {
        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        amplitudes.push(rms);

        let crossings = chunk
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        let zcr = crossings as f32 / chunk.len() as f32;
        hues.push(zcr_to_hue(zcr));
    }

    // Normalise amplitudes to [0, 1]
    let max = amplitudes.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for a in &mut amplitudes {
            *a /= max;
        }
    }

    Ok(WaveformData { amplitudes, hues })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zcr_to_hue_low_maps_to_red() {
        assert_eq!(zcr_to_hue(0.0), 0);
    }

    #[test]
    fn zcr_to_hue_mid_maps_to_green() {
        assert_eq!(zcr_to_hue(0.5), 120);
    }

    #[test]
    fn zcr_to_hue_high_maps_to_blue() {
        assert_eq!(zcr_to_hue(1.0), 240);
    }

    #[test]
    fn zcr_to_hue_clamps_out_of_range_inputs() {
        assert_eq!(zcr_to_hue(-1.0), 0, "negative clamps to 0 -> red");
        assert_eq!(zcr_to_hue(2.0), 240, "above 1.0 clamps to blue");
        assert_eq!(zcr_to_hue(f32::NAN), 0, "NaN clamps via clamp semantics");
    }

    #[test]
    fn zcr_to_hue_never_exceeds_240() {
        for i in 0..=100 {
            let z = i as f32 / 100.0;
            let h = zcr_to_hue(z);
            assert!(h <= 240, "zcr={z} produced hue {h}");
        }
    }

    #[test]
    fn waveform_data_serializes_as_expected_shape() {
        let w = WaveformData {
            amplitudes: vec![0.1, 0.5, 1.0],
            hues: vec![0, 120, 240],
        };
        let v: serde_json::Value = serde_json::to_value(&w).unwrap();
        assert!(v["amplitudes"].is_array());
        assert!(v["hues"].is_array());
        assert_eq!(v["amplitudes"].as_array().unwrap().len(), 3);
        assert_eq!(v["hues"][1].as_u64(), Some(120));
    }
}
