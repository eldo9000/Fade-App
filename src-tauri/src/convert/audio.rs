use crate::args::build_ffmpeg_audio_args;
use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{parse_out_time_ms, probe_duration, truncate_stderr, ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Window;

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
///
/// Progress emission for ffmpeg-driven encodes uses a paired
/// `Phase(<elapsed-message>)` + `Percent(<fraction>)` cadence: the wrapper
/// coalesces a Phase immediately followed by a Percent into the single
/// `{percent, message}` payload the frontend expects.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    let duration = probe_duration(input);
    let args = build_ffmpeg_audio_args(input, output, opts);

    let mut child = match Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("ffmpeg not found: {e}")),
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
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

    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(elapsed) = parse_out_time_ms(&line) {
                let fraction = if let Some(dur) = duration {
                    (elapsed / dur).min(0.99) as f32
                } else {
                    0.0
                };
                progress(ProgressEvent::Phase(format!("{:.0}s elapsed", elapsed)));
                progress(ProgressEvent::Percent(fraction));
            }
        }
    }

    let error_output = stderr_thread.join().unwrap_or_default();

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
        ConvertResult::Done
    } else {
        let _ = std::fs::remove_file(output);
        ConvertResult::Error(if error_output.trim().is_empty() {
            "FFmpeg audio conversion failed".to_string()
        } else {
            truncate_stderr(&error_output)
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
    let mut emit = crate::convert::window_progress_emitter_batched(window, job_id);
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
