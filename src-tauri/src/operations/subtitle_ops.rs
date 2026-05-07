//! Subtitle operations:
//!   · burn_subtitles  — hard-burn SRT/ASS into video via `ffmpeg -vf subtitles=`
//!   · embed_subtitles — soft-embed subtitle track (stream copy + add sub stream)
//!   · shift_subtitles — pure Rust: parse SRT, shift all timestamps by N ms

use crate::operations::run_ffmpeg;
use crate::probe_duration;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

// ── H6-a: Burn subtitles ──────────────────────────────────────────────────────

/// Hard-burn a subtitle file into a video.
///
/// Uses `ffmpeg -vf subtitles=<subtitle_file>` to render subtitles into the
/// picture. Output retains the original audio track (copy).
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_burn_subtitles(
    window: &Window,
    job_id: &str,
    video: &str,
    subtitle_file: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(video);

    // Escape single-quotes in the subtitle path for the ffmpeg filter string.
    let sub_esc = subtitle_file.replace('\\', "\\\\").replace('\'', "'\\''");
    let vf = format!("subtitles='{sub_esc}'");

    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        video.to_string(),
        "-vf".into(),
        vf,
        "-c:a".into(),
        "copy".into(),
        "-progress".into(),
        "pipe:1".into(),
        output.to_string(),
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── H6-b: Embed subtitles ─────────────────────────────────────────────────────

/// Soft-embed a subtitle file as an additional track in the output container.
///
/// The video and audio streams are stream-copied; the subtitle is added as a
/// new track. Codec selection depends on the output container:
///   - `.mp4` / `.m4v`  → `mov_text`
///   - `.mkv`           → `ass` (or `copy` if input is already ASS/SSA)
///   - all other        → `copy` (pass-through)
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_embed_subtitles(
    window: &Window,
    job_id: &str,
    video: &str,
    subtitle_file: &str,
    output: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(video);

    let out_ext = Path::new(output)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let sub_codec = match out_ext.as_str() {
        "mp4" | "m4v" => "mov_text",
        "mkv" => "ass",
        _ => "copy",
    };

    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        video.to_string(),
        "-i".into(),
        subtitle_file.to_string(),
        "-c".into(),
        "copy".into(),
        "-c:s".into(),
        sub_codec.to_string(),
        "-progress".into(),
        "pipe:1".into(),
        output.to_string(),
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── H6-c: Shift subtitles (pure Rust) ────────────────────────────────────────

/// Parse an SRT file at `input_srt`, shift every timestamp by `shift_ms`
/// milliseconds (negative = shift earlier), and write the result to
/// `output_srt`.
///
/// Lines that are not SRT timestamp lines are copied verbatim. Timestamps
/// that would become negative are clamped to `00:00:00,000`.
///
/// Returns `Ok(())` on success or an error string on failure.
pub fn run_shift_subtitles(input_srt: &str, output_srt: &str, shift_ms: i64) -> Result<(), String> {
    let content = std::fs::read_to_string(input_srt).map_err(|e| format!("read SRT: {e}"))?;

    let shifted = shift_srt(&content, shift_ms);

    std::fs::write(output_srt, shifted).map_err(|e| format!("write SRT: {e}"))?;
    Ok(())
}

// ── SRT parsing helpers ───────────────────────────────────────────────────────

/// Shift all SRT timestamps in `content` by `shift_ms` milliseconds and
/// return the modified content as a `String`. This is the pure-Rust core
/// used by both `run_shift_subtitles` and the unit tests.
pub(crate) fn shift_srt(content: &str, shift_ms: i64) -> String {
    let mut out = String::with_capacity(content.len());
    for line in content.lines() {
        if let Some(shifted_line) = try_shift_timestamp_line(line, shift_ms) {
            out.push_str(&shifted_line);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// If `line` is an SRT timestamp line (`HH:MM:SS,mmm --> HH:MM:SS,mmm`),
/// return the line with both timestamps shifted; otherwise return `None`.
fn try_shift_timestamp_line(line: &str, shift_ms: i64) -> Option<String> {
    let arrow = line.find(" --> ")?;
    let start_str = line[..arrow].trim();
    let end_str = line[arrow + 5..].trim();

    let start_ms = parse_srt_ts(start_str)?;
    let end_ms = parse_srt_ts(end_str)?;

    let new_start = (start_ms + shift_ms).max(0);
    let new_end = (end_ms + shift_ms).max(0);

    Some(format!(
        "{} --> {}",
        format_srt_ts(new_start),
        format_srt_ts(new_end)
    ))
}

/// Parse `HH:MM:SS,mmm` into total milliseconds.
fn parse_srt_ts(ts: &str) -> Option<i64> {
    // Split on ',' first to isolate ms.
    let (hms, ms_str) = ts.split_once(',')?;
    let ms: i64 = ms_str.trim().parse().ok()?;
    let mut hms_parts = hms.splitn(3, ':');
    let h: i64 = hms_parts.next()?.trim().parse().ok()?;
    let m: i64 = hms_parts.next()?.trim().parse().ok()?;
    let s: i64 = hms_parts.next()?.trim().parse().ok()?;
    Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
}

/// Format total milliseconds back to `HH:MM:SS,mmm`.
fn format_srt_ts(total_ms: i64) -> String {
    let ms = total_ms % 1_000;
    let total_s = total_ms / 1_000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_srt_ts ──────────────────────────────────────────────────────────

    #[test]
    fn parse_srt_ts_basic() {
        assert_eq!(parse_srt_ts("00:00:01,000"), Some(1_000));
        assert_eq!(parse_srt_ts("00:01:00,000"), Some(60_000));
        assert_eq!(parse_srt_ts("01:00:00,000"), Some(3_600_000));
        assert_eq!(parse_srt_ts("00:00:00,500"), Some(500));
    }

    #[test]
    fn parse_srt_ts_invalid_returns_none() {
        assert_eq!(parse_srt_ts("not-a-timestamp"), None);
        assert_eq!(parse_srt_ts("00:00"), None);
    }

    // ── format_srt_ts ─────────────────────────────────────────────────────────

    #[test]
    fn format_srt_ts_basic() {
        assert_eq!(format_srt_ts(1_000), "00:00:01,000");
        assert_eq!(format_srt_ts(61_500), "00:01:01,500");
        assert_eq!(format_srt_ts(3_661_001), "01:01:01,001");
    }

    #[test]
    fn format_srt_ts_zero() {
        assert_eq!(format_srt_ts(0), "00:00:00,000");
    }

    // ── try_shift_timestamp_line ──────────────────────────────────────────────

    #[test]
    fn shift_timestamp_line_positive() {
        let result = try_shift_timestamp_line("00:00:01,000 --> 00:00:03,500", 500).unwrap();
        assert_eq!(result, "00:00:01,500 --> 00:00:04,000");
    }

    #[test]
    fn shift_timestamp_line_negative() {
        let result = try_shift_timestamp_line("00:00:05,000 --> 00:00:07,000", -2_000).unwrap();
        assert_eq!(result, "00:00:03,000 --> 00:00:05,000");
    }

    #[test]
    fn shift_timestamp_line_clamps_to_zero() {
        // Both timestamps would go negative — clamp at 0.
        let result = try_shift_timestamp_line("00:00:00,100 --> 00:00:00,500", -1_000).unwrap();
        assert_eq!(result, "00:00:00,000 --> 00:00:00,000");
    }

    #[test]
    fn shift_timestamp_line_non_ts_returns_none() {
        assert_eq!(try_shift_timestamp_line("1", 500), None);
        assert_eq!(try_shift_timestamp_line("Hello world", 500), None);
        assert_eq!(try_shift_timestamp_line("", 500), None);
    }

    // ── shift_srt (full document) ─────────────────────────────────────────────

    #[test]
    fn shift_srt_preserves_non_ts_lines() {
        let input =
            "1\n00:00:01,000 --> 00:00:03,000\nHello world\n\n2\n00:00:04,000 --> 00:00:06,000\nFoo bar\n";
        let out = shift_srt(input, 1_000);
        assert!(out.contains("Hello world"), "text lost");
        assert!(out.contains("Foo bar"), "text lost");
        assert!(
            out.contains("00:00:02,000 --> 00:00:04,000"),
            "ts not shifted"
        );
        assert!(
            out.contains("00:00:05,000 --> 00:00:07,000"),
            "ts not shifted"
        );
    }

    #[test]
    fn shift_srt_zero_shift_idempotent() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nHi\n";
        let out = shift_srt(input, 0);
        assert!(out.contains("00:00:01,000 --> 00:00:02,000"));
    }

    // ── run_shift_subtitles (integration: reads/writes real files) ────────────

    #[test]
    fn run_shift_subtitles_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "fade-sub-shift-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let input = dir.join("in.srt");
        let output = dir.join("out.srt");

        std::fs::write(&input, "1\n00:00:01,000 --> 00:00:03,000\nTest cue\n\n").unwrap();

        run_shift_subtitles(input.to_str().unwrap(), output.to_str().unwrap(), 2_000)
            .expect("shift should succeed");

        let result = std::fs::read_to_string(&output).unwrap();
        assert!(
            result.contains("00:00:03,000 --> 00:00:05,000"),
            "got: {result}"
        );
        assert!(result.contains("Test cue"), "text lost");

        std::fs::remove_dir_all(&dir).ok();
    }
}
