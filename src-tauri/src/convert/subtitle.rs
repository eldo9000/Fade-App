//! Subtitle conversion — shells out to ffmpeg. ffmpeg handles
//! srt ↔ vtt ↔ ass ↔ ssa natively by extension and can write ttml.
//! SBV (YouTube's sub format) is not recognised by ffmpeg in either
//! direction, so we hand-roll SRT↔SBV and bridge through a temp SRT
//! when the other side is a non-SRT format.
//!
//! Subtitle files are tiny and instant — no progress parsing, just
//! 0% then 100%.

use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::operations::run_ffmpeg as op_run_ffmpeg;
use crate::ConvertOptions;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

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
pub fn srt_to_sbv(srt: &str) -> String {
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
pub fn sbv_to_srt(sbv: &str) -> String {
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

/// Trait abstracting the ffmpeg shell-out so `convert()` does not need a
/// `&Window`. The Tauri wrapper supplies a real implementation backed by
/// `operations::run_ffmpeg`; tests that exercise only pure-Rust paths can
/// pass a no-op implementation that errors if invoked.
pub trait FfmpegRunner {
    fn run(
        &mut self,
        input: &str,
        output: &str,
        processes: Arc<Mutex<HashMap<String, Child>>>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String>;
}

/// FfmpegRunner that always errors — for tests that don't exercise
/// ffmpeg-backed paths.
pub struct UnavailableFfmpeg;
impl FfmpegRunner for UnavailableFfmpeg {
    fn run(
        &mut self,
        _input: &str,
        _output: &str,
        _processes: Arc<Mutex<HashMap<String, Child>>>,
        _cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        Err("ffmpeg runner not available in this context".to_string())
    }
}

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// `ffmpeg_runner` is invoked for any conversion path that needs ffmpeg
/// (srt/vtt/ass/ssa/ttml interchange). Pure-Rust paths (srt ↔ sbv) never
/// touch it.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    output: &str,
    _opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    ffmpeg_runner: &mut dyn FfmpegRunner,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
    _job_id: &str,
) -> Result<(), String> {
    progress(ProgressEvent::Started);

    let in_ext = ext_of(input);
    let out_ext = ext_of(output);

    let result = match (in_ext.as_str(), out_ext.as_str()) {
        // Both sides ffmpeg-native — one-shot.
        ("srt" | "vtt" | "ass" | "ssa", "srt" | "vtt" | "ass" | "ssa" | "ttml") => ffmpeg_runner
            .run(
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
            #[cfg(unix)]
            let sandbox = {
                use std::os::unix::fs::PermissionsExt;
                tempfile::Builder::new()
                    .permissions(std::fs::Permissions::from_mode(0o700))
                    .tempdir_in(std::env::temp_dir())
                    .map_err(|e| format!("failed to create temp sandbox: {e}"))?
            };
            #[cfg(not(unix))]
            let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
                .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
            let tmp = sandbox.path().join("temp.srt");
            fs::write(&tmp, srt).map_err(|e| e.to_string())?;
            ffmpeg_runner.run(
                &tmp.to_string_lossy(),
                output,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            )
            // sandbox drops here → auto-cleanup
        }

        // other → SBV — bridge through a temp SRT.
        (_, "sbv") => {
            #[cfg(unix)]
            let sandbox = {
                use std::os::unix::fs::PermissionsExt;
                tempfile::Builder::new()
                    .permissions(std::fs::Permissions::from_mode(0o700))
                    .tempdir_in(std::env::temp_dir())
                    .map_err(|e| format!("failed to create temp sandbox: {e}"))?
            };
            #[cfg(not(unix))]
            let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
                .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
            let tmp = sandbox.path().join("temp.srt");
            let res = ffmpeg_runner.run(
                input,
                &tmp.to_string_lossy(),
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            );
            if res.is_ok() {
                let srt = fs::read_to_string(&tmp).map_err(|e| e.to_string())?;
                let sbv = srt_to_sbv(&srt);
                fs::write(output, sbv).map_err(|e| e.to_string())
            } else {
                res
            }
            // sandbox drops here → auto-cleanup
        }

        // Fallback — let ffmpeg try. Covers ttml/vtt/ass/ssa on either side.
        _ => ffmpeg_runner.run(
            input,
            output,
            Arc::clone(&processes),
            Arc::clone(&cancelled),
        ),
    };

    result?;

    progress(ProgressEvent::Done);
    Ok(())
}

/// FfmpegRunner backed by the canonical `operations::run_ffmpeg`. Lives
/// in `run()` only — anything that touches a `&Window` stays inside the
/// wrapper.
struct WindowFfmpegRunner<'a> {
    window: &'a Window,
    job_id: String,
}

impl FfmpegRunner for WindowFfmpegRunner<'_> {
    fn run(
        &mut self,
        input: &str,
        output: &str,
        processes: Arc<Mutex<HashMap<String, Child>>>,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let args = vec![
            "-y".to_string(),
            "-i".to_string(),
            input.to_string(),
            output.to_string(),
        ];
        op_run_ffmpeg(self.window, &self.job_id, &args, None, processes, cancelled)
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
) -> Result<(), String> {
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting subtitle…");
    let mut runner = WindowFfmpegRunner {
        window,
        job_id: job_id.to_string(),
    };
    convert(
        input,
        output,
        opts,
        &mut emit,
        &mut runner,
        processes,
        cancelled,
        job_id,
    )
}
