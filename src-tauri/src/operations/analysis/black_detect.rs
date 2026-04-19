//! Black-interval detection via the `blackdetect` filter.
//! Parses stderr lines of the form:
//!   [blackdetect @ 0x...] black_start:3.00 black_end:5.04 black_duration:2.04

use super::run_ffmpeg_capture;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct BlackInterval {
    pub start: f64,
    pub end: f64,
    pub duration: f64,
}

#[tauri::command]
pub fn analyze_black_detect(
    input_path: String,
    min_duration: f64, // d=
    pix_th: f64,       // pix_th=
    pic_th: f64,       // pic_th=
) -> Result<Vec<BlackInterval>, String> {
    let filter = format!(
        "blackdetect=d={:.3}:pix_th={:.3}:pic_th={:.3}",
        min_duration, pix_th, pic_th,
    );
    let args = vec![
        "-hide_banner".to_string(),
        "-nostats".to_string(),
        "-i".to_string(),
        input_path,
        "-vf".to_string(),
        filter,
        "-an".to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];
    let stderr = run_ffmpeg_capture(&args)?;

    let mut out = Vec::new();
    for line in stderr.lines() {
        if !line.contains("blackdetect") || !line.contains("black_start") {
            continue;
        }
        let parse = |key: &str| -> Option<f64> {
            let idx = line.find(key)? + key.len();
            line[idx..]
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f64>().ok())
        };
        if let (Some(s), Some(e), Some(d)) = (
            parse("black_start:"),
            parse("black_end:"),
            parse("black_duration:"),
        ) {
            out.push(BlackInterval {
                start: s,
                end: e,
                duration: d,
            });
        }
    }
    Ok(out)
}
