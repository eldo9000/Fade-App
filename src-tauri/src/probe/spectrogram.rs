use crate::operations::analysis::run_ffmpeg_capture_registered;
use crate::{truncate_stderr, AppState};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{command, Emitter, State, Window};

#[derive(Serialize, Clone)]
pub struct SpectrogramResult {
    pub job_id: String,
    /// Base64-encoded PNG bytes.
    pub data: Option<String>,
    pub error: Option<String>,
    pub cancelled: bool,
}

/// Spawn ffmpeg with piped stdout to capture the PNG payload, register the
/// child under `job_id` so `cancel_job` can kill it, then read stdout to
/// completion. Mirrors `run_ffmpeg_capture_registered` but keeps stdout bytes
/// (the PNG) rather than discarding them.
fn run_ffmpeg_capture_stdout_registered(
    args: &[String],
    processes: Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let mut child = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let stdout_thread = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        if let Some(s) = stdout {
            let mut reader = BufReader::new(s);
            reader.read_to_end(&mut buf)?;
        }
        Ok(buf)
    });

    let stderr_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        if let Some(mut s) = stderr {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Close the cancel TOCTOU window.
    if cancelled.load(Ordering::SeqCst) {
        let mut map = processes.lock();
        if let Some(child) = map.get_mut(job_id) {
            let _ = child.kill();
        }
    }

    let stdout_bytes = stdout_thread
        .join()
        .unwrap_or_else(|_| Ok(Vec::new()))
        .unwrap_or_default();
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

    if success {
        Ok(stdout_bytes)
    } else {
        Err(if stderr_text.trim().is_empty() {
            "spectrogram failed".to_string()
        } else {
            truncate_stderr(&stderr_text)
        })
    }
}

/// Render a rainbow spectrogram PNG via ffmpeg showspectrumpic and emit it as
/// base64 on `analysis-result:{job_id}`. Uses image2pipe + png codec to write
/// the PNG directly to stdout — no temp files.
#[command]
pub fn get_spectrogram(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    path: String,
) -> Result<(), String> {
    // Register cancellation flag before spawning the thread.
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let args = vec![
            "-i".to_string(),
            path,
            "-lavfi".to_string(),
            "showspectrumpic=s=800x200:legend=0:color=magma:scale=log:fscale=log".to_string(),
            "-frames:v".to_string(),
            "1".to_string(),
            "-f".to_string(),
            "image2pipe".to_string(),
            "-vcodec".to_string(),
            "png".to_string(),
            "-".to_string(),
        ];

        let result = run_ffmpeg_capture_stdout_registered(
            &args,
            Arc::clone(&processes),
            &job_id,
            Arc::clone(&cancelled),
        );

        // Clean up cancellation registry entry.
        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let payload = match result {
            Ok(bytes) if bytes.is_empty() => SpectrogramResult {
                job_id: job_id.clone(),
                data: None,
                error: Some("spectrogram produced no output".to_string()),
                cancelled: false,
            },
            Ok(bytes) => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                SpectrogramResult {
                    job_id: job_id.clone(),
                    data: Some(b64),
                    error: None,
                    cancelled: false,
                }
            }
            Err(msg) if msg == "CANCELLED" => SpectrogramResult {
                job_id: job_id.clone(),
                data: None,
                error: None,
                cancelled: true,
            },
            Err(msg) => SpectrogramResult {
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

// Keep `run_ffmpeg_capture_registered` import "used" for symmetry in case
// future edits switch to the shared helper.
#[allow(dead_code)]
fn _unused_ref(
    args: &[String],
    processes: Arc<Mutex<HashMap<String, Child>>>,
    job_id: &str,
    cancelled: Arc<AtomicBool>,
) -> Result<String, String> {
    run_ffmpeg_capture_registered(args, processes, job_id, cancelled)
}
