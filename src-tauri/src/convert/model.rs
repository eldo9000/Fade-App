//! 3D-model conversion pipeline — shells out to `assimp` (Open Asset Import
//! Library CLI). Follows the same shape as `convert::image` since assimp,
//! like ImageMagick, doesn't emit progress — it runs to completion and we
//! capture stderr for failure diagnosis.
//!
//! Formats that assimp cannot handle (USD, USDZ, Alembic, Blender native)
//! are delegated to `model_blender::run` via `needs_blender`.

use crate::args::build_assimp_args;
use crate::args::model_blender::needs_blender;
use crate::convert::model_blender;
use crate::{truncate_stderr, ConvertOptions, ConvertResult, JobProgress};
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
) -> ConvertResult {
    let input_ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let output_ext = opts.output_format.as_str();

    if needs_blender(input_ext, output_ext) {
        return model_blender::run(window, job_id, input, output, opts, processes, cancelled);
    }

    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting 3D model…".to_string(),
        },
    );

    let args = build_assimp_args(input, output, opts);

    let mut child = match Command::new("assimp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("assimp not found: {e}\n\nInstall with:\n  macOS:   brew install assimp\n  Linux:   apt install assimp-utils  (or equivalent)\n  Windows: scoop install assimp")),
    };

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
        return ConvertResult::Cancelled;
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
        ConvertResult::Done
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "assimp conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}
