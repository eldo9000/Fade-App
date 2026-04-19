//! Analysis operations — read-only measurements that return JSON directly.
//!
//! Unlike the mechanical ops in the parent module, these don't emit
//! job-progress events and don't produce an output file. They run FFmpeg
//! synchronously, parse stderr/stdout, and return a typed result.

pub mod audio_norm;
pub mod black_detect;
pub mod cut_detect;
pub mod framemd5;
pub mod loudness;
pub mod vmaf;

use std::process::Command;

/// Run ffmpeg with `args`, collect full stderr, and return it.
/// Used by analysis ops that parse printed measurements from stderr.
pub(crate) fn run_ffmpeg_capture(args: &[String]) -> Result<String, String> {
    let out = Command::new("ffmpeg")
        .args(args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;
    // Many analysis filters print to stderr even on success.
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        // Still return stderr so callers that expect measurements there can
        // inspect it; but for filters that require success, treat as error.
        return Err(if stderr.trim().is_empty() {
            "FFmpeg analysis failed".to_string()
        } else {
            stderr
        });
    }
    Ok(stderr)
}
