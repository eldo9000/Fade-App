//! 3D-model conversion pipeline — shells out to `assimp` (Open Asset Import
//! Library CLI). Follows the same shape as `convert::image` since assimp,
//! like ImageMagick, doesn't emit progress — it runs to completion and we
//! capture stderr for failure diagnosis.

use crate::args::build_assimp_args;
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
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting 3D model…".to_string(),
        },
    );

    let args = build_assimp_args(input, output, opts);

    let mut child = Command::new("assimp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("assimp not found: {e}\n\nInstall with:\n  macOS:   brew install assimp\n  Linux:   apt install assimp-utils  (or equivalent)\n  Windows: scoop install assimp"))?;

    // Assimp writes progress-ish info to stdout and errors to stderr.
    // Drain both so the child can't block on a full pipe. We only surface
    // stderr on failure.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Drain stdout in a background thread so we never deadlock on a full
    // pipe while blocked reading stderr.
    let stdout_handle = stdout.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for _ in reader.lines().map_while(Result::ok) {
                // assimp stdout is chatty ("export took approx. 0.001s") —
                // we don't surface it. Just keep the pipe flowing.
            }
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
            "assimp conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}
