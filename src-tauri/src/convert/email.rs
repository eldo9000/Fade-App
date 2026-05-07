//! Email conversion — pure Rust, no shell.
//!
//! An mbox file is a concatenation of RFC2822 messages separated by
//! lines starting with `From ` (space, no colon). This module handles
//! eml ↔ mbox. `.msg` (Outlook binary) is converted via `msgconvert`
//! (libemail-outlook-message-perl) or `pst-convert` (libpst).

use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::ConvertOptions;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

// ── MSG conversion helper ─────────────────────────────────────────────────────

/// Convert an Outlook `.msg` file to EML via `msgconvert` or `pst-convert`.
///
/// Tool preference: `msgconvert` first (libemail-outlook-message-perl),
/// then `pst-convert` from libpst. If neither is found returns a clear error.
pub fn convert_msg(input: &str, output_path: &str, progress: ProgressFn<'_>) -> Result<(), String> {
    // Detect available tool
    let tool = {
        let has_msgconvert = Command::new("which")
            .arg("msgconvert")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        let has_pst_convert = Command::new("which")
            .arg("pst-convert")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if has_msgconvert {
            "msgconvert"
        } else if has_pst_convert {
            "pst-convert"
        } else {
            return Err(
                "MSG conversion requires msgconvert or libpst.\n\
                Install with:\n  \
                  macOS/Linux: brew install libpst  (provides pst-convert)\n  \
                  Debian/Ubuntu: sudo apt install libemail-outlook-message-perl  (provides msgconvert)\n  \
                  or: sudo apt install libpst-dev".to_string()
            );
        }
    };

    progress(ProgressEvent::Phase(format!("Converting MSG via {tool}…")));

    let result = if tool == "msgconvert" {
        // msgconvert writes to stdout; capture and write to output_path
        Command::new("msgconvert")
            .args(["--outfile", output_path, input])
            .output()
    } else {
        // pst-convert: `pst-convert -o <outdir> <input>` — writes to directory
        // We pass the output path parent as the outdir and rename afterwards.
        let out_dir = Path::new(output_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .to_string();
        Command::new("pst-convert")
            .args(["-o", &out_dir, input])
            .output()
    };

    let output = result.map_err(|e| format!("Failed to spawn {tool}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{tool} failed:\n{}",
            crate::truncate_stderr(&stderr)
        ));
    }

    // For pst-convert we need to locate the output file and move it
    if tool == "pst-convert" {
        let out_dir = Path::new(output_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let input_stem = Path::new(input)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // pst-convert typically produces <stem>.eml or similar; try stem.eml
        let candidate = out_dir.join(format!("{input_stem}.eml"));
        if candidate.exists() && candidate != Path::new(output_path) {
            std::fs::rename(&candidate, output_path)
                .map_err(|e| format!("Could not move pst-convert output: {e}"))?;
        } else if !Path::new(output_path).exists() {
            return Err(format!(
                "pst-convert ran but output file not found at expected path: {}",
                candidate.display()
            ));
        }
    }

    progress(ProgressEvent::Done);
    Ok(())
}

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// The `cancelled` flag is accepted for signature parity with other modules
/// but is not consulted — email conversions are instant and have no
/// long-running step to interrupt.
pub fn convert(
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    _cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    progress(ProgressEvent::Started);

    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();

    if out_fmt == "msg" {
        return Err("MSG output is not supported — try EML or MBOX.".to_string());
    }

    // MSG input — delegate to the shell-out helper
    if in_ext == "msg" {
        return convert_msg(input_path, output_path, progress);
    }

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;

    let output = match (in_ext.as_str(), out_fmt.as_str()) {
        ("eml", "mbox") => eml_to_mbox(&raw),
        ("mbox", "eml") => mbox_to_eml(&raw)?,
        ("eml", "eml") | ("mbox", "mbox") => raw,
        (i, o) => return Err(format!("Unsupported email conversion: {i} → {o}")),
    };

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;

    progress(ProgressEvent::Done);
    Ok(())
}

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
) -> Result<(), String> {
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting email…");
    let cancelled = Arc::new(AtomicBool::new(false));
    convert(input_path, output_path, opts, &mut emit, &cancelled)
}

pub fn eml_to_mbox(raw: &str) -> String {
    // Mbox format uses `From <sender> <date>` postmark lines. We emit a
    // minimal, parseable one; downstream mbox readers only require the
    // `From ` prefix + a date-ish trailer.
    let postmark = "From -\n";
    // Escape any stray `From ` lines inside the message body so they
    // don't get re-interpreted as message separators.
    let escaped = raw
        .lines()
        .map(|l| {
            if l.starts_with("From ") {
                format!(">{l}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = String::with_capacity(postmark.len() + escaped.len() + 1);
    out.push_str(postmark);
    out.push_str(&escaped);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

pub fn mbox_to_eml(raw: &str) -> Result<String, String> {
    let mut first: Option<Vec<&str>> = None;
    let mut current: Vec<&str> = Vec::new();
    let mut started = false;
    for line in raw.lines() {
        if line.starts_with("From ") {
            if started {
                first = Some(std::mem::take(&mut current));
                break;
            }
            started = true;
            continue; // skip the postmark line itself
        }
        if started {
            current.push(line);
        }
    }
    if first.is_none() && started {
        first = Some(current);
    }
    let msg = first.ok_or_else(|| "No messages found in mbox".to_string())?;
    // Un-escape `>From ` → `From `.
    let out = msg
        .iter()
        .map(|l| {
            l.strip_prefix('>')
                .filter(|r| r.starts_with("From "))
                .unwrap_or(l)
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(out)
}
