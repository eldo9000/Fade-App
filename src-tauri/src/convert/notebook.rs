//! Jupyter notebook conversion — shells out to `jupyter nbconvert`.
//!
//! Routed by INPUT extension (`.ipynb`), unlike most pipelines which
//! route by output extension. See `lib.rs` convert_file for the
//! input-side branch. Supported output formats: md, py, html.
//!
//! `jupyter nbconvert` auto-generates the output filename from the
//! input stem; we pin it via `--output` (which takes the stem without
//! extension) and pass the parent dir via `--output-dir`.

use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{truncate_stderr, ConvertOptions, ConvertResult, JobProgress};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    progress(ProgressEvent::Started);

    let out_fmt = opts.output_format.to_lowercase();
    let to_flag = match out_fmt.as_str() {
        "md" | "markdown" => "markdown",
        "py" => "python",
        "html" | "htm" => "html",
        other => {
            return ConvertResult::Error(format!("Unsupported notebook output format: {other}"))
        }
    };

    let out_path = Path::new(output);
    let out_dir = out_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    let out_stem = match out_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
    {
        Some(s) => s,
        None => return ConvertResult::Error("Invalid output path".to_string()),
    };

    let mut child = match Command::new("jupyter")
        .args([
            "nbconvert",
            "--to",
            to_flag,
            input,
            "--output",
            &out_stem,
            "--output-dir",
            &out_dir,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ConvertResult::Error(format!(
            "jupyter nbconvert not found: {e}\n\nInstall with:\n  pip install jupyter nbconvert"
        ))
        }
    };

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
        return ConvertResult::Cancelled;
    }

    if success {
        progress(ProgressEvent::Done);
        ConvertResult::Done
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "jupyter nbconvert failed".to_string()
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
                message: "Converting notebook…".to_string(),
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
