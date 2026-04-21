//! Ebook conversion — shells out to Calibre's `ebook-convert` CLI.
//! Syntax: `ebook-convert in.epub out.mobi`. A single tool covers the
//! whole epub/mobi/azw3/fb2/lit/pdf matrix.
//!
//! Follows the same shape as `convert::timeline` — no progress stream,
//! drain stdout/stderr to avoid pipe deadlock, surface stderr on failure.

use crate::{truncate_stderr, ConvertOptions, JobProgress};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    _opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting ebook…".to_string(),
        },
    );

    let mut child = Command::new("ebook-convert")
        .args([input, output])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ebook-convert not found: {e}\n\nInstall with:\n  macOS:   brew install --cask calibre\n  Linux:   sudo apt install calibre\n  Windows: https://calibre-ebook.com/download"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    let stdout_handle = stdout.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for _ in reader.lines().map_while(Result::ok) {}
        })
    });

    let stderr_content = {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    };

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }

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
        let _ = window.emit(
            "job-progress",
            JobProgress {
                job_id: job_id.to_string(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        );
        Ok(())
    } else {
        Err(if stderr_content.trim().is_empty() {
            "ebook-convert failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}
