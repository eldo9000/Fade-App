//! Silence remover — cuts silent stretches via the `silenceremove` filter.
//!
//! Params mirror the UI: threshold in dB, minimum silence duration, and a
//! pad in ms preserved around each surviving segment. Re-encodes audio
//! because `silenceremove` is a filter graph; video stays stream-copied
//! when present.

use super::run_ffmpeg;
use crate::probe_duration;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use parking_lot::Mutex;
use tauri::Window;

#[allow(clippy::too_many_arguments)]
pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    threshold_db: f64,
    min_silence_s: f64,
    pad_ms: u32,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input_path);

    // start_periods=1 / stop_periods=-1 → remove every silent stretch.
    // stop_threshold is dB below reference; silenceremove expects a linear
    // amplitude for the `_threshold` fields but also accepts "dB" suffix.
    let pad_s = pad_ms as f64 / 1000.0;
    let filter = format!(
        "silenceremove=start_periods=1:start_silence={pad:.3}:start_threshold={th}dB:stop_periods=-1:stop_silence={pad:.3}:stop_threshold={th}dB:stop_duration={dur:.3}",
        pad = pad_s,
        th = threshold_db,
        dur = min_silence_s,
    );

    let args = vec![
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string(),
        "-af".to_string(),
        filter,
        "-c:v".to_string(),
        "copy".to_string(),
        "-progress".to_string(),
        "pipe:1".to_string(),
        output_path.to_string(),
    ];
    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}
