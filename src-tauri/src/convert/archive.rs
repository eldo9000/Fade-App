use crate::{truncate_stderr, ConvertOptions, JobDone, JobProgress};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Window};

/// 7-Zip ships under two binary names depending on platform/packaging:
/// `7z` (classic p7zip, Linux distros, Windows installer) vs `7zz` (modern
/// sevenzip project, Homebrew on macOS). Return whichever exists in PATH,
/// preferring `7z` when both are available.
fn seven_zip_bin() -> &'static str {
    if Command::new("7z").arg("i").output().is_ok() {
        "7z"
    } else {
        "7zz"
    }
}

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let operation = opts.archive_operation.as_deref().unwrap_or("convert");

    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: if operation == "extract" {
                "Extracting…".to_string()
            } else {
                "Repacking…".to_string()
            },
        },
    );

    if operation == "extract" {
        // Extract to {stem}_extracted/ beside input
        let p = Path::new(input_path);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let parent = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        let out_dir = opts.output_dir.as_deref().unwrap_or(&parent);
        let extract_folder = format!("{}/{}_extracted", out_dir, stem);

        let mut child = Command::new(seven_zip_bin())
            .args(["x", input_path, &format!("-o{}", extract_folder), "-y"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("7z not found: {e}"))?;

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
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit(
                        "job-progress",
                        JobProgress {
                            job_id: job_id.to_string(),
                            percent: pct,
                            message: format!("{}%", pct as u32),
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
            Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err("CANCELLED".to_string());
        }
        if success {
            // Emit job-done with the extract folder as output_path; caller sees
            // __DONE__ sentinel and skips its default job-done emission.
            let _ = window.emit(
                "job-done",
                JobDone {
                    job_id: job_id.to_string(),
                    output_path: extract_folder,
                },
            );
            return Err("__DONE__".to_string());
        } else {
            return Err(if error_output.trim().is_empty() {
                "7z extraction failed".to_string()
            } else {
                truncate_stderr(&error_output)
            });
        }
    }

    // Convert: extract to temp dir, repack to new format
    let tmp_dir = format!("/tmp/fade_archive_{}", job_id);
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

    // Step 1: extract
    {
        let mut child = Command::new(seven_zip_bin())
            .args(["x", input_path, &format!("-o{}", tmp_dir), "-y"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("7z not found: {e}"))?;

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
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit(
                        "job-progress",
                        JobProgress {
                            job_id: job_id.to_string(),
                            percent: pct / 2.0,
                            message: format!("Extracting {}%", pct as u32),
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
            Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        if cancelled.load(Ordering::SeqCst) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err("CANCELLED".to_string());
        }
        if !success {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err(if error_output.trim().is_empty() {
                "7z extraction failed".to_string()
            } else {
                truncate_stderr(&error_output)
            });
        }
    }

    // Step 2: repack. 7z's `-mx=<0..9>` sets compression level for all supported
    // output formats (zip, 7z, gz, etc.). Omit when the user hasn't set one so
    // the binary uses its own default (typically 5).
    {
        let mut repack_args: Vec<String> = vec![
            "a".to_string(),
            output_path.to_string(),
            format!("{}/*", tmp_dir),
        ];
        if let Some(level) = opts.archive_compression {
            repack_args.push(format!("-mx={}", level.min(9)));
        }
        let mut child = Command::new(seven_zip_bin())
            .args(&repack_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("7z not found: {e}"))?;

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
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit(
                        "job-progress",
                        JobProgress {
                            job_id: job_id.to_string(),
                            percent: 50.0 + pct / 2.0,
                            message: format!("Packing {}%", pct as u32),
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
            Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        if cancelled.load(Ordering::SeqCst) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return Err("CANCELLED".to_string());
        }
        let _ = std::fs::remove_dir_all(&tmp_dir);
        if !success {
            return Err(if error_output.trim().is_empty() {
                "7z repack failed".to_string()
            } else {
                truncate_stderr(&error_output)
            });
        }
    }

    Ok(())
}

/// Parse 7z progress lines like "  7% - filename.ext"
fn parse_7z_percent(line: &str) -> Option<f32> {
    let trimmed = line.trim();
    let pct_end = trimmed.find('%')?;
    trimmed[..pct_end].trim().parse::<f32>().ok()
}
