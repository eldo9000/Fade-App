//! Subtitle conversion — shells out to ffmpeg. ffmpeg handles
//! srt ↔ vtt ↔ ass ↔ ssa natively by extension and can write ttml.
//! SBV (YouTube's sub format) is not recognised by ffmpeg in either
//! direction, so we hand-roll SRT↔SBV and bridge through a temp SRT
//! when the other side is a non-SRT format.
//!
//! Subtitle files are tiny and instant — no progress parsing, just
//! 0% then 100%.

use crate::{truncate_stderr, ConvertOptions, JobProgress};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Window};

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Hand-rolled SRT → SBV. SBV is YouTube's subtitle format: identical to
/// SRT minus the sequence numbers, and with `,` instead of `-->` between
/// timestamps (and `.` instead of `,` in the decimal).
///
/// Example SRT cue:
///   1
///   00:00:01,200 --> 00:00:03,400
///   Hello world
///
/// Same cue in SBV:
///   0:00:01.200,0:00:03.400
///   Hello world
fn srt_to_sbv(srt: &str) -> String {
    let mut out = String::new();
    for line in srt.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        // Sequence number line — all digits. Skip it.
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // Timestamp line.
        if let Some((a, b)) = trimmed.split_once("-->") {
            let a = a.trim().replace(',', ".");
            let b = b.trim().replace(',', ".");
            // SBV strips the leading zero on the hour digit ("0:00:01.200").
            let a = a.strip_prefix('0').unwrap_or(&a).to_string();
            let b = b.strip_prefix('0').unwrap_or(&b).to_string();
            out.push_str(&format!("{a},{b}\n"));
            continue;
        }
        // Text line — copy through.
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Hand-rolled SBV → SRT. Inverse of the above; we invent sequence numbers.
fn sbv_to_srt(sbv: &str) -> String {
    let mut out = String::new();
    let mut seq = 1u32;
    // Split into blank-line-separated blocks.
    let mut buf: Vec<&str> = Vec::new();
    let flush = |buf: &mut Vec<&str>, seq: &mut u32, out: &mut String| {
        if buf.is_empty() {
            return;
        }
        // First line is the timestamp, rest is body.
        let ts = buf[0].trim();
        if let Some((a, b)) = ts.split_once(',') {
            let a = a.trim().replace('.', ",");
            let b = b.trim().replace('.', ",");
            // Normalise to HH:MM:SS,mmm — pad single-digit hours.
            let pad = |s: String| {
                if let Some(rest) = s.strip_prefix(|c: char| c.is_ascii_digit()) {
                    if rest.starts_with(':') && s.len() < 12 {
                        return format!("0{s}");
                    }
                }
                s
            };
            let a = pad(a);
            let b = pad(b);
            out.push_str(&seq.to_string());
            out.push('\n');
            out.push_str(&format!("{a} --> {b}\n"));
            for body in &buf[1..] {
                out.push_str(body);
                out.push('\n');
            }
            out.push('\n');
            *seq += 1;
        }
        buf.clear();
    };
    for line in sbv.lines() {
        if line.trim().is_empty() {
            flush(&mut buf, &mut seq, &mut out);
        } else {
            buf.push(line);
        }
    }
    flush(&mut buf, &mut seq, &mut out);
    out
}

/// Spawn ffmpeg with the standard `-y -i INPUT OUTPUT` args and block until
/// completion, streaming stderr for error reporting and honouring the
/// cancellation flag. Shared by the ffmpeg-handled paths.
fn run_ffmpeg(
    job_id: &str,
    input: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut child = Command::new("ffmpeg")
        .args(["-y", "-i", input, output])
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
        Err(if stderr_content.trim().is_empty() {
            "ffmpeg subtitle conversion failed".to_string()
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
    _opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting subtitle…".to_string(),
        },
    );

    let in_ext = ext_of(input);
    let out_ext = ext_of(output);

    let result = match (in_ext.as_str(), out_ext.as_str()) {
        // Both sides ffmpeg-native — one-shot.
        ("srt" | "vtt" | "ass" | "ssa", "srt" | "vtt" | "ass" | "ssa" | "ttml") => run_ffmpeg(
            job_id,
            input,
            output,
            Arc::clone(&processes),
            Arc::clone(&cancelled),
        ),

        // SBV → SBV — trivial copy, but keep behaviour consistent.
        ("sbv", "sbv") => fs::copy(input, output)
            .map(|_| ())
            .map_err(|e| e.to_string()),

        // SBV → SRT — pure hand-roll.
        ("sbv", "srt") => {
            let sbv = fs::read_to_string(input).map_err(|e| e.to_string())?;
            let srt = sbv_to_srt(&sbv);
            fs::write(output, srt).map_err(|e| e.to_string())
        }

        // SRT → SBV — pure hand-roll.
        ("srt", "sbv") => {
            let srt = fs::read_to_string(input).map_err(|e| e.to_string())?;
            let sbv = srt_to_sbv(&srt);
            fs::write(output, sbv).map_err(|e| e.to_string())
        }

        // SBV → other (vtt/ass/ssa/ttml) — bridge through a temp SRT.
        ("sbv", _) => {
            let sbv = fs::read_to_string(input).map_err(|e| e.to_string())?;
            let srt = sbv_to_srt(&sbv);
            let tmp = std::env::temp_dir().join(format!("fade-{job_id}.srt"));
            fs::write(&tmp, srt).map_err(|e| e.to_string())?;
            let res = run_ffmpeg(
                job_id,
                &tmp.to_string_lossy(),
                output,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            );
            let _ = fs::remove_file(&tmp);
            res
        }

        // other → SBV — bridge through a temp SRT.
        (_, "sbv") => {
            let tmp = std::env::temp_dir().join(format!("fade-{job_id}.srt"));
            let res = run_ffmpeg(
                job_id,
                input,
                &tmp.to_string_lossy(),
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            );
            if res.is_ok() {
                let srt = fs::read_to_string(&tmp).map_err(|e| e.to_string())?;
                let sbv = srt_to_sbv(&srt);
                let _ = fs::remove_file(&tmp);
                fs::write(output, sbv).map_err(|e| e.to_string())
            } else {
                let _ = fs::remove_file(&tmp);
                res
            }
        }

        // Fallback — let ffmpeg try. Covers ttml/vtt/ass/ssa on either side.
        _ => run_ffmpeg(
            job_id,
            input,
            output,
            Arc::clone(&processes),
            Arc::clone(&cancelled),
        ),
    };

    result?;

    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 100.0,
            message: "Done".to_string(),
        },
    );
    Ok(())
}
