//! lint_subtitle — native parser + caption-style rules.
//!
//! Supported formats: SRT, WebVTT, ASS/SSA. Detection is by file extension;
//! for all three we extract a list of cues (start/end/text) and then apply
//! the threshold checks. Rules:
//!   · CPS (chars/sec) ceiling
//!   · min/max duration ms
//!   · per-line max chars
//!   · max lines per cue

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Clone, Copy)]
pub struct LintThresholds {
    pub cps_max: f64,
    pub min_dur_ms: u32,
    pub max_dur_ms: u32,
    pub line_max_chars: u32,
    pub max_lines: u32,
}

#[derive(Serialize, Clone)]
pub struct LintIssue {
    pub cue_index: usize,
    pub time: String,
    pub kind: String,
    pub message: String,
}

struct Cue {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

fn parse_srt_time(s: &str) -> Option<u64> {
    // "HH:MM:SS,mmm" or "HH:MM:SS.mmm"
    let s = s.trim().replace(',', ".");
    let (hms, ms) = s.rsplit_once('.')?;
    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec: u64 = parts[2].parse().ok()?;
    let ms: u64 = ms.parse().ok()?;
    Some(h * 3_600_000 + m * 60_000 + sec * 1_000 + ms)
}

fn parse_ass_time(s: &str) -> Option<u64> {
    // "H:MM:SS.CC"  (centiseconds)
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let (sec, cs) = parts[2].split_once('.').unwrap_or((parts[2], "0"));
    let sec: u64 = sec.parse().ok()?;
    let cs: u64 = cs.parse().ok()?;
    Some(h * 3_600_000 + m * 60_000 + sec * 1_000 + cs * 10)
}

fn parse_srt(body: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut lines = body.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Skip sequence number if present.
        let time_line = if line.chars().all(|c| c.is_ascii_digit()) {
            lines.next().unwrap_or("").trim()
        } else {
            line
        };
        let Some((a, b)) = time_line.split_once("-->") else {
            continue;
        };
        let Some(start_ms) = parse_srt_time(a) else {
            continue;
        };
        let Some(end_ms) = parse_srt_time(b.split_whitespace().next().unwrap_or(b)) else {
            continue;
        };
        let mut text = String::new();
        for l in lines.by_ref() {
            if l.trim().is_empty() {
                break;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(l);
        }
        cues.push(Cue {
            start_ms,
            end_ms,
            text,
        });
    }
    cues
}

fn parse_vtt(body: &str) -> Vec<Cue> {
    // WebVTT is a superset of SRT's cue format with "." instead of "," and
    // an optional "WEBVTT" header + NOTE blocks. Strip those, re-use parser.
    let mut filtered = String::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("WEBVTT") || t.starts_with("NOTE") || t.starts_with("STYLE") {
            continue;
        }
        filtered.push_str(line);
        filtered.push('\n');
    }
    parse_srt(&filtered)
}

fn parse_ass(body: &str) -> Vec<Cue> {
    // ASS: lines beginning with "Dialogue:" have comma-separated fields.
    //   Dialogue: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
    let mut cues = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.strip_prefix("Dialogue:") else {
            continue;
        };
        // Split at most 9 commas to preserve commas inside Text.
        let parts: Vec<&str> = rest.splitn(10, ',').collect();
        if parts.len() < 10 {
            continue;
        }
        let Some(start_ms) = parse_ass_time(parts[1]) else {
            continue;
        };
        let Some(end_ms) = parse_ass_time(parts[2]) else {
            continue;
        };
        // Strip ASS override blocks {\...} and convert \N to newline.
        let mut text = String::new();
        let raw = parts[9].replace("\\N", "\n").replace("\\n", "\n");
        let mut in_override = false;
        for ch in raw.chars() {
            if ch == '{' {
                in_override = true;
            } else if ch == '}' {
                in_override = false;
            } else if !in_override {
                text.push(ch);
            }
        }
        cues.push(Cue {
            start_ms,
            end_ms,
            text,
        });
    }
    cues
}

fn fmt_time_ms(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms / 60_000) % 60;
    let s = (ms / 1_000) % 60;
    let mss = ms % 1_000;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, mss)
}

#[tauri::command]
pub async fn lint_subtitle(
    input_path: String,
    thresholds: LintThresholds,
) -> Result<Vec<LintIssue>, String> {
    tokio::task::spawn_blocking(move || -> Result<Vec<LintIssue>, String> {
        crate::validate_no_traversal(&input_path)?;
        let body = super::read_subtitle_capped(&input_path)?;
        let ext = Path::new(&input_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let cues = match ext.as_str() {
            "srt" => parse_srt(&body),
            "vtt" => parse_vtt(&body),
            "ass" | "ssa" => parse_ass(&body),
            _ => return Err(format!("unsupported subtitle format: .{ext}")),
        };

        let mut issues = Vec::new();
        for (i, cue) in cues.iter().enumerate() {
            let time = fmt_time_ms(cue.start_ms);
            let dur_ms = cue.end_ms.saturating_sub(cue.start_ms);
            let plain_len: usize = cue.text.chars().filter(|c| *c != '\n').count();

            if dur_ms < thresholds.min_dur_ms as u64 {
                issues.push(LintIssue {
                    cue_index: i,
                    time: time.clone(),
                    kind: "duration_short".to_string(),
                    message: format!("cue is {} ms (<{})", dur_ms, thresholds.min_dur_ms),
                });
            }
            if dur_ms > thresholds.max_dur_ms as u64 {
                issues.push(LintIssue {
                    cue_index: i,
                    time: time.clone(),
                    kind: "duration_long".to_string(),
                    message: format!("cue is {} ms (>{})", dur_ms, thresholds.max_dur_ms),
                });
            }
            if dur_ms > 0 {
                let cps = plain_len as f64 / (dur_ms as f64 / 1000.0);
                if cps > thresholds.cps_max {
                    issues.push(LintIssue {
                        cue_index: i,
                        time: time.clone(),
                        kind: "cps_high".to_string(),
                        message: format!("{:.1} cps (>{})", cps, thresholds.cps_max),
                    });
                }
            }
            let lines: Vec<&str> = cue.text.split('\n').collect();
            if lines.len() > thresholds.max_lines as usize {
                issues.push(LintIssue {
                    cue_index: i,
                    time: time.clone(),
                    kind: "too_many_lines".to_string(),
                    message: format!("{} lines (>{})", lines.len(), thresholds.max_lines),
                });
            }
            for (li, l) in lines.iter().enumerate() {
                if l.chars().count() > thresholds.line_max_chars as usize {
                    issues.push(LintIssue {
                        cue_index: i,
                        time: time.clone(),
                        kind: "line_too_long".to_string(),
                        message: format!(
                            "line {} is {} chars (>{})",
                            li + 1,
                            l.chars().count(),
                            thresholds.line_max_chars
                        ),
                    });
                }
            }
        }
        Ok(issues)
    })
    .await
    .map_err(|e| e.to_string())?
}
