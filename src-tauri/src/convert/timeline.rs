//! Timeline / edit-decision-list conversion — shells out to OpenTimelineIO's
//! `otioconvert` CLI. OTIO is the Rosetta stone between Premiere, Resolve,
//! FCP, and Avid; the adapter is auto-detected from the file extension.
//!
//! Follows the same shape as `convert::model` (assimp): no progress stream,
//! drain stdout/stderr to avoid pipe deadlock, surface stderr on failure.
//!
//! AAF support requires the optional `otio-aaf-adapter` extra — install via
//! `pip install opentimelineio[aaf]`. Without it, otioconvert errors out
//! with a clear message which we pass through unmodified.

use crate::{truncate_stderr, ConvertOptions, JobProgress};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
            message: "Converting timeline…".to_string(),
        },
    );

    let mut child = Command::new("otioconvert")
        .args(["-i", input, "-o", output])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("otioconvert not found: {e}\n\nInstall with:\n  pip install opentimelineio\n\nFor AAF support:\n  pip install opentimelineio[aaf]"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock().unwrap();
        map.insert(job_id.to_string(), child);
    }

    // Drain stdout in a background thread so the child can't block on a
    // full pipe while we're reading stderr.
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
        let mut map = processes.lock().unwrap();
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
            "otioconvert failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}
