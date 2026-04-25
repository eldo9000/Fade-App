//! 3D-model conversion pipeline — shells out to `assimp` (Open Asset Import
//! Library CLI). Follows the same shape as `convert::image` since assimp,
//! like ImageMagick, doesn't emit progress — it runs to completion and we
//! capture stderr for failure diagnosis.
//!
//! Formats that assimp cannot handle (USD, USDZ, Alembic, Blender native)
//! are delegated to `model_blender::convert` via `needs_blender`.

use crate::args::build_assimp_args;
use crate::args::model_blender::needs_blender;
use crate::convert::model_blender;
use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{truncate_stderr, ConvertOptions, ConvertResult, JobProgress};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// Dispatches to `model_blender::convert` for formats assimp cannot handle.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    let input_ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let output_ext = opts.output_format.as_str();

    if needs_blender(input_ext, output_ext) {
        return model_blender::convert(input, output, opts, progress, job_id, processes, cancelled);
    }

    progress(ProgressEvent::Started);

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
        progress(ProgressEvent::Done);
        ConvertResult::Done
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "assimp conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}

pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let job_id_owned = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = match ev {
            ProgressEvent::Started => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: "Converting 3D model…".to_string(),
            },
            ProgressEvent::Phase(msg) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: msg,
            },
            ProgressEvent::Percent(p) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: (p * 100.0).clamp(0.0, 100.0),
                message: String::new(),
            },
            ProgressEvent::Done => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        };
        let _ = win.emit("job-progress", payload);
    };
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
