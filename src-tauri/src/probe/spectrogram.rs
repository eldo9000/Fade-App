use crate::truncate_stderr;
use std::process::Command;
use tauri::command;

/// Render a rainbow spectrogram PNG via ffmpeg showspectrumpic and return it as base64.
/// Uses image2pipe + png codec to write the PNG directly to stdout — no temp files.
#[command]
pub fn get_spectrogram(path: String) -> Result<String, String> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            &path,
            "-lavfi",
            "showspectrumpic=s=800x200:legend=0:color=magma:scale=log:fscale=log",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "spectrogram failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&output.stderr))
        ));
    }

    if output.stdout.is_empty() {
        return Err("spectrogram produced no output".to_string());
    }

    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.encode(&output.stdout))
}
