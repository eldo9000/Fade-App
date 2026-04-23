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

use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

/// Spawn ffmpeg, register the child in `processes` under `job_id` so it can be
/// cancelled, then wait for completion. Returns captured stderr on success or
/// `Err("CANCELLED")` if the cancellation flag tripped. Mirrors the
/// process-map + TOCTOU-safe pattern used by `operations::run_ffmpeg`, but for
/// analysis ops that parse stderr instead of tracking progress on stdout.
pub(crate) fn run_ffmpeg_capture_registered(
    args: &[String],
    processes: Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<String, String> {
    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    let stderr = child.stderr.take();
    // Drain stdout in a background thread so a filled pipe doesn't deadlock
    // the child before the stderr reader finishes.
    let stdout = child.stdout.take();
    let stdout_thread = std::thread::spawn(move || {
        if let Some(s) = stdout {
            let reader = BufReader::new(s);
            for _ in reader.lines().map_while(Result::ok) {}
        }
    });

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Close the cancel TOCTOU window — cancel_job may have already tried to
    // kill via the processes map before the child was registered.
    if cancelled.load(Ordering::SeqCst) {
        let mut map = processes.lock();
        if let Some(child) = map.get_mut(job_id) {
            let _ = child.kill();
        }
    }

    let stderr_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    });

    let error_output = stderr_thread.join().unwrap_or_default();
    let _ = stdout_thread.join();

    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    if cancelled.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if success {
        Ok(error_output)
    } else {
        // Analysis filters sometimes exit non-zero while still emitting the
        // measurement text we want. Return stderr so the caller can decide.
        Err(if error_output.trim().is_empty() {
            "FFmpeg analysis failed".to_string()
        } else {
            error_output
        })
    }
}
