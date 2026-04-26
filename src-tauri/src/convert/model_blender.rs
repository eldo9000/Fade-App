use crate::args::model_blender::{blender_not_found_msg, build_blender_args, find_blender};
use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{truncate_stderr, ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Window;

fn locate_script() -> Result<PathBuf, String> {
    // Try cwd-relative first (dev mode), then exe-relative (packaged app).
    let rel = PathBuf::from("scripts/blender_convert.py");
    if rel.exists() {
        return Ok(rel);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("scripts/blender_convert.py");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err("blender_convert.py not found — expected at scripts/blender_convert.py relative to the working directory or executable".to_string())
}

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
pub fn convert(
    input: &str,
    output: &str,
    _opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    progress(ProgressEvent::Started);

    let blender_bin = match find_blender() {
        Some(b) => b,
        None => return ConvertResult::Error(blender_not_found_msg()),
    };
    let script_path = match locate_script() {
        Ok(p) => p,
        Err(e) => return ConvertResult::Error(e),
    };

    let in_ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_blend_input = in_ext == "blend";

    let args = build_blender_args(&blender_bin, &script_path, input, output, is_blend_input);

    let mut child = match Command::new(&blender_bin)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("failed to launch blender: {e}")),
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Drain stdout and watch for the sentinel that confirms a successful export.
    let sentinel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sentinel_clone = Arc::clone(&sentinel_flag);
    let stdout_handle = stdout.map(move |s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                if line.trim() == "FADE_BLENDER_OK" {
                    sentinel_clone.store(true, Ordering::SeqCst);
                }
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

    let exit_success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }

    let sentinel_seen = sentinel_flag.load(Ordering::SeqCst);

    if exit_success && sentinel_seen {
        progress(ProgressEvent::Done);
        ConvertResult::Done
    } else if exit_success && !sentinel_seen {
        ConvertResult::Error("blender conversion failed (no output sentinel)".to_string())
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "blender conversion failed".to_string()
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
    let mut emit =
        crate::convert::window_progress_emitter(window, job_id, "Converting 3D model (Blender)…");
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
