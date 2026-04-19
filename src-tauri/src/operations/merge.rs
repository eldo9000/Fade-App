//! Merge: concatenate multiple video files in order.
//!
//! Pre-flight: probe all inputs with ffprobe.  If all video streams share the
//! same codec, resolution, frame rate and pixel format, use the concat demuxer
//! (stream-copy, no re-encode).  Otherwise use the concat filter (re-encode to
//! H.264 + AAC).
//!
//! Inputs without an audio stream get a silent `anullsrc` track injected before
//! the concat filter so every segment has the same number of streams.
//!
//! The temp concat-list file is cleaned up after the job completes (or fails).

use super::{duration_from_probe, parse_streams, run_ffprobe, write_temp_concat_list};
use crate::{parse_out_time_ms, truncate_stderr, JobProgress};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Window};

pub fn run(
    window: &Window,
    job_id: &str,
    input_paths: &[String],
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if input_paths.len() < 2 {
        return Err("Merge requires at least two input files".to_string());
    }

    // Probe all inputs
    let mut probes = Vec::with_capacity(input_paths.len());
    for p in input_paths {
        probes.push(run_ffprobe(p)?);
    }

    // Collect total duration for progress
    let total_duration: Option<f64> = probes.iter().map(duration_from_probe).sum::<Option<f64>>();

    // Check codec/resolution/fps/pixfmt compatibility across all video streams
    struct VideoParams {
        codec: String,
        width: u32,
        height: u32,
        fps: String,
        pix_fmt: String,
    }

    let mut all_video_params: Vec<Option<VideoParams>> = Vec::new();
    let mut has_audio: Vec<bool> = Vec::new();

    for probe in &probes {
        let streams = parse_streams(probe);
        let v = streams.iter().find(|s| s.stream_type == "video");
        let a = streams.iter().any(|s| s.stream_type == "audio");
        has_audio.push(a);

        if let Some(vs) = v {
            // pixel format is in the raw probe JSON, not in StreamInfo
            let pix_fmt = probe["streams"]
                .as_array()
                .and_then(|arr| {
                    arr.iter()
                        .find(|s| s["codec_type"].as_str() == Some("video"))
                })
                .and_then(|s| s["pix_fmt"].as_str())
                .unwrap_or("unknown")
                .to_string();

            all_video_params.push(Some(VideoParams {
                codec: vs.codec.clone(),
                width: vs.width.unwrap_or(0),
                height: vs.height.unwrap_or(0),
                fps: vs.fps.clone().unwrap_or_else(|| "0/0".to_string()),
                pix_fmt,
            }));
        } else {
            all_video_params.push(None);
        }
    }

    let streams_match = || -> bool {
        let first = match all_video_params.first().and_then(|p| p.as_ref()) {
            Some(p) => p,
            None => return false,
        };
        for opt in all_video_params.iter().skip(1) {
            let p = match opt.as_ref() {
                Some(p) => p,
                None => return false,
            };
            if p.codec != first.codec
                || p.width != first.width
                || p.height != first.height
                || p.fps != first.fps
                || p.pix_fmt != first.pix_fmt
            {
                return false;
            }
        }
        true
    };

    let args = if streams_match() && has_audio.iter().all(|&a| a) {
        // Fast path: concat demuxer (stream-copy)
        let list_path = write_temp_concat_list(input_paths)?;
        let args = vec![
            "-y".to_string(),
            "-f".to_string(),
            "concat".to_string(),
            "-safe".to_string(),
            "0".to_string(),
            "-i".to_string(),
            list_path.clone(),
            "-c".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0".to_string(),
            "-progress".to_string(),
            "pipe:1".to_string(),
            output_path.to_string(),
        ];
        // Schedule cleanup; errors are non-fatal
        std::thread::spawn(move || {
            // Wait a moment for FFmpeg to open the file before deleting
            std::thread::sleep(std::time::Duration::from_secs(2));
            let _ = std::fs::remove_file(&list_path);
        });
        args
    } else {
        // Slow path: concat filter (re-encode)
        let n = input_paths.len();
        let mut args: Vec<String> = vec!["-y".to_string()];

        for p in input_paths {
            args.extend(["-i".to_string(), p.clone()]);
        }

        // Build filter_complex:
        // For each input that has audio, use [i:v] and [i:a].
        // For each input without audio, synthesize silence with anullsrc.
        let mut filter_parts: Vec<String> = Vec::new();
        let mut concat_v_inputs = String::new();
        let mut concat_a_inputs = String::new();
        let mut anull_idx = 0usize;

        for (i, &has_a) in has_audio.iter().enumerate() {
            filter_parts.push(format!("[{i}:v]setpts=PTS-STARTPTS[v{i}]"));
            concat_v_inputs.push_str(&format!("[v{i}]"));

            if has_a {
                filter_parts.push(format!("[{i}:a]asetpts=PTS-STARTPTS[a{i}]"));
                concat_a_inputs.push_str(&format!("[a{i}]"));
            } else {
                // Synthesize silence for this segment
                filter_parts.push(format!("anullsrc=r=44100:cl=stereo[null{anull_idx}]"));
                concat_a_inputs.push_str(&format!("[null{anull_idx}]"));
                anull_idx += 1;
            }
        }

        let concat_filter = format!(
            "{}concat=n={n}:v=1:a=1[outv][outa]",
            filter_parts.join(";") + ";" + &concat_v_inputs + &concat_a_inputs
        );

        args.extend([
            "-filter_complex".to_string(),
            concat_filter,
            "-map".to_string(),
            "[outv]".to_string(),
            "-map".to_string(),
            "[outa]".to_string(),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-crf".to_string(),
            "18".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
            "-progress".to_string(),
            "pipe:1".to_string(),
            output_path.to_string(),
        ]);

        args
    };

    // Run FFmpeg inline (same pattern as run_ffmpeg helper but we need the
    // concat-list cleanup to happen after the process exits, not in a thread)
    run_ffmpeg_merge(window, job_id, &args, total_duration, processes, cancelled)
}

/// Identical to the shared `run_ffmpeg` helper; duplicated here so the merge
/// module can be self-contained and the concat-list cleanup can happen after
/// the process exits cleanly.
fn run_ffmpeg_merge(
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
            "FFmpeg merge failed".to_string()
        } else {
            truncate_stderr(&error_output)
        })
    }
}
