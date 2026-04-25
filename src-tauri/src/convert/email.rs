//! Email conversion — pure Rust, no shell.
//!
//! An mbox file is a concatenation of RFC2822 messages separated by
//! lines starting with `From ` (space, no colon). This module handles
//! eml ↔ mbox. `.msg` (Outlook binary) is deferred and returns a clear
//! error if requested as output.

use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{ConvertOptions, JobProgress};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, Window};

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
    let job_id_owned = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = match ev {
            ProgressEvent::Started => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: "Converting email…".to_string(),
            },
            ProgressEvent::Phase(msg) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: msg,
            },
            ProgressEvent::Percent(p) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: (p * 100.0).clamp(0.0, 100.0),
                message: String::new(),
            },
            ProgressEvent::Done => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        };
        let _ = win.emit("job-progress", payload);
    };
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
