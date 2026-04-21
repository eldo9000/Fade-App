//! Mechanical video/audio operations.
//!
//! Each submodule implements one operation that drives FFmpeg as a subprocess,
//! emitting the same `job-progress` / `job-done` / `job-error` / `job-cancelled`
//! events used by the existing `convert_file` command.

pub mod analysis;
pub mod audio_filters;
pub mod audio_offset;
pub mod chroma_key;
pub mod conform;
pub mod cut;
pub mod extract;
pub mod frame_ops;
pub mod loop_op;
pub mod merge;
pub mod metadata_strip;
pub mod rate_limiter;
pub mod remove_audio;
pub mod remove_video;
pub mod replace_audio;
pub mod rewrap;
pub mod silence_remove;
pub mod split;
pub mod subtitling;
pub mod video_filters;

use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{Emitter, Window};

use crate::{parse_out_time_ms, truncate_stderr, JobProgress};
use rate_limiter::RateLimiter;

/// Default progress-emission cadence for `run_ffmpeg`. ffmpeg-emitted
/// `out_time_ms` lines arrive per encoded frame (~60 Hz at 60fps); the
/// limiter throttles UI wakeups to roughly 10 Hz with a 0.5 % minimum
/// percent delta. First emit always passes so the UI sees a 0 % tick.
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(100);
const PROGRESS_MIN_DELTA: f32 = 0.5;

// ── Shared types ───────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct StreamInfo {
    pub index: u32,
    pub stream_type: String,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    // video
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<String>,
    // audio
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

// ── ffprobe helpers ────────────────────────────────────────────────────────────

/// Run ffprobe on `path` and return parsed JSON.
pub(crate) fn run_ffprobe(path: &str) -> Result<serde_json::Value, String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .map_err(|e| format!("ffprobe not found: {e}"))?;
    serde_json::from_slice(&out.stdout).map_err(|e| format!("ffprobe JSON parse error: {e}"))
}

/// Parse stream list from a ffprobe JSON object.
pub(crate) fn parse_streams(json: &serde_json::Value) -> Vec<StreamInfo> {
    let Some(streams) = json["streams"].as_array() else {
        return vec![];
    };
    streams
        .iter()
        .map(|s| {
            let codec_type = s["codec_type"].as_str().unwrap_or("data").to_string();
            let tags = &s["tags"];
            let language = tags["language"]
                .as_str()
                .filter(|l| *l != "und")
                .map(|l| l.to_string());
            let title = tags["title"].as_str().map(|t| t.to_string());

            let width = s["width"].as_u64().map(|v| v as u32);
            let height = s["height"].as_u64().map(|v| v as u32);
            let fps = s["r_frame_rate"]
                .as_str()
                .filter(|f| *f != "0/0")
                .map(|f| f.to_string());

            let sample_rate = s["sample_rate"]
                .as_str()
                .and_then(|r| r.parse::<u32>().ok());
            let channels = s["channels"].as_u64().map(|v| v as u32);

            StreamInfo {
                index: s["index"].as_u64().unwrap_or(0) as u32,
                stream_type: codec_type,
                codec: s["codec_name"].as_str().unwrap_or("unknown").to_string(),
                language,
                title,
                width,
                height,
                fps,
                sample_rate,
                channels,
            }
        })
        .collect()
}

/// Compute a clamped completion percent for a `job-progress` event.
///
/// `duration == None` (unknown total) → 0.0; known duration → percent
/// capped at 99.0 so the in-flight progress loop never reports completion
/// — the terminal 100 % flips to a `job-done` event instead.
fn clamped_percent(elapsed: f64, duration: Option<f64>) -> f32 {
    match duration {
        Some(dur) if dur > 0.0 => ((elapsed / dur) * 100.0).min(99.0) as f32,
        _ => 0.0,
    }
}

/// Return duration in seconds from ffprobe JSON.
pub(crate) fn duration_from_probe(json: &serde_json::Value) -> Option<f64> {
    json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
}

/// Parse wall-clock duration from the "Duration: HH:MM:SS.mmm" line that
/// FFmpeg prints for the input file even under `-hide_banner -nostats`.
/// Returns `None` if the line is absent or value is "N/A" (streaming inputs).
pub(crate) fn parse_duration_from_ffmpeg_stderr(stderr: &str) -> Option<f64> {
    let line = stderr
        .lines()
        .find(|l| l.trim_start().starts_with("Duration:"))?;
    let after = line.trim_start().strip_prefix("Duration:")?.trim();
    let ts = after.split(',').next()?.trim();
    if ts == "N/A" {
        return None;
    }
    let mut parts = ts.splitn(3, ':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

// ── FFmpeg runner ──────────────────────────────────────────────────────────────

/// Kill the child registered at `job_id` if `cancelled` is set.
///
/// Extracted so the TOCTOU-closing re-check in `run_ffmpeg` is unit-testable
/// without spinning up ffmpeg. Returns `true` iff a kill was issued.
pub(crate) fn kill_if_cancelled(
    processes: &Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: &Arc<AtomicBool>,
) -> bool {
    if !cancelled.load(Ordering::SeqCst) {
        return false;
    }
    let mut map = processes.lock().expect("processes mutex poisoned");
    if let Some(child) = map.get_mut(job_id) {
        let _ = child.kill();
        return true;
    }
    false
}

/// Spawn FFmpeg with `args`, track progress events, and wait for completion.
/// Uses the same cancellation / process-map patterns as the convert module.
pub(crate) fn run_ffmpeg(
    window: &Window,
    job_id: &str,
    args: &[String],
    duration: Option<f64>,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock().expect("processes mutex poisoned");
        map.insert(job_id.to_string(), child);
    }

    // Close the cancel TOCTOU window: `cancel_job` sets the flag and tries to
    // kill via the processes map, but the worker can only register the child
    // *after* `Command::spawn`. If the user cancelled during that gap, the
    // map lookup in `cancel_job` returned None — no kill issued — and the
    // child would otherwise run to completion. Re-check the flag now that
    // the child is registered and kill proactively; the progress loop, the
    // stderr drain, and the post-wait branch all still unwind normally.
    kill_if_cancelled(&processes, job_id, &cancelled);

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
        let mut limiter = RateLimiter::new(PROGRESS_MIN_INTERVAL, PROGRESS_MIN_DELTA);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(elapsed) = parse_out_time_ms(&line) {
                let percent = clamped_percent(elapsed, duration);
                if limiter.should_emit(Instant::now(), percent) {
                    let _ = window.emit(
                        "job-progress",
                        JobProgress {
                            job_id: job_id.to_string(),
                            percent,
                            message: format!("{:.0}s elapsed", elapsed),
                        },
                    );
                }
            }
        }
    }

    let error_output = stderr_thread.join().unwrap_or_default();

    let child_opt = {
        let mut map = processes.lock().expect("processes mutex poisoned");
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
        Ok(())
    } else {
        Err(if error_output.trim().is_empty() {
            "FFmpeg operation failed".to_string()
        } else {
            truncate_stderr(&error_output)
        })
    }
}

/// Write a concat demuxer list to a temp file and return its path.
pub(crate) fn write_temp_concat_list(input_paths: &[String]) -> Result<String, String> {
    let path = std::env::temp_dir()
        .join(format!("fade_concat_{}.txt", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();

    let mut content = String::new();
    for p in input_paths {
        content.push_str(&format!("file '{}'\n", p.replace('\'', "'\\''")));
    }
    std::fs::write(&path, &content).map_err(|e| format!("write concat list: {e}"))?;
    Ok(path)
}

/// Container extension → set of codec names that are incompatible with it.
/// Returns an error string if `codec` cannot be stored in `container_ext`.
pub(crate) fn check_codec_container_compat(
    codec: &str,
    stream_type: &str,
    container_ext: &str,
) -> Option<String> {
    let ext = container_ext.to_lowercase();
    let codec_lc = codec.to_lowercase();

    if stream_type == "subtitle" {
        match ext.as_str() {
            "mp4" | "m4v" if codec_lc != "mov_text" && codec_lc != "tx3g" => {
                return Some(format!(
                    "Subtitle codec '{}' cannot be stored in MP4 — use MKV or convert subtitles to mov_text",
                    codec
                ));
            }
            "avi" => {
                return Some("AVI does not support subtitle streams — use MKV".to_string());
            }
            _ => {}
        }
    }

    if stream_type == "audio" {
        if let "mp4" | "m4v" | "mov" = ext.as_str() {
            // PCM formats are not valid in MP4
            if codec_lc.starts_with("pcm_") {
                return Some(format!(
                    "Audio codec '{}' is not compatible with MP4 — transcode to AAC or use MKV",
                    codec
                ));
            }
        }
    }

    None
}

/// Get file extension (lowercase) from a path string.
pub(crate) fn ext_of(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn clamped_percent_returns_zero_when_duration_unknown() {
        assert_eq!(clamped_percent(42.0, None), 0.0);
    }

    #[test]
    fn clamped_percent_returns_zero_when_duration_zero() {
        assert_eq!(clamped_percent(42.0, Some(0.0)), 0.0);
    }

    #[test]
    fn clamped_percent_caps_at_ninety_nine() {
        // elapsed > duration must still report 99, never 100.
        assert_eq!(clamped_percent(120.0, Some(60.0)), 99.0);
        assert_eq!(clamped_percent(60.0, Some(60.0)), 99.0);
    }

    #[test]
    fn clamped_percent_scales_proportionally_below_cap() {
        assert_eq!(clamped_percent(30.0, Some(60.0)), 50.0);
        assert_eq!(clamped_percent(0.0, Some(60.0)), 0.0);
    }

    #[test]
    fn progress_rate_limiter_constants_match_canonical_use() {
        // Guard the wired-in cadence: B7 RateLimiter + canonical run_ffmpeg
        // must agree on the throttle. If a future change loosens these,
        // re-baseline this test deliberately.
        assert_eq!(PROGRESS_MIN_INTERVAL, Duration::from_millis(100));
        assert!((PROGRESS_MIN_DELTA - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn kill_if_cancelled_returns_false_when_flag_unset() {
        let processes: Arc<Mutex<HashMap<String, Child>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let cancelled = Arc::new(AtomicBool::new(false));
        // No child registered; flag unset — must be a no-op (returns false).
        assert!(!kill_if_cancelled(&processes, "missing", &cancelled));
    }

    #[test]
    fn kill_if_cancelled_returns_false_when_no_child_registered() {
        let processes: Arc<Mutex<HashMap<String, Child>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let cancelled = Arc::new(AtomicBool::new(true));
        // Flag set but nothing to kill — returns false, does not panic.
        assert!(!kill_if_cancelled(&processes, "missing", &cancelled));
    }

    #[test]
    fn kill_if_cancelled_kills_registered_child_when_flag_set() {
        // Regression for F-02 cancel TOCTOU. Simulate the window where
        // `cancel_job` has already stored `cancelled = true` but the worker
        // has only just inserted its child into the processes map. The
        // re-check must kill the child so it cannot run to completion.
        #[cfg(unix)]
        let mut cmd = Command::new("sleep");
        #[cfg(unix)]
        cmd.arg("30");
        #[cfg(windows)]
        let mut cmd = Command::new("cmd");
        #[cfg(windows)]
        cmd.args(["/C", "ping", "-n", "60", "127.0.0.1"]);

        let child = cmd
            .spawn()
            .expect("spawn a long-running child for the test");

        let processes: Arc<Mutex<HashMap<String, Child>>> =
            Arc::new(Mutex::new(HashMap::new()));
        processes
            .lock()
            .unwrap()
            .insert("job-toctou".to_string(), child);

        let cancelled = Arc::new(AtomicBool::new(true));
        assert!(kill_if_cancelled(&processes, "job-toctou", &cancelled));

        // After the kill, the caller is responsible for removing the
        // child from the map and waiting. Drain here so the test doesn't
        // leak a zombie on Unix.
        let mut child = processes.lock().unwrap().remove("job-toctou").unwrap();
        let status = child.wait().expect("child wait after kill");
        assert!(!status.success(), "killed child must not report success");
    }

    #[test]
    fn duration_from_probe_extracts_seconds() {
        let json = serde_json::json!({ "format": { "duration": "93.5" } });
        assert_eq!(duration_from_probe(&json), Some(93.5));
    }

    #[test]
    fn duration_from_probe_returns_none_on_missing_field() {
        let json = serde_json::json!({ "format": {} });
        assert_eq!(duration_from_probe(&json), None);
    }

    #[test]
    fn parse_duration_from_ffmpeg_stderr_basic() {
        let stderr = "  Duration: 00:01:30.50, start: 0.000000, bitrate: 1234 kb/s";
        assert_eq!(parse_duration_from_ffmpeg_stderr(stderr), Some(90.5));
    }

    #[test]
    fn parse_duration_from_ffmpeg_stderr_hours() {
        let stderr = "Input #0, matroska\n  Duration: 01:00:00.00, start: 0.0\n";
        assert_eq!(parse_duration_from_ffmpeg_stderr(stderr), Some(3600.0));
    }

    #[test]
    fn parse_duration_from_ffmpeg_stderr_na_returns_none() {
        let stderr = "  Duration: N/A, start: 0.000000, bitrate: N/A";
        assert_eq!(parse_duration_from_ffmpeg_stderr(stderr), None);
    }

    #[test]
    fn parse_duration_from_ffmpeg_stderr_absent_returns_none() {
        let stderr = "ffmpeg version 6.0\nSome other output\n";
        assert_eq!(parse_duration_from_ffmpeg_stderr(stderr), None);
    }

    #[test]
    fn rate_limiter_dampens_high_frequency_progress_stream() {
        // Simulate a 60 fps ffmpeg progress stream with monotonic percent
        // ticks of 0.1% per frame. Without throttling: 60 emits per second.
        // With the canonical 100 ms / 0.5% gates: at most ~6 per second
        // (interval-bound) and only when the 0.5% delta is also crossed.
        let mut limiter = RateLimiter::new(PROGRESS_MIN_INTERVAL, PROGRESS_MIN_DELTA);
        let t0 = std::time::Instant::now();
        let mut accepted = 0usize;
        for frame in 0..60u32 {
            let now = t0 + Duration::from_millis(frame as u64 * 16); // ~16ms per frame
            let percent = (frame as f32) * 0.1; // 0.0 .. 5.9
            if limiter.should_emit(now, percent) {
                accepted += 1;
            }
        }
        // Expected: 1st frame always accepts, then ~every 5th frame
        // crosses the 0.5%-delta gate (5 × 0.1 = 0.5) and the ~80 ms
        // interval since the prior accept (5 × 16 = 80ms). The 100 ms
        // interval gate dominates → roughly 1 accept per 7 frames.
        assert!(
            accepted <= 12,
            "expected heavy dampening, got {accepted} of 60"
        );
        assert!(accepted >= 1, "must accept at least the initial frame");
    }
}
