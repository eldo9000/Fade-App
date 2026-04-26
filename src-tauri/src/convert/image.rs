use crate::args::build_image_magick_args;
use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{truncate_stderr, ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Window;

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

    let args = build_image_magick_args(input, output, opts);

    let mut child = match Command::new("magick")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("ImageMagick not found: {e}")),
    };

    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Blocks until process exits or pipe closes after kill
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
            "ImageMagick convert failed".to_string()
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
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting image…");
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
