//! Mechanical video/audio operations.
//!
//! Each submodule implements one operation that drives FFmpeg as a subprocess,
//! emitting the same `job-progress` / `job-done` / `job-error` / `job-cancelled`
//! events used by the existing `convert_file` command.

pub mod audio_offset;
pub mod conform;
pub mod cut;
pub mod extract;
pub mod merge;
pub mod replace_audio;
pub mod rewrap;
pub mod split;

use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Window};

use crate::{parse_out_time_ms, truncate_stderr, JobProgress};

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

/// Return duration in seconds from ffprobe JSON.
pub(crate) fn duration_from_probe(json: &serde_json::Value) -> Option<f64> {
    json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
}

// ── FFmpeg runner ──────────────────────────────────────────────────────────────

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
        let mut map = processes.lock().unwrap();
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
                let percent = if let Some(dur) = duration {
                    ((elapsed / dur) * 100.0).min(99.0) as f32
                } else {
                    0.0
                };
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

    let error_output = stderr_thread.join().unwrap_or_default();

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
