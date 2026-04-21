//! Font conversion pipeline — shells out to `fonttools` (Python) via a
//! small inline script. `TTFont` auto-detects the input flavor and setting
//! `.flavor` before `.save()` picks the output wrapping (woff/woff2/raw
//! sfnt). Follows the shape of `convert::model` since the tool runs to
//! completion without streaming progress.
//!
//! Known limit: ttf↔otf does NOT re-encode the underlying outline table
//! (glyf vs. CFF). The sfnt container is re-wrapped but the inner outline
//! format is preserved. For true outline conversion the user needs afdko
//! (`otf2ttf` / `ttf2otf`). Good enough for web-font pipelines, which is
//! the dominant use case.

use crate::{truncate_stderr, ConvertOptions, JobProgress};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::{Emitter, Window};

const FONTTOOLS_SCRIPT: &str = r#"
import sys
from fontTools.ttLib import TTFont
input_path, output_path = sys.argv[1], sys.argv[2]
out_ext = output_path.rsplit('.', 1)[-1].lower()
flavor_map = {'woff': 'woff', 'woff2': 'woff2', 'ttf': None, 'otf': None}
if out_ext not in flavor_map:
    print(f"Unsupported output format: {out_ext}", file=sys.stderr)
    sys.exit(1)
font = TTFont(input_path)
font.flavor = flavor_map[out_ext]
font.save(output_path)
"#;

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
            message: "Converting font…".to_string(),
        },
    );

    // woff2 needs the `brotli` Python module; without it fonttools raises at
    // save time. We point the user at both in the install hint to avoid a
    // second round-trip.
    let out_ext = Path::new(output)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let python = if Command::new("python3").arg("--version").output().is_ok() {
        "python3"
    } else {
        "python"
    };

    let mut child = Command::new(python)
        .args(["-c", FONTTOOLS_SCRIPT, input, output])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!(
            "fonttools not found: {e}\n\nInstall with:\n  pip install fonttools brotli\n\n(brotli is required for .woff2 output.)"
        ))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
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
        let mut map = processes.lock();
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
        let _ = window.emit(
            "job-progress",
            JobProgress {
                job_id: job_id.to_string(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        );
        Ok(())
    } else if stderr_content.contains("No module named") && stderr_content.contains("fontTools") {
        Err("fonttools not installed.\n\nInstall with:\n  pip install fonttools brotli".to_string())
    } else if out_ext == "woff2" && stderr_content.contains("brotli") {
        Err("brotli Python module required for .woff2 output.\n\nInstall with:\n  pip install brotli".to_string())
    } else {
        Err(if stderr_content.trim().is_empty() {
            "fonttools conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}
