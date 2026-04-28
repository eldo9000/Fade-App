use crate::args::build_ffmpeg_video_args;
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
    // ── DNxHR minimum-resolution guard ────────────────────────────────────────
    // The dnxhd encoder (which handles DNxHR) requires at least 1280×720.
    // If the caller has explicitly set a resolution we can check it now and
    // return a clear error before spawning ffmpeg.
    //
    // Guard fires only when opts.resolution is explicitly set. If the caller
    // passes no resolution, the input dimensions pass through unchanged —
    // ffmpeg will still reject sub-minimum inputs, but the error will be
    // less descriptive. Full pre-flight dimension detection is deferred.
    if opts.codec.as_deref() == Some("dnxhr") {
        if let Some(res) = &opts.resolution {
            if let Some((w_str, h_str)) = res.split_once('x') {
                if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                    if w < 1280 || h < 720 {
                        return ConvertResult::Error(
                            "DNxHR requires a minimum output resolution of 1280×720. \
                             Set a higher resolution or leave unscaled."
                                .to_string(),
                        );
                    }
                }
            }
        }
    }

    // DNxHD minimum-resolution guard — same constraint as DNxHR.
    if opts.codec.as_deref() == Some("dnxhd") {
        if let Some(res) = &opts.resolution {
            if let Some((w_str, h_str)) = res.split_once('x') {
                if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                    if w < 1280 || h < 720 {
                        return ConvertResult::Error(
                            "DNxHD requires a minimum output resolution of 1280×720. \
                             Set a higher resolution or leave unscaled."
                                .to_string(),
                        );
                    }
                }
            }
        }
    }

    let duration = probe_duration(input);
    let args = build_ffmpeg_video_args(input, output, opts);

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
            "FFmpeg conversion failed".to_string()
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

#[cfg(test)]
mod tests {
    use crate::{ConvertOptions, ConvertResult};
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    fn dnxhr_opts(resolution: Option<&str>) -> ConvertOptions {
        ConvertOptions {
            output_format: "mov".into(),
            codec: Some("dnxhr".into()),
            resolution: resolution.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    // Helper: run the guard path only (no actual ffmpeg). We rely on ffmpeg not
    // being available in the test environment — if it is, the test still checks
    // the ConvertResult::Error variant for the right message, so it's safe.
    fn run_guard(opts: ConvertOptions) -> ConvertResult {
        let processes = Arc::new(Mutex::new(HashMap::new()));
        let cancelled = Arc::new(AtomicBool::new(false));
        super::convert(
            "nonexistent_input.mp4",
            "/tmp/test_dnxhr_guard_out.mov",
            &opts,
            &mut |_| {},
            "test-job",
            processes,
            &cancelled,
        )
    }

    #[test]
    fn dnxhr_resolution_guard_rejects_small_resolution() {
        let result = run_guard(dnxhr_opts(Some("640x360")));
        match result {
            ConvertResult::Error(msg) => {
                assert!(
                    msg.contains("1280"),
                    "Error message should mention 1280, got: {msg}"
                );
            }
            other => panic!("Expected ConvertResult::Error, got {other:?}"),
        }
    }

    #[test]
    fn dnxhr_resolution_guard_allows_hd_resolution() {
        // This will fail past the guard (ffmpeg not found or input missing), but
        // must NOT return the guard-specific error message.
        let result = run_guard(dnxhr_opts(Some("1920x1080")));
        // Guard must not fire for a valid HD resolution.
        if let ConvertResult::Error(msg) = &result {
            assert!(
                !msg.contains("DNxHR requires"),
                "Guard should not fire for 1920x1080, but got: {msg}"
            );
        }
    }

    #[test]
    fn dnxhr_resolution_guard_allows_unset_resolution() {
        // Resolution is None — guard must not fire.
        let result = run_guard(dnxhr_opts(None));
        if let ConvertResult::Error(msg) = &result {
            assert!(
                !msg.contains("DNxHR requires"),
                "Guard should not fire when resolution is unset, but got: {msg}"
            );
        }
    }
}
