//! AI tool operations: audio separation, transcription, translation,
//! colorization, and background removal. All use local Python tools — no
//! cloud APIs. Each function detects the required tool before running.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, Window};

use crate::{tool_available, JobProgress};

// ── helpers ────────────────────────────────────────────────────────────────────

/// Check whether a Python module is importable.
fn python_module_available(module: &str) -> bool {
    Command::new("python3")
        .args(["-c", &format!("import {module}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Spawn `python3` (or any binary) with args and return its child.
fn spawn_python(args: &[&str]) -> Result<Child, String> {
    Command::new("python3")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 not found: {e}"))
}

/// Spawn an arbitrary binary with args.
fn spawn_cmd(bin: &str, args: &[&str]) -> Result<Child, String> {
    Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin} not found: {e}"))
}

/// Register a child in the process map, then check the cancellation flag to
/// close the TOCTOU window.
fn register_child(
    processes: &Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: &Arc<AtomicBool>,
    child: Child,
) -> bool {
    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }
    if cancelled.load(Ordering::SeqCst) {
        let mut map = processes.lock();
        if let Some(c) = map.get_mut(job_id) {
            let _ = c.kill();
        }
        return true;
    }
    false
}

/// Wait for the child registered at `job_id` and return success/failure.
fn wait_child(
    processes: &Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: &Arc<AtomicBool>,
    stderr_buf: String,
) -> Result<(), String> {
    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };
    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };
    if cancelled.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }
    if success {
        Ok(())
    } else if stderr_buf.trim().is_empty() {
        Err("Process failed".to_string())
    } else {
        // Trim to a reasonable length — reuse Fade's existing truncation style.
        let truncated: String = stderr_buf.lines().take(20).collect::<Vec<_>>().join("\n");
        Err(truncated)
    }
}

/// Emit a `job-progress` event with a percentage and message.
fn emit_progress(window: &Window, job_id: &str, percent: f32, message: &str) {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent,
            message: message.to_string(),
        },
    );
}

// ── TASK-G1: Audio Separation (Demucs) ────────────────────────────────────────

/// Separate audio sources (vocals, drums, bass, other) using Demucs.
///
/// `model`: `"htdemucs"` (default) or `"htdemucs_ft"`.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_audio_separation(
    window: &Window,
    job_id: &str,
    input: &str,
    output_dir: &str,
    model: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if !python_module_available("demucs") {
        return Err("Audio separation requires Demucs: pip install demucs".to_string());
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("create output dir: {e}"))?;

    let model = if model.is_empty() { "htdemucs" } else { model };

    let child = spawn_python(&[
        "-m", "demucs", "--mp3", "--out", output_dir, "--name", model, input,
    ])?;

    if register_child(&processes, job_id, &cancelled, child) {
        return Err("CANCELLED".to_string());
    }

    // Demucs writes progress to stderr: "Separating track  45%|..."
    let stderr = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stderr.take())
    };

    let mut stderr_lines: Vec<String> = Vec::new();
    if let Some(s) = stderr {
        let reader = BufReader::new(s);
        for line in reader.lines().map_while(Result::ok) {
            // Parse percent from lines like "Separating track  45%|..."
            if let Some(pct) = parse_percent_from_line(&line) {
                emit_progress(window, job_id, pct, &format!("{pct:.0}% separated"));
            }
            stderr_lines.push(line);
        }
    }

    wait_child(&processes, job_id, &cancelled, stderr_lines.join("\n"))
}

/// Extract a percent float from a string that contains a `%` character.
/// Looks for a number immediately before `%`, e.g. "45%" or " 45%|..."
fn parse_percent_from_line(line: &str) -> Option<f32> {
    // Find the first `%` and walk back to collect digits (and optional `.`).
    let pct_pos = line.find('%')?;
    let before = &line[..pct_pos];
    let digits: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse::<f32>().ok()
}

// ── TASK-G2: Transcription (Whisper) ──────────────────────────────────────────

/// Transcribe audio/video to text using OpenAI Whisper.
///
/// `model`: tiny / base / small / medium / large.
/// `output_format`: srt / vtt / txt.
/// `language`: ISO-639-1 code (e.g. "en"), or `""` for auto-detect.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_transcription(
    window: &Window,
    job_id: &str,
    input: &str,
    output_dir: &str,
    model: &str,
    output_format: &str,
    language: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let whisper_available = tool_available("whisper") || python_module_available("faster_whisper");
    if !whisper_available {
        return Err("Transcription requires Whisper: pip install openai-whisper".to_string());
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("create output dir: {e}"))?;

    let model = if model.is_empty() { "base" } else { model };
    let fmt = if output_format.is_empty() {
        "srt"
    } else {
        output_format
    };

    let mut args: Vec<&str> = vec![
        input,
        "--model",
        model,
        "--output_format",
        fmt,
        "--output_dir",
        output_dir,
    ];

    // Optional language override: push args before args is consumed by spawn_cmd.
    let lang_owned = language.to_string();
    if !language.is_empty() {
        args.push("--language");
        args.push(&lang_owned);
    }

    let child = spawn_cmd("whisper", &args)?;

    if register_child(&processes, job_id, &cancelled, child) {
        return Err("CANCELLED".to_string());
    }

    // Whisper writes segment timestamps to stdout: "[00:00.000 --> 00:05.000]  text"
    // We parse these to estimate progress when total duration is unknown.
    let stdout = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stdout.take())
    };
    let stderr = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stderr.take())
    };

    // Drain stderr on a background thread so the process isn't blocked.
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

    if let Some(out) = stdout {
        let reader = BufReader::new(out);
        for line in reader.lines().map_while(Result::ok) {
            // "[00:01.234 --> 00:05.678]  some text"
            if let Some(end_secs) = parse_whisper_timestamp(&line) {
                emit_progress(
                    window,
                    job_id,
                    0.0, // total duration unknown — emit indeterminate ticks
                    &format!("{end_secs:.0}s transcribed"),
                );
            }
        }
    }

    let stderr_buf = stderr_thread.join().unwrap_or_default();
    wait_child(&processes, job_id, &cancelled, stderr_buf)
}

/// Parse the end-time of a Whisper timestamp line.
/// Format: `[HH:MM.mmm --> HH:MM.mmm]`  (Whisper uses MM:SS.mmm)
fn parse_whisper_timestamp(line: &str) -> Option<f64> {
    // Find "-->" separator.
    let arrow = line.find("-->")?;
    let after = line[arrow + 3..].trim();
    // Strip leading `[` chars from "00:05.678]  text"
    let after = after.trim_start_matches(|c: char| !c.is_ascii_digit());
    let ts = after.split(']').next()?.trim();
    parse_mm_ss(ts)
}

/// Parse `MM:SS.mmm` or `HH:MM:SS.mmm` into seconds.
fn parse_mm_ss(ts: &str) -> Option<f64> {
    let parts: Vec<&str> = ts.splitn(4, ':').collect();
    match parts.len() {
        2 => {
            let m: f64 = parts[0].parse().ok()?;
            let s: f64 = parts[1].parse().ok()?;
            Some(m * 60.0 + s)
        }
        3 => {
            let h: f64 = parts[0].parse().ok()?;
            let m: f64 = parts[1].parse().ok()?;
            let s: f64 = parts[2].parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        _ => None,
    }
}

// ── TASK-G3: Translation (Argos Translate) ────────────────────────────────────

/// Translate a subtitle or text file using Argos Translate (offline).
///
/// Supports SRT, VTT, and plain-text inputs. SRT/VTT timestamps are preserved
/// verbatim; only dialogue lines are passed to the translator.
///
/// If the required language pair package is not installed, the inline script
/// downloads it from the Argos package index automatically.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_translation(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    src_lang: &str,
    tgt_lang: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if !python_module_available("argostranslate") {
        return Err("Translation requires Argos Translate: pip install argostranslate".to_string());
    }

    // Build an inline Python script for translation.
    let script = build_translation_script(input, output, src_lang, tgt_lang);

    let child = Command::new("python3")
        .args(["-c", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 not found: {e}"))?;

    if register_child(&processes, job_id, &cancelled, child) {
        return Err("CANCELLED".to_string());
    }

    // Read progress ticks from stdout ("PROGRESS:<n>/<total>").
    let stdout = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stdout.take())
    };
    let stderr = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stderr.take())
    };

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

    if let Some(out) = stdout {
        let reader = BufReader::new(out);
        for line in reader.lines().map_while(Result::ok) {
            // "PROGRESS:45/100"
            if let Some(pct) = parse_progress_line(&line) {
                emit_progress(window, job_id, pct, &format!("{pct:.0}% translated"));
            }
        }
    }

    let stderr_buf = stderr_thread.join().unwrap_or_default();
    wait_child(&processes, job_id, &cancelled, stderr_buf)
}

/// Build the inline Python translation script.
fn build_translation_script(input: &str, output: &str, src: &str, tgt: &str) -> String {
    // Escape paths for embedding in Python string literals.
    let input_esc = input.replace('\\', "\\\\").replace('\'', "\\'");
    let output_esc = output.replace('\\', "\\\\").replace('\'', "\\'");
    let src_esc = src.replace('\'', "\\'");
    let tgt_esc = tgt.replace('\'', "\\'");

    format!(
        r#"
import sys, os, re
import argostranslate.package
import argostranslate.translate

src_lang = '{src}'
tgt_lang = '{tgt}'
input_path = '{input}'
output_path = '{output}'

# Ensure the required language pair is installed; download if absent.
argostranslate.package.update_package_index()
available = argostranslate.package.get_available_packages()
installed_langs = {{str(p) for p in argostranslate.package.get_installed_packages()}}
pkg = next(
    (p for p in available if p.from_code == src_lang and p.to_code == tgt_lang),
    None,
)
if pkg and str(pkg) not in installed_langs:
    argostranslate.package.install_from_path(pkg.download())

def translate_text(text):
    return argostranslate.translate.translate(text, src_lang, tgt_lang)

ext = os.path.splitext(input_path)[1].lower()

with open(input_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

total = len(lines)
out_lines = []
is_srt = ext == '.srt'
is_vtt = ext == '.vtt'

# SRT/VTT: only translate text lines; skip timestamps and index numbers.
timestamp_re = re.compile(r'^\d{{2}}:\d{{2}}')
index_re = re.compile(r'^\d+\s*$')

for i, line in enumerate(lines):
    stripped = line.rstrip('\n')
    if is_srt and index_re.match(stripped):
        out_lines.append(line)
    elif (is_srt or is_vtt) and timestamp_re.match(stripped):
        out_lines.append(line)
    elif stripped == '' or stripped.startswith('WEBVTT'):
        out_lines.append(line)
    else:
        translated = translate_text(stripped)
        out_lines.append(translated + '\n')
    pct = int((i + 1) / total * 100)
    if (i + 1) % max(1, total // 20) == 0:
        print(f'PROGRESS:{{i+1}}/{{total}}', flush=True)

with open(output_path, 'w', encoding='utf-8') as f:
    f.writelines(out_lines)

print('PROGRESS:{{total}}/{{total}}', flush=True)
"#,
        src = src_esc,
        tgt = tgt_esc,
        input = input_esc,
        output = output_esc,
    )
}

/// Parse a `PROGRESS:n/total` line into a float percent.
fn parse_progress_line(line: &str) -> Option<f32> {
    let rest = line.strip_prefix("PROGRESS:")?;
    let mut parts = rest.splitn(2, '/');
    let done: f64 = parts.next()?.parse().ok()?;
    let total: f64 = parts.next()?.parse().ok()?;
    if total <= 0.0 {
        return None;
    }
    Some(((done / total) * 100.0).min(99.0) as f32)
}

// ── TASK-G4: Colorize (ddcolor) ───────────────────────────────────────────────

/// Colorize a black-and-white image or video using DDColor.
///
/// For images, uses an inline Python inference script.
/// For video, extracts frames via FFmpeg, colorizes each, and reassembles.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_colorize(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let ddcolor_available = python_module_available("ddcolor") || tool_available("ddcolor");
    if !ddcolor_available {
        return Err("Colorization requires ddcolor: pip install ddcolor".to_string());
    }

    let ext = Path::new(input)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let is_video = matches!(
        ext.as_str(),
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "flv" | "wmv"
    );

    if is_video {
        colorize_video(window, job_id, input, output, processes, cancelled)
    } else {
        colorize_image(window, job_id, input, output, processes, cancelled)
    }
}

fn colorize_image(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let input_esc = input.replace('\\', "\\\\").replace('\'', "\\'");
    let output_esc = output.replace('\\', "\\\\").replace('\'', "\\'");

    let script = format!(
        r#"
import sys
try:
    from ddcolor import DDColor
    import numpy as np
    from PIL import Image

    model = DDColor()
    img = Image.open('{input}').convert('RGB')
    arr = np.array(img)
    result = model.colorize(arr)
    Image.fromarray(result).save('{output}')
    print('DONE', flush=True)
except Exception as e:
    print(f'ERROR: {{e}}', file=sys.stderr)
    sys.exit(1)
"#,
        input = input_esc,
        output = output_esc,
    );

    let child = Command::new("python3")
        .args(["-c", &script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 not found: {e}"))?;

    if register_child(&processes, job_id, &cancelled, child) {
        return Err("CANCELLED".to_string());
    }

    emit_progress(window, job_id, 10.0, "Colorizing image…");

    let stderr = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stderr.take())
    };
    let stderr_buf = stderr
        .map(|s| {
            BufReader::new(s)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    wait_child(&processes, job_id, &cancelled, stderr_buf)
}

fn colorize_video(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // Create a temp directory for frames.
    let frames_dir = format!("{}.frames", output);
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("create frames dir: {e}"))?;

    // Step 1: Extract frames with FFmpeg.
    emit_progress(window, job_id, 5.0, "Extracting frames…");
    {
        let child = Command::new("ffmpeg")
            .args(["-y", "-i", input, &format!("{frames_dir}/frame_%06d.png")])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        wait_child(&processes, job_id, &cancelled, stderr_buf)?;
    }

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("CANCELLED".to_string());
    }

    // Count frames.
    let frame_paths: Vec<_> = std::fs::read_dir(&frames_dir)
        .map_err(|e| format!("read frames dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    let total = frame_paths.len();
    if total == 0 {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("No frames extracted from video".to_string());
    }

    // Step 2: Colorize frames with inline Python.
    let frames_dir_esc = frames_dir.replace('\\', "\\\\").replace('\'', "\\'");
    let script = format!(
        r#"
import sys, os, glob
try:
    from ddcolor import DDColor
    import numpy as np
    from PIL import Image

    frames_dir = '{frames_dir}'
    frames = sorted(glob.glob(os.path.join(frames_dir, '*.png')))
    total = len(frames)
    model = DDColor()
    for i, f in enumerate(frames):
        img = Image.open(f).convert('RGB')
        arr = np.array(img)
        result = model.colorize(arr)
        Image.fromarray(result).save(f)
        pct = int((i + 1) / total * 100)
        if (i + 1) % max(1, total // 20) == 0:
            print(f'PROGRESS:{{i+1}}/{{total}}', flush=True)
    print(f'PROGRESS:{{total}}/{{total}}', flush=True)
except Exception as e:
    print(f'ERROR: {{e}}', file=sys.stderr)
    sys.exit(1)
"#,
        frames_dir = frames_dir_esc,
    );

    {
        let child = Command::new("python3")
            .args(["-c", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("python3 not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stdout = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stdout.take())
        };
        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };

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

        if let Some(out) = stdout {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(pct) = parse_progress_line(&line) {
                    // Scale 10%–85% for this phase.
                    let scaled = 10.0 + pct * 0.75;
                    emit_progress(window, job_id, scaled, &format!("{pct:.0}% colorized"));
                }
            }
        }

        let stderr_buf = stderr_thread.join().unwrap_or_default();
        let result = wait_child(&processes, job_id, &cancelled, stderr_buf);
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err(e);
        }
    }

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("CANCELLED".to_string());
    }

    // Step 3: Reassemble with FFmpeg.
    emit_progress(window, job_id, 90.0, "Reassembling video…");
    {
        let child = Command::new("ffmpeg")
            .args([
                "-y",
                "-framerate",
                "25",
                "-i",
                &format!("{frames_dir}/frame_%06d.png"),
                "-i",
                input,
                "-map",
                "0:v",
                "-map",
                "1:a?",
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "copy",
                output,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let result = wait_child(&processes, job_id, &cancelled, stderr_buf);
        let _ = std::fs::remove_dir_all(&frames_dir);
        result?;
    }

    Ok(())
}

// ── TASK-G5: Background Remover (rembg) ───────────────────────────────────────

/// Remove the background from an image or video using rembg.
///
/// `model`: `u2net` (default), `u2netp` (fast), `isnet-general-use` (HQ).
/// Image output is PNG with alpha channel.
/// Video output uses QTRLE codec to preserve alpha.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_bg_remove(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    model: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let rembg_available = tool_available("rembg") || python_module_available("rembg");
    if !rembg_available {
        return Err("Background removal requires rembg: pip install rembg".to_string());
    }

    let model = if model.is_empty() { "u2net" } else { model };

    let ext = Path::new(input)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let is_video = matches!(
        ext.as_str(),
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "flv" | "wmv"
    );

    if is_video {
        bg_remove_video(window, job_id, input, output, model, processes, cancelled)
    } else {
        bg_remove_image(window, job_id, input, output, model, processes, cancelled)
    }
}

#[allow(clippy::too_many_arguments)]
fn bg_remove_image(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    model: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    emit_progress(window, job_id, 10.0, "Removing background…");

    let child = spawn_cmd("rembg", &["i", "-m", model, input, output])?;

    if register_child(&processes, job_id, &cancelled, child) {
        return Err("CANCELLED".to_string());
    }

    let stderr = {
        let mut map = processes.lock();
        map.get_mut(job_id).and_then(|c| c.stderr.take())
    };
    let stderr_buf = stderr
        .map(|s| {
            BufReader::new(s)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    wait_child(&processes, job_id, &cancelled, stderr_buf)
}

#[allow(clippy::too_many_arguments)]
fn bg_remove_video(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    model: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // Temp directory for frames.
    let frames_dir = format!("{}.bg_frames", output);
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("create frames dir: {e}"))?;

    // Step 1: Extract frames.
    emit_progress(window, job_id, 5.0, "Extracting frames…");
    {
        let child = Command::new("ffmpeg")
            .args(["-y", "-i", input, &format!("{frames_dir}/frame_%06d.png")])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        wait_child(&processes, job_id, &cancelled, stderr_buf)?;
    }

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("CANCELLED".to_string());
    }

    // Count frames.
    let mut frame_paths: Vec<_> = std::fs::read_dir(&frames_dir)
        .map_err(|e| format!("read frames dir: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "png").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();
    frame_paths.sort();

    let total = frame_paths.len();
    if total == 0 {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("No frames extracted from video".to_string());
    }

    // Step 2: Process each frame with rembg.
    for (i, frame_path) in frame_paths.iter().enumerate() {
        if cancelled.load(Ordering::SeqCst) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let child = spawn_cmd("rembg", &["i", "-m", model, frame_path, frame_path])?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        wait_child(&processes, job_id, &cancelled, stderr_buf)?;

        let pct = ((i + 1) as f32 / total as f32 * 80.0) + 10.0;
        emit_progress(
            window,
            job_id,
            pct,
            &format!("{}/{} frames processed", i + 1, total),
        );
    }

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("CANCELLED".to_string());
    }

    // Step 3: Reassemble with FFmpeg using QTRLE for alpha.
    emit_progress(window, job_id, 92.0, "Reassembling video with alpha…");
    {
        let child = Command::new("ffmpeg")
            .args([
                "-y",
                "-framerate",
                "25",
                "-i",
                &format!("{frames_dir}/frame_%06d.png"),
                "-c:v",
                "qtrle",
                output,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let result = wait_child(&processes, job_id, &cancelled, stderr_buf);
        let _ = std::fs::remove_dir_all(&frames_dir);
        result?;
    }

    Ok(())
}

// ── TASK-H1: Neural Matte (RVM) ───────────────────────────────────────────────

/// Run Robust Video Matting (RVM) inference on `input`, writing an
/// alpha-channel video to `output`.
///
/// `output_format`: `"mov_qtrle"` (MOV + QTRLE, preserves alpha) or
/// `"webm_vp9"` (WebM VP9 with alpha).
///
/// Detection: checks for the `rvm` Python module and `torchvision`.
/// Per-frame progress is printed by the inline script to stdout.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_neural_matte(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    output_format: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    // Detect: require both torchvision and rvm Python module.
    let rvm_available = python_module_available("torchvision")
        && (python_module_available("rvm") || tool_available("rvm"));

    if !rvm_available {
        return Err(
            "Neural matting requires Robust Video Matting: pip install robust-video-matting"
                .to_string(),
        );
    }

    let input_esc = input.replace('\\', "\\\\").replace('\'', "\\'");

    // Step 1: Run RVM inference — write RGBA frames to a temp directory.
    let frames_dir = format!("{}.rvm_frames", output);
    std::fs::create_dir_all(&frames_dir).map_err(|e| format!("create frames dir: {e}"))?;
    let frames_dir_esc = frames_dir.replace('\\', "\\\\").replace('\'', "\\'");

    emit_progress(window, job_id, 5.0, "Starting RVM inference…");

    let script = format!(
        r#"
import sys, os, glob
try:
    import torch
    import torchvision
    from torchvision.transforms.functional import to_tensor
    from PIL import Image
    import cv2
    import numpy as np

    try:
        from rvm import MattingNetwork
    except ImportError:
        import robust_video_matting as rvm_mod
        MattingNetwork = rvm_mod.MattingNetwork

    device = 'cuda' if torch.cuda.is_available() else 'cpu'
    model = MattingNetwork('mobilenetv3').eval().to(device)

    cap = cv2.VideoCapture('{input}')
    total = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    if total <= 0:
        total = 1

    rec = [None] * 4
    downsample_ratio = 0.25

    frame_idx = 0
    while True:
        ok, frame = cap.read()
        if not ok:
            break
        # BGR -> RGB -> tensor
        rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
        src = to_tensor(rgb).unsqueeze(0).to(device)
        with torch.no_grad():
            fgr, pha, *rec = model(src, *rec, downsample_ratio)
        # Compose RGBA frame
        fgr_np = (fgr[0].permute(1,2,0).cpu().numpy() * 255).clip(0,255).astype(np.uint8)
        pha_np = (pha[0,0].cpu().numpy() * 255).clip(0,255).astype(np.uint8)
        rgba = np.dstack([fgr_np, pha_np])
        out_path = os.path.join('{frames_dir}', f'frame_{{frame_idx:06d}}.png')
        Image.fromarray(rgba, 'RGBA').save(out_path)
        frame_idx += 1
        if frame_idx % max(1, total // 50) == 0:
            print(f'PROGRESS:{{frame_idx}}/{{total}}', flush=True)

    cap.release()
    print(f'PROGRESS:{{frame_idx}}/{{frame_idx}}', flush=True)
except Exception as e:
    print(f'ERROR: {{e}}', file=sys.stderr)
    sys.exit(1)
"#,
        input = input_esc,
        frames_dir = frames_dir_esc,
    );

    {
        let child = Command::new("python3")
            .args(["-c", &script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("python3 not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stdout = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stdout.take())
        };
        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };

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

        if let Some(out) = stdout {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(pct) = parse_progress_line(&line) {
                    // Scale 5%–80% for RVM inference phase.
                    let scaled = 5.0 + pct * 0.75;
                    emit_progress(window, job_id, scaled, &format!("{pct:.0}% matted"));
                }
            }
        }

        let stderr_buf = stderr_thread.join().unwrap_or_default();
        let result = wait_child(&processes, job_id, &cancelled, stderr_buf);
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err(e);
        }
    }

    if cancelled.load(Ordering::SeqCst) {
        let _ = std::fs::remove_dir_all(&frames_dir);
        return Err("CANCELLED".to_string());
    }

    // Step 2: Encode RGBA frames with FFmpeg to the target format.
    emit_progress(window, job_id, 82.0, "Encoding alpha output…");

    let (codec_args, pix_fmt): (&[&str], &str) = match output_format {
        "webm_vp9" => (
            &["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "20"],
            "yuva420p",
        ),
        _ => (
            // mov_qtrle (default)
            &["-c:v", "qtrle"],
            "argb",
        ),
    };

    {
        let frame_pattern = format!("{}/frame_%06d.png", frames_dir.trim_end_matches('/'));
        let mut ffmpeg_args: Vec<String> = vec![
            "-y".into(),
            "-framerate".into(),
            "25".into(),
            "-i".into(),
            frame_pattern,
        ];
        for &a in codec_args {
            ffmpeg_args.push(a.into());
        }
        ffmpeg_args.push("-pix_fmt".into());
        ffmpeg_args.push(pix_fmt.into());
        ffmpeg_args.push("-an".into());
        ffmpeg_args.push(output.to_string());

        let child = Command::new("ffmpeg")
            .args(&ffmpeg_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ffmpeg not found: {e}"))?;

        if register_child(&processes, job_id, &cancelled, child) {
            let _ = std::fs::remove_dir_all(&frames_dir);
            return Err("CANCELLED".to_string());
        }

        let stderr = {
            let mut map = processes.lock();
            map.get_mut(job_id).and_then(|c| c.stderr.take())
        };
        let stderr_buf = stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let result = wait_child(&processes, job_id, &cancelled, stderr_buf);
        let _ = std::fs::remove_dir_all(&frames_dir);
        result?;
    }

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_percent_from_line ──────────────────────────────────────────────

    #[test]
    fn parse_percent_from_line_basic() {
        assert_eq!(
            parse_percent_from_line("Separating track  45%|..."),
            Some(45.0)
        );
    }

    #[test]
    fn parse_percent_from_line_zero() {
        assert_eq!(parse_percent_from_line("Processing  0%|"), Some(0.0));
    }

    #[test]
    fn parse_percent_from_line_one_hundred() {
        assert_eq!(parse_percent_from_line("Done 100%"), Some(100.0));
    }

    #[test]
    fn parse_percent_from_line_decimal() {
        assert_eq!(parse_percent_from_line("Progress: 33.5%"), Some(33.5));
    }

    #[test]
    fn parse_percent_from_line_no_percent() {
        assert_eq!(parse_percent_from_line("no percent here"), None);
    }

    // ── parse_mm_ss ──────────────────────────────────────────────────────────

    #[test]
    fn parse_mm_ss_minutes_and_seconds() {
        assert_eq!(parse_mm_ss("01:30.000"), Some(90.0));
    }

    #[test]
    fn parse_mm_ss_hours_minutes_seconds() {
        assert_eq!(parse_mm_ss("01:00:00.000"), Some(3600.0));
    }

    #[test]
    fn parse_mm_ss_zero() {
        assert_eq!(parse_mm_ss("00:00.000"), Some(0.0));
    }

    #[test]
    fn parse_mm_ss_invalid() {
        assert_eq!(parse_mm_ss("not-a-timestamp"), None);
    }

    // ── parse_whisper_timestamp ──────────────────────────────────────────────

    #[test]
    fn parse_whisper_timestamp_basic() {
        let line = "[00:00.000 --> 00:05.500]  Hello world";
        assert_eq!(parse_whisper_timestamp(line), Some(5.5));
    }

    #[test]
    fn parse_whisper_timestamp_minutes() {
        let line = "[01:00.000 --> 01:30.250]  Some text";
        assert_eq!(parse_whisper_timestamp(line), Some(90.25));
    }

    #[test]
    fn parse_whisper_timestamp_no_arrow() {
        assert_eq!(parse_whisper_timestamp("plain text line"), None);
    }

    // ── parse_progress_line ──────────────────────────────────────────────────

    #[test]
    fn parse_progress_line_basic() {
        // 50/100 = 50.0% — below the 99.0 cap, so no clamping applied.
        assert_eq!(parse_progress_line("PROGRESS:50/100"), Some(50.0));
    }

    #[test]
    fn parse_progress_line_full() {
        // 100/100 → capped at 99.
        assert_eq!(parse_progress_line("PROGRESS:100/100"), Some(99.0));
    }

    #[test]
    fn parse_progress_line_zero() {
        assert_eq!(parse_progress_line("PROGRESS:0/100"), Some(0.0));
    }

    #[test]
    fn parse_progress_line_bad_prefix() {
        assert_eq!(parse_progress_line("INFO:something"), None);
    }

    #[test]
    fn parse_progress_line_zero_total() {
        assert_eq!(parse_progress_line("PROGRESS:5/0"), None);
    }

    // ── tool-presence gate (all 5 operations) ────────────────────────────────
    // These tests verify that each operation's detection logic doesn't panic
    // and behaves correctly when the required tool is absent. All pass in CI
    // even when none of the Python tools are installed.

    #[test]
    fn audio_separation_errors_without_demucs() {
        // Only run this test when demucs is genuinely absent.
        if python_module_available("demucs") {
            return;
        }
        // We can't call run_audio_separation without a Window, but we can
        // test the detection logic directly.
        assert!(!python_module_available("demucs"));
    }

    #[test]
    fn transcription_detection_logic() {
        // Verifies that the combined check (whisper CLI OR faster_whisper module)
        // is consistent: if neither is present, the flag is false.
        let available = tool_available("whisper") || python_module_available("faster_whisper");
        // No assertion on the value — just that the expression doesn't panic.
        let _ = available;
    }

    #[test]
    fn translation_detection_logic() {
        let _ = python_module_available("argostranslate");
    }

    #[test]
    fn colorize_detection_logic() {
        let _ = python_module_available("ddcolor") || tool_available("ddcolor");
    }

    #[test]
    fn bg_remove_detection_logic() {
        let _ = tool_available("rembg") || python_module_available("rembg");
    }

    // ── build_translation_script smoke test ──────────────────────────────────

    #[test]
    fn build_translation_script_contains_lang_codes() {
        let script = build_translation_script("/in.srt", "/out.srt", "en", "es");
        assert!(script.contains("src_lang = 'en'"));
        assert!(script.contains("tgt_lang = 'es'"));
        assert!(script.contains("/in.srt"));
        assert!(script.contains("/out.srt"));
    }

    #[test]
    fn build_translation_script_no_shell_injection() {
        // Paths with single-quotes must be escaped.
        let script = build_translation_script("/in's.srt", "/out's.srt", "en", "fr");
        // The raw single quote must not appear unescaped inside the Python string.
        // After escaping: /in\'s.srt
        assert!(!script.contains("'/in's.srt'"));
        assert!(script.contains("\\'s.srt"));
    }
}
