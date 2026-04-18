use crate::truncate_stderr;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[derive(Serialize, Clone)]
pub struct ImageQualityPreview {
    diff_path: String,
    compressed_path: String,
}

/// Encode the source image at `quality` in `output_format`, then compute a
/// per-pixel difference against the original and return both as temp file paths.
/// Only meaningful for lossy formats (JPEG, WebP, AVIF).
#[command]
pub fn preview_image_quality(
    path: String,
    quality: u32,
    output_format: String,
) -> Result<ImageQualityPreview, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    match output_format.as_str() {
        "jpeg" | "jpg" | "webp" | "avif" => {},
        other => return Err(format!("{other} is lossless — no compression artifacts to preview")),
    }

    let tmp_dir = std::env::temp_dir();
    let job_id = uuid::Uuid::new_v4().to_string();
    let ext = if output_format == "jpeg" { "jpg" } else { output_format.as_str() };
    let compressed = tmp_dir.join(format!("fade-imgq-enc-{job_id}.{ext}"));
    let diff = tmp_dir.join(format!("fade-imgq-diff-{job_id}.png"));

    // Pass 1: encode at requested quality (induces lossy compression artifacts)
    let enc_out = Command::new("magick")
        .args([
            path.as_str(),
            "-quality",
            &quality.to_string(),
            compressed.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("magick not found: {e}"))?;
    if !enc_out.status.success() {
        return Err(format!(
            "encode failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&enc_out.stderr))
        ));
    }

    // Pass 2: amplified grayscale difference (original − encoded)
    let diff_out = Command::new("magick")
        .args([
            path.as_str(),
            compressed.to_str().unwrap_or(""),
            "-compose",
            "Difference",
            "-composite",
            "-evaluate",
            "multiply",
            "8",
            "-colorspace",
            "gray",
            diff.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("magick not found: {e}"))?;
    if !diff_out.status.success() {
        let _ = std::fs::remove_file(&compressed);
        return Err(format!(
            "diff failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&diff_out.stderr))
        ));
    }

    Ok(ImageQualityPreview {
        diff_path: diff.to_string_lossy().to_string(),
        compressed_path: compressed.to_string_lossy().to_string(),
    })
}
