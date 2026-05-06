//! Per-frame MD5 hashes via `-f framemd5`.
//!
//! If `diff_path` is provided, both files are hashed and the first mismatching
//! line index is returned along with both hash lists (capped).

use crate::{validate_input_path, AppState};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct FrameHash {
    pub idx: usize,
    pub hash: String,
}

#[derive(Serialize, Clone)]
pub struct FrameMd5Result {
    pub hashes: Vec<FrameHash>,
    pub first_divergence: Option<usize>,
}

#[derive(Serialize, Clone)]
pub struct FrameMd5JobResult {
    pub job_id: String,
    pub data: Option<FrameMd5Result>,
    pub error: Option<String>,
    pub cancelled: bool,
}

/// Spawn ffmpeg to emit framemd5 to stdout. Registers the child under
/// `job_id` so it can be cancelled. Returns the hash lines on success.
fn hash_file_registered(
    input_path: &str,
    stream: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<String>, String> {
    let map_arg = match stream {
        "audio" => vec!["-map".to_string(), "0:a:0".to_string()],
        "video" => vec!["-map".to_string(), "0:v:0".to_string()],
        _ => vec![],
    };
    let mut args = vec![
        "-hide_banner".to_string(),
        "-nostats".to_string(),
        "-i".to_string(),
        input_path.to_string(),
    ];
    args.extend(map_arg);
    args.extend(["-f".to_string(), "framemd5".to_string(), "-".to_string()]);

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    if cancelled.load(Ordering::SeqCst) {
        let mut map = processes.lock();
        if let Some(child) = map.get_mut(job_id) {
            let _ = child.kill();
        }
    }

    let stdout_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stdout {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
        buf
    });

    let stdout_lines = stdout_thread.join().unwrap_or_default();
    let stderr_text = stderr_thread.join().unwrap_or_default();

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

    if !success {
        return Err(if stderr_text.trim().is_empty() {
            "framemd5 failed".to_string()
        } else {
            stderr_text
        });
    }

    // framemd5 lines: 0, 0, 0, 1, 6220800, <md5>
    Ok(stdout_lines
        .into_iter()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next().map(|s| s.trim().to_string()))
        .filter(|h| h.len() == 32)
        .collect())
}

fn compute_framemd5(
    input_path: &str,
    stream: &str,
    diff_path: Option<&str>,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<FrameMd5Result, String> {
    // `both` isn't directly supported by -f framemd5 in a single pass without
    // losing which hash belongs to which stream; hash video then audio and
    // concat. Keeps the return shape flat.
    let hashes_a = if stream == "both" {
        let mut v = hash_file_registered(
            input_path,
            "video",
            Arc::clone(&processes),
            job_id,
            Arc::clone(&cancelled),
        )?;
        v.extend(hash_file_registered(
            input_path,
            "audio",
            Arc::clone(&processes),
            job_id,
            Arc::clone(&cancelled),
        )?);
        v
    } else {
        hash_file_registered(
            input_path,
            stream,
            Arc::clone(&processes),
            job_id,
            Arc::clone(&cancelled),
        )?
    };

    if let Some(diff) = diff_path {
        let hashes_b = if stream == "both" {
            let mut v = hash_file_registered(
                diff,
                "video",
                Arc::clone(&processes),
                job_id,
                Arc::clone(&cancelled),
            )?;
            v.extend(hash_file_registered(
                diff,
                "audio",
                Arc::clone(&processes),
                job_id,
                Arc::clone(&cancelled),
            )?);
            v
        } else {
            hash_file_registered(
                diff,
                stream,
                Arc::clone(&processes),
                job_id,
                Arc::clone(&cancelled),
            )?
        };
        let first_divergence = hashes_a
            .iter()
            .zip(hashes_b.iter())
            .position(|(a, b)| a != b)
            .or_else(|| {
                if hashes_a.len() != hashes_b.len() {
                    Some(hashes_a.len().min(hashes_b.len()))
                } else {
                    None
                }
            });
        let cap = 256;
        let hashes: Vec<FrameHash> = hashes_a
            .into_iter()
            .enumerate()
            .take(cap)
            .map(|(idx, hash)| FrameHash { idx, hash })
            .collect();
        Ok(FrameMd5Result {
            hashes,
            first_divergence,
        })
    } else {
        let cap = 256;
        let hashes: Vec<FrameHash> = hashes_a
            .into_iter()
            .enumerate()
            .take(cap)
            .map(|(idx, hash)| FrameHash { idx, hash })
            .collect();
        Ok(FrameMd5Result {
            hashes,
            first_divergence: None,
        })
    }
}

#[tauri::command]
pub fn analyze_framemd5(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    input_path: String,
    stream: String, // "video" | "audio" | "both"
    diff_path: Option<String>,
) -> Result<(), String> {
    validate_input_path(&input_path)?;
    if let Some(path) = &diff_path {
        validate_input_path(path)?;
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let result = compute_framemd5(
            &input_path,
            &stream,
            diff_path.as_deref(),
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let payload = match result {
            Ok(data) => FrameMd5JobResult {
                job_id: job_id.clone(),
                data: Some(data),
                error: None,
                cancelled: false,
            },
            Err(msg) if msg == "CANCELLED" => FrameMd5JobResult {
                job_id: job_id.clone(),
                data: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => FrameMd5JobResult {
                job_id: job_id.clone(),
                data: None,
                error: Some(msg),
                cancelled: false,
            },
        };

        let _ = window.emit(&format!("analysis-result:{}", job_id), payload);
    });

    Ok(())
}
