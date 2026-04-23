use crate::{truncate_stderr, ConvertOptions, ConvertResult, JobDone, JobProgress};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tauri::{Emitter, Window};

/// 7-Zip ships under two binary names depending on platform/packaging:
/// `7z` (classic p7zip, Linux distros, Windows installer) vs `7zz` (modern
/// sevenzip project, Homebrew on macOS). Return whichever exists in PATH,
/// preferring `7z` when both are available. Result is memoized for the
/// process lifetime — probing spawns a subprocess and we call this on every
/// archive op.
static SEVEN_ZIP_BIN: OnceLock<&'static str> = OnceLock::new();

fn resolve_seven_zip_bin(probe: impl Fn(&str) -> bool) -> &'static str {
    if probe("7z") {
        "7z"
    } else {
        "7zz"
    }
}

fn seven_zip_bin() -> &'static str {
    SEVEN_ZIP_BIN
        .get_or_init(|| resolve_seven_zip_bin(|name| Command::new(name).arg("i").output().is_ok()))
}

fn tool_in_path(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let operation = opts.archive_operation.as_deref().unwrap_or("convert");
    let in_ext = ext_of(input_path);
    let out_ext = ext_of(output_path);

    // Repack guards: some formats are extract-only or platform-locked.
    if operation != "extract" {
        if out_ext == "rar" {
            return ConvertResult::Error(
                "RAR creation is not supported (proprietary) — try 7z or zip".to_string(),
            );
        }
        if out_ext == "dmg" && !cfg!(target_os = "macos") {
            return ConvertResult::Error("DMG creation is macOS-only".to_string());
        }
    }

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
        let p = Path::new(input_path);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let parent = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        let out_dir = opts.output_dir.as_deref().unwrap_or(&parent);
        let extract_folder = format!("{}/{}_extracted", out_dir, stem);

        match extract_archive(
            window,
            job_id,
            input_path,
            &in_ext,
            &extract_folder,
            processes.clone(),
            cancelled.clone(),
            1.0,
        ) {
            ConvertResult::Done => {}
            other => return other,
        }

        let _ = window.emit(
            "job-done",
            JobDone {
                job_id: job_id.to_string(),
                output_path: extract_folder,
            },
        );
        return ConvertResult::DoneEmitted;
    }

    // Convert: extract to temp dir, repack to new format.
    let tmp_dir = format!("/tmp/fade_archive_{}", job_id);
    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        return ConvertResult::Error(e.to_string());
    }

    let extract_res = extract_archive(
        window,
        job_id,
        input_path,
        &in_ext,
        &tmp_dir,
        processes.clone(),
        cancelled.clone(),
        0.5,
    );
    match extract_res {
        ConvertResult::Done => {}
        other => {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            return other;
        }
    }

    let repack_res = repack_archive(
        window,
        job_id,
        &tmp_dir,
        output_path,
        &out_ext,
        opts,
        processes,
        cancelled.clone(),
    );
    let _ = std::fs::remove_dir_all(&tmp_dir);
    repack_res
}

/// Extract `input_path` into `dest_dir`. `progress_scale` is how much of the
/// overall job this step represents (1.0 for extract-only, 0.5 for convert).
#[allow(clippy::too_many_arguments)]
fn extract_archive(
    window: &Window,
    job_id: &str,
    input_path: &str,
    in_ext: &str,
    dest_dir: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
    progress_scale: f32,
) -> ConvertResult {
    // For rar/cbr, prefer `unar` (libre, handles modern RAR5 reliably);
    // fall back to 7z which can also read RAR.
    let use_unar = matches!(in_ext, "rar" | "cbr") && tool_in_path("unar");

    if use_unar {
        if let Err(e) = std::fs::create_dir_all(dest_dir) {
            return ConvertResult::Error(e.to_string());
        }
        let mut child = match Command::new("unar")
            .args(["-force-overwrite", "-o", dest_dir, input_path])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return ConvertResult::Error(format!("unar not found: {e}")),
        };
        let stderr = child.stderr.take();
        {
            let mut map = processes.lock();
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
        let _ = window.emit(
            "job-progress",
            JobProgress {
                job_id: job_id.to_string(),
                percent: 25.0 * progress_scale,
                message: "Extracting…".to_string(),
            },
        );
        let error_output = stderr_thread.join().unwrap_or_default();
        let child_opt = {
            let mut map = processes.lock();
            map.remove(job_id)
        };
        let success = match child_opt {
            Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        if cancelled.load(Ordering::SeqCst) {
            return ConvertResult::Cancelled;
        }
        if !success {
            return ConvertResult::Error(if error_output.trim().is_empty() {
                "unar extraction failed".to_string()
            } else {
                truncate_stderr(&error_output)
            });
        }
        return ConvertResult::Done;
    }

    // Default: 7z. Handles zip/7z/tar/gz/bz2/xz/rar/cbz/iso and partial dmg.
    let mut child = match Command::new(seven_zip_bin())
        .args(["x", input_path, &format!("-o{}", dest_dir), "-y"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ConvertResult::Error(if matches!(in_ext, "rar" | "cbr") {
                format!("Install `unar` or `7z` to read RAR archives: {e}")
            } else {
                format!("7z not found: {e}")
            });
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    {
        let mut map = processes.lock();
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
                        percent: pct * progress_scale,
                        message: format!("Extracting {}%", pct as u32),
                    },
                );
            }
        }
    }

    let error_output = stderr_thread.join().unwrap_or_default();
    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };
    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };
    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }
    if !success {
        return ConvertResult::Error(if error_output.trim().is_empty() {
            "7z extraction failed".to_string()
        } else {
            truncate_stderr(&error_output)
        });
    }
    ConvertResult::Done
}

#[allow(clippy::too_many_arguments)]
fn repack_archive(
    window: &Window,
    job_id: &str,
    src_dir: &str,
    output_path: &str,
    out_ext: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    match out_ext {
        "iso" => repack_iso(window, job_id, src_dir, output_path, processes, cancelled),
        "dmg" => repack_dmg(window, job_id, src_dir, output_path, processes, cancelled),
        _ => repack_with_7z(
            window,
            job_id,
            src_dir,
            output_path,
            opts,
            processes,
            cancelled,
        ),
    }
}

fn repack_with_7z(
    window: &Window,
    job_id: &str,
    src_dir: &str,
    output_path: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let mut repack_args: Vec<String> = vec![
        "a".to_string(),
        output_path.to_string(),
        format!("{}/*", src_dir),
    ];
    if let Some(level) = opts.archive_compression {
        repack_args.push(format!("-mx={}", level.min(9)));
    }
    let mut child = match Command::new(seven_zip_bin())
        .args(&repack_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("7z not found: {e}")),
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    {
        let mut map = processes.lock();
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
        let mut map = processes.lock();
        map.remove(job_id)
    };
    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };
    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }
    if !success {
        return ConvertResult::Error(if error_output.trim().is_empty() {
            "7z repack failed".to_string()
        } else {
            truncate_stderr(&error_output)
        });
    }
    ConvertResult::Done
}

fn repack_iso(
    window: &Window,
    job_id: &str,
    src_dir: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    if !tool_in_path("xorriso") {
        return ConvertResult::Error("ISO creation requires `xorriso`.\n\nInstall with:\n  macOS:   brew install xorriso\n  Linux:   apt install xorriso  (or equivalent)".to_string());
    }
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 60.0,
            message: "Building ISO…".to_string(),
        },
    );
    let mut child = match Command::new("xorriso")
        .args([
            "-as",
            "mkisofs",
            "-quiet",
            "-o",
            output_path,
            "-V",
            "FADE",
            src_dir,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("xorriso failed to start: {e}")),
    };
    let stderr = child.stderr.take();
    {
        let mut map = processes.lock();
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
    let error_output = stderr_thread.join().unwrap_or_default();
    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };
    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };
    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }
    if !success {
        return ConvertResult::Error(if error_output.trim().is_empty() {
            "xorriso failed to build ISO".to_string()
        } else {
            truncate_stderr(&error_output)
        });
    }
    ConvertResult::Done
}

fn repack_dmg(
    window: &Window,
    job_id: &str,
    src_dir: &str,
    output_path: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    // hdiutil is macOS-only; the caller has already gated on cfg(target_os).
    if !tool_in_path("hdiutil") {
        return ConvertResult::Error("DMG creation requires macOS `hdiutil`".to_string());
    }
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 60.0,
            message: "Building DMG…".to_string(),
        },
    );
    let mut child = match Command::new("hdiutil")
        .args([
            "create",
            "-quiet",
            "-volname",
            "Fade",
            "-srcfolder",
            src_dir,
            "-ov",
            "-format",
            "UDZO",
            output_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("hdiutil failed to start: {e}")),
    };
    let stderr = child.stderr.take();
    {
        let mut map = processes.lock();
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
    let error_output = stderr_thread.join().unwrap_or_default();
    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };
    let success = match child_opt {
        Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };
    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }
    if !success {
        return ConvertResult::Error(if error_output.trim().is_empty() {
            "hdiutil failed to build DMG".to_string()
        } else {
            truncate_stderr(&error_output)
        });
    }
    ConvertResult::Done
}

/// Parse 7z progress lines like "  7% - filename.ext"
fn parse_7z_percent(line: &str) -> Option<f32> {
    let trimmed = line.trim();
    let pct_end = trimmed.find('%')?;
    trimmed[..pct_end].trim().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_seven_zip_bin_prefers_7z_when_present() {
        assert_eq!(resolve_seven_zip_bin(|name| name == "7z"), "7z");
    }

    #[test]
    fn resolve_seven_zip_bin_falls_back_to_7zz() {
        assert_eq!(resolve_seven_zip_bin(|_| false), "7zz");
    }

    #[test]
    fn seven_zip_bin_memoizes() {
        let a = seven_zip_bin();
        let b = seven_zip_bin();
        assert_eq!(a, b);
        assert!(matches!(a, "7z" | "7zz"));
    }
}
