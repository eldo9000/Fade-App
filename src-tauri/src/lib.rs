use librewin_common::config::{read_presets, write_presets, FadePreset};
use librewin_common::media::media_type_for;
use librewin_common::{get_accent as lw_get_accent, get_theme as lw_get_theme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{command, Emitter, State, Window};

// ── AppState ───────────────────────────────────────────────────────────────────

pub struct AppState {
    pub processes: Arc<Mutex<HashMap<String, Child>>>,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct JobProgress {
    job_id: String,
    percent: f32,
    message: String,
}

#[derive(Serialize, Clone)]
struct JobDone {
    job_id: String,
    output_path: String,
}

#[derive(Serialize, Clone)]
struct JobError {
    job_id: String,
    message: String,
}

#[derive(Serialize, Clone)]
struct JobCancelled {
    job_id: String,
}

#[derive(Deserialize, Clone)]
pub struct ConvertOptions {
    pub output_format: String,
    pub output_dir: Option<String>,
    // Image
    pub resize_mode: Option<String>,
    pub resize_percent: Option<u32>,
    pub resize_width: Option<u32>,
    pub resize_height: Option<u32>,
    pub quality: Option<u32>,
    pub crop_x: Option<u32>,
    pub crop_y: Option<u32>,
    pub crop_width: Option<u32>,
    pub crop_height: Option<u32>,
    pub rotation: Option<u32>,
    pub flip_h: Option<bool>,
    pub flip_v: Option<bool>,
    pub auto_rotate: Option<bool>,
    // Video
    pub codec: Option<String>,
    pub resolution: Option<String>,
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
    pub remove_audio: Option<bool>,
    pub extract_audio: Option<bool>,
    pub audio_format: Option<String>,
    // Audio
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub normalize_loudness: Option<bool>,
    // Output naming
    pub output_suffix: Option<String>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            output_format: "mp4".to_string(),
            output_dir: None,
            resize_mode: None,
            resize_percent: None,
            resize_width: None,
            resize_height: None,
            quality: None,
            crop_x: None,
            crop_y: None,
            crop_width: None,
            crop_height: None,
            rotation: None,
            flip_h: None,
            flip_v: None,
            auto_rotate: None,
            codec: None,
            resolution: None,
            trim_start: None,
            trim_end: None,
            remove_audio: None,
            extract_audio: None,
            audio_format: None,
            bitrate: None,
            sample_rate: None,
            normalize_loudness: None,
            output_suffix: None,
        }
    }
}

#[derive(Serialize)]
pub struct FileInfo {
    pub duration_secs: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codec: Option<String>,
    pub format: Option<String>,
    pub file_size: u64,
    pub media_type: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Get duration from ffprobe JSON output; returns None if unavailable.
fn probe_duration(path: &str) -> Option<f64> {
    let out = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_format", path])
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let dur_str = json["format"]["duration"].as_str()?;
    dur_str.parse::<f64>().ok()
}

/// Build the output path: same dir as input (or output_dir), stem + suffix + new ext.
fn build_output_path(input: &str, new_ext: &str, output_dir: Option<&str>, suffix: &str) -> String {
    let p = Path::new(input);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let dir = output_dir.map(|d| d.to_string()).unwrap_or_else(|| {
        p.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string())
    });
    if suffix.is_empty() {
        format!("{}/{}.{}", dir, stem, new_ext)
    } else {
        format!("{}/{}_{}.{}", dir, stem, suffix, new_ext)
    }
}

/// Validate that a suffix only contains safe characters (alphanumeric, hyphen, underscore).
fn validate_suffix(suffix: &str) -> Result<(), String> {
    if suffix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(format!(
            "Invalid suffix '{}': only letters, digits, hyphens, and underscores allowed",
            suffix
        ))
    }
}

/// Parse out_time_ms line from ffmpeg -progress output to get elapsed seconds.
fn parse_out_time_ms(line: &str) -> Option<f64> {
    let val = line.strip_prefix("out_time_ms=")?;
    val.trim().parse::<f64>().ok().map(|ms| ms / 1_000_000.0)
}

/// Check whether a tool is available in PATH.
fn tool_available(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Append an entry to ~/.config/librewin/fade.log, keeping at most 100 lines.
fn write_fade_log(entry: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_path = format!("{}/.config/librewin/fade.log", home);
    let existing = std::fs::read_to_string(&log_path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    lines.push(entry.to_string());
    if lines.len() > 100 {
        let start = lines.len() - 100;
        lines.drain(0..start);
    }
    let _ = std::fs::write(&log_path, lines.join("\n") + "\n");
}

fn format_log_entry(job_id: &str, input_path: &str, status: &str, detail: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("[{}] {} {} {} {}", ts, job_id, input_path, status, detail)
}

/// Truncate stderr to the last 2000 chars (FFmpeg stderr is verbose).
fn truncate_stderr(s: &str) -> String {
    if s.len() > 2000 {
        s[s.len() - 2000..].to_string()
    } else {
        s.to_string()
    }
}

// ── Pure arg builders (testable, no I/O) ──────────────────────────────────────

pub fn build_image_magick_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = vec![input.to_string()];

    if opts.auto_rotate == Some(true) {
        args.push("-auto-orient".to_string());
    }

    if let (Some(cw), Some(ch)) = (opts.crop_width, opts.crop_height) {
        if cw > 0 && ch > 0 {
            let cx = opts.crop_x.unwrap_or(0);
            let cy = opts.crop_y.unwrap_or(0);
            args.push("-crop".to_string());
            args.push(format!("{}x{}+{}+{}", cw, ch, cx, cy));
            args.push("+repage".to_string());
        }
    }

    match opts.resize_mode.as_deref() {
        Some("percent") => {
            let pct = opts.resize_percent.unwrap_or(100);
            args.push("-resize".to_string());
            args.push(format!("{}%", pct));
        },
        Some("pixels") => {
            let w = opts.resize_width.unwrap_or(0);
            let h = opts.resize_height.unwrap_or(0);
            if w > 0 && h > 0 {
                args.push("-resize".to_string());
                args.push(format!("{}x{}", w, h));
            } else if w > 0 {
                args.push("-resize".to_string());
                args.push(format!("{}x", w));
            } else if h > 0 {
                args.push("-resize".to_string());
                args.push(format!("x{}", h));
            }
        },
        _ => {},
    }

    if let Some(deg) = opts.rotation {
        if deg == 90 || deg == 180 || deg == 270 {
            args.push("-rotate".to_string());
            args.push(deg.to_string());
        }
    }

    if opts.flip_v == Some(true) {
        args.push("-flip".to_string());
    }
    if opts.flip_h == Some(true) {
        args.push("-flop".to_string());
    }

    if let Some(q) = opts.quality {
        args.push("-quality".to_string());
        args.push(q.to_string());
    }

    args.push("-strip".to_string());
    args.push(output.to_string());
    args
}

pub fn build_ffmpeg_video_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["-y".to_string()];

    if let Some(ss) = opts.trim_start {
        args.extend(["-ss".to_string(), ss.to_string()]);
    }

    args.extend(["-i".to_string(), input.to_string()]);

    if let Some(t) = opts.trim_end {
        let end = if let Some(ss) = opts.trim_start {
            t - ss
        } else {
            t
        };
        args.extend(["-t".to_string(), end.to_string()]);
    }

    let codec = opts.codec.as_deref().unwrap_or("copy");
    if opts.extract_audio == Some(true) {
        args.push("-vn".to_string());
    } else if opts.remove_audio == Some(true) {
        args.push("-an".to_string());
        if codec == "copy" {
            args.extend(["-vcodec".to_string(), "copy".to_string()]);
        } else {
            args.extend(ffmpeg_video_codec_args(codec));
        }
    } else if codec == "copy" {
        args.extend(["-c".to_string(), "copy".to_string()]);
    } else {
        args.extend(ffmpeg_video_codec_args(codec));
    }

    if let Some(res) = &opts.resolution {
        if res != "original" && opts.extract_audio != Some(true) {
            let scale = resolution_to_scale(res);
            args.extend(["-vf".to_string(), scale]);
        }
    }

    if opts.remove_audio != Some(true) {
        if let Some(br) = opts.bitrate {
            args.extend(["-b:a".to_string(), format!("{}k", br)]);
        }
        if let Some(sr) = opts.sample_rate {
            args.extend(["-ar".to_string(), sr.to_string()]);
        }
    }

    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}

pub fn build_ffmpeg_audio_args(input: &str, output: &str, opts: &ConvertOptions) -> Vec<String> {
    let mut args: Vec<String> = vec!["-y".to_string()];

    if let Some(ss) = opts.trim_start {
        args.extend(["-ss".to_string(), ss.to_string()]);
    }

    args.extend(["-i".to_string(), input.to_string()]);

    if let Some(t) = opts.trim_end {
        let end = if let Some(ss) = opts.trim_start {
            t - ss
        } else {
            t
        };
        args.extend(["-t".to_string(), end.to_string()]);
    }

    args.push("-vn".to_string());

    if let Some(br) = opts.bitrate {
        args.extend(["-b:a".to_string(), format!("{}k", br)]);
    }
    if let Some(sr) = opts.sample_rate {
        args.extend(["-ar".to_string(), sr.to_string()]);
    }

    if opts.normalize_loudness == Some(true) {
        args.extend(["-af".to_string(), "loudnorm".to_string()]);
    }

    args.extend([
        "-progress".to_string(),
        "pipe:1".to_string(),
        "-nostats".to_string(),
    ]);

    args.push(output.to_string());
    args
}

fn ffmpeg_video_codec_args(codec: &str) -> Vec<String> {
    match codec {
        "h264" => vec![
            "-vcodec".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
        ],
        "h265" => vec![
            "-vcodec".to_string(),
            "libx265".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
        ],
        "vp9" => vec![
            "-vcodec".to_string(),
            "libvpx-vp9".to_string(),
            "-b:v".to_string(),
            "0".to_string(),
            "-crf".to_string(),
            "33".to_string(),
        ],
        "av1" => vec![
            "-vcodec".to_string(),
            "libaom-av1".to_string(),
            "-crf".to_string(),
            "30".to_string(),
            "-b:v".to_string(),
            "0".to_string(),
        ],
        _ => vec!["-c".to_string(), "copy".to_string()],
    }
}

fn resolution_to_scale(res: &str) -> String {
    match res {
        "1920x1080" => {
            "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        "1280x720" => {
            "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        "854x480" => {
            "scale=854:480:force_original_aspect_ratio=decrease,pad=854:480:(ow-iw)/2:(oh-ih)/2"
                .to_string()
        },
        other => format!("scale={}", other),
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Return file info (duration, dimensions, codec, media type, size).
#[command]
fn get_file_info(path: String) -> Result<FileInfo, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    let file_size = p.metadata().map(|m| m.len()).unwrap_or(0);
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mtype = media_type_for(&ext);

    if mtype == "image" {
        let out = Command::new("identify")
            .args(["-format", "%wx%h\n", &path])
            .output()
            .map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&out.stdout);
        let dims: Vec<&str> = s.trim().splitn(2, 'x').collect();
        let width = dims.first().and_then(|v| v.parse().ok());
        let height = dims.get(1).and_then(|v| v.parse().ok());
        return Ok(FileInfo {
            duration_secs: None,
            width,
            height,
            codec: None,
            format: Some(ext.to_string()),
            file_size,
            media_type: "image".to_string(),
        });
    }

    let out = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &path,
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;

    let duration_secs = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok());
    let format = json["format"]["format_name"]
        .as_str()
        .map(|s| s.split(',').next().unwrap_or(s).to_string());

    let mut width = None;
    let mut height = None;
    let mut codec = None;

    if let Some(streams) = json["streams"].as_array() {
        for stream in streams {
            let ct = stream["codec_type"].as_str().unwrap_or("");
            if ct == "video" {
                width = stream["width"].as_u64().map(|v| v as u32);
                height = stream["height"].as_u64().map(|v| v as u32);
                codec = stream["codec_name"].as_str().map(|s| s.to_string());
                break;
            }
            if ct == "audio" && codec.is_none() {
                codec = stream["codec_name"].as_str().map(|s| s.to_string());
            }
        }
    }

    Ok(FileInfo {
        duration_secs,
        width,
        height,
        codec,
        format,
        file_size,
        media_type: mtype.to_string(),
    })
}

/// Convert a media file. Runs in a background thread and emits progress events.
/// Events emitted: job-progress, job-done, job-error, job-cancelled.
#[command]
fn convert_file(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    input_path: String,
    options: ConvertOptions,
) -> Result<(), String> {
    let p = Path::new(&input_path);
    if !p.exists() || !p.is_file() {
        return Err(format!(
            "File not found or not a regular file: {}",
            input_path
        ));
    }

    let ext = options.output_format.to_lowercase();

    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(format!("Invalid output format: {}", options.output_format));
    }

    let mtype = media_type_for(&ext);
    if mtype == "unknown" {
        return Err(format!(
            "Unsupported output format: {}",
            options.output_format
        ));
    }

    let suffix = options.output_suffix.as_deref().unwrap_or("converted");
    validate_suffix(suffix)?;

    let output_path = build_output_path(&input_path, &ext, options.output_dir.as_deref(), suffix);

    // Register cancellation flag before spawning the thread
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock().unwrap();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    // Clone arcs so they can be moved into the thread
    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let result = match mtype {
            "image" => run_image_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "video" => run_video_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "audio" => run_audio_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            _ => Err("Unsupported format".to_string()),
        };

        // Clean up cancellation registry entry
        {
            let mut map = cancellations.lock().unwrap();
            map.remove(&job_id);
        }

        let output_path_clone = output_path.clone();
        match result {
            Ok(()) => {
                write_fade_log(&format_log_entry(
                    &job_id,
                    &input_path,
                    "done",
                    &output_path,
                ));
                let _ = window.emit(
                    "job-done",
                    JobDone {
                        job_id,
                        output_path,
                    },
                );
            },
            Err(msg) if msg == "CANCELLED" => {
                let _ = std::fs::remove_file(&output_path_clone);
                write_fade_log(&format_log_entry(&job_id, &input_path, "cancelled", ""));
                let _ = window.emit("job-cancelled", JobCancelled { job_id });
            },
            Err(msg) => {
                let first_line = msg.lines().next().unwrap_or("").to_string();
                write_fade_log(&format_log_entry(
                    &job_id,
                    &input_path,
                    "error",
                    &first_line,
                ));
                let _ = window.emit(
                    "job-error",
                    JobError {
                        job_id,
                        message: msg,
                    },
                );
            },
        }
    });

    Ok(())
}

/// Cancel a running job by killing its subprocess.
#[command]
fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    // Set the cancelled flag first so the background thread knows why it stopped
    {
        let map = state.cancellations.lock().unwrap();
        if let Some(flag) = map.get(&job_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
    // Kill and remove the child process
    {
        let mut map = state.processes.lock().unwrap();
        if let Some(child) = map.get_mut(&job_id) {
            let _ = child.kill();
        }
        map.remove(&job_id);
    }
    Ok(())
}

/// Check whether required external tools are available in PATH.
#[command]
fn check_tools() -> serde_json::Value {
    serde_json::json!({
        "ffmpeg":  tool_available("ffmpeg"),
        "ffprobe": tool_available("ffprobe"),
        "magick":  tool_available("magick"),
    })
}

// ── Image conversion (ImageMagick) ────────────────────────────────────────────

fn run_image_convert(
    window: &Window,
    job_id: &str,
    _input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress {
            job_id: job_id.to_string(),
            percent: 0.0,
            message: "Converting image…".to_string(),
        },
    );

    // build_image_magick_args uses _input via opts indirectly; we need the raw paths
    // Re-derive args using the actual input embedded in opts via the caller's input param.
    // We pass _input as first element in build_image_magick_args, so call it properly:
    let args = {
        // build_image_magick_args signature: (input, output, opts)
        build_image_magick_args(_input, output, opts)
    };

    let mut child = Command::new("magick")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ImageMagick not found: {e}"))?;

    let stderr = child.stderr.take();

    {
        let mut map = processes.lock().unwrap();
        map.insert(job_id.to_string(), child);
    } // lock dropped

    // Collect stderr (blocks until process exits or pipe closes after kill)
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

    // Remove child from map and wait for exit status
    let child_opt = {
        let mut map = processes.lock().unwrap();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false, // killed and removed by cancel_job
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
    } else {
        Err(if stderr_content.trim().is_empty() {
            "ImageMagick convert failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}

// ── Video conversion (FFmpeg) ─────────────────────────────────────────────────

fn run_video_convert(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input);
    let args = build_ffmpeg_video_args(input, output, opts);

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    // Extract pipes before storing child in map
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock().unwrap();
        map.insert(job_id.to_string(), child);
    } // lock dropped immediately

    // Drain stderr concurrently while reading stdout for progress
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

    // Stream progress events from stdout
    if let Some(stdout) = stdout {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(elapsed) = parse_out_time_ms(&line) {
                let percent = if let Some(dur) = duration {
                    ((elapsed / dur) * 100.0).min(99.0) as f32
                } else {
                    0.0
                };
                let _ = window.emit(
                    "job-progress",
                    JobProgress {
                        job_id: job_id.to_string(),
                        percent,
                        message: format!("{:.0}s elapsed", elapsed),
                    },
                );
            }
        }
    }

    let error_output = stderr_thread.join().unwrap_or_default();

    // Remove child from map and wait for exit status
    let child_opt = {
        let mut map = processes.lock().unwrap();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false, // killed and removed by cancel_job
    };

    if cancelled.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if success {
        Ok(())
    } else {
        Err(if error_output.trim().is_empty() {
            "FFmpeg conversion failed".to_string()
        } else {
            truncate_stderr(&error_output)
        })
    }
}

// ── Audio conversion (FFmpeg) ─────────────────────────────────────────────────

fn run_audio_convert(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let duration = probe_duration(input);
    let args = build_ffmpeg_audio_args(input, output, opts);

    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock().unwrap();
        map.insert(job_id.to_string(), child);
    } // lock dropped

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
            if let Some(elapsed) = parse_out_time_ms(&line) {
                let percent = if let Some(dur) = duration {
                    ((elapsed / dur) * 100.0).min(99.0) as f32
                } else {
                    0.0
                };
                let _ = window.emit(
                    "job-progress",
                    JobProgress {
                        job_id: job_id.to_string(),
                        percent,
                        message: format!("{:.0}s elapsed", elapsed),
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
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    if cancelled.load(Ordering::SeqCst) {
        return Err("CANCELLED".to_string());
    }

    if success {
        Ok(())
    } else {
        Err(if error_output.trim().is_empty() {
            "FFmpeg audio conversion failed".to_string()
        } else {
            truncate_stderr(&error_output)
        })
    }
}

// ── Theme / accent ────────────────────────────────────────────────────────────

#[command]
fn get_theme() -> String {
    lw_get_theme()
}

#[command]
fn get_accent() -> String {
    lw_get_accent()
}

// ── Custom presets ────────────────────────────────────────────────────────────

#[command]
fn list_presets() -> Vec<FadePreset> {
    read_presets()
}

#[command]
fn save_preset(
    name: String,
    media_type: String,
    output_format: String,
    quality: Option<u32>,
    codec: Option<String>,
    bitrate: Option<u32>,
    sample_rate: Option<u32>,
) -> Result<FadePreset, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Preset name cannot be empty".to_string());
    }
    if name.len() > 64 {
        return Err("Preset name too long (max 64 chars)".to_string());
    }

    let preset = FadePreset {
        id: uuid_v4().to_string(),
        name,
        media_type,
        output_format,
        quality,
        codec,
        bitrate,
        sample_rate,
    };

    let mut presets = read_presets();
    presets.push(preset.clone());
    write_presets(&presets)?;
    Ok(preset)
}

#[command]
fn delete_preset(id: String) -> Result<(), String> {
    let mut presets = read_presets();
    let before = presets.len();
    presets.retain(|p| p.id != id);
    if presets.len() == before {
        return Err(format!("Preset not found: {id}"));
    }
    write_presets(&presets)
}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            processes: Arc::new(Mutex::new(HashMap::new())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        })
        .invoke_handler(tauri::generate_handler![
            get_file_info,
            convert_file,
            cancel_job,
            check_tools,
            get_theme,
            get_accent,
            list_presets,
            save_preset,
            delete_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running fade");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_output_path ─────────────────────────────────────────────────────

    #[test]
    fn build_output_path_with_suffix() {
        let result = build_output_path("/home/user/video.mp4", "mkv", None, "converted");
        assert_eq!(result, "/home/user/video_converted.mkv");
    }

    #[test]
    fn build_output_path_empty_suffix() {
        let result = build_output_path("/home/user/video.mp4", "mkv", None, "");
        assert_eq!(result, "/home/user/video.mkv");
    }

    #[test]
    fn build_output_path_custom_output_dir() {
        let result =
            build_output_path("/home/user/video.mp4", "mp3", Some("/tmp/out"), "converted");
        assert_eq!(result, "/tmp/out/video_converted.mp3");
    }

    // ── validate_suffix ───────────────────────────────────────────────────────

    #[test]
    fn validate_suffix_accepts_safe_chars() {
        assert!(validate_suffix("converted").is_ok());
        assert!(validate_suffix("my-export_v2").is_ok());
        assert!(validate_suffix("").is_ok());
    }

    #[test]
    fn validate_suffix_rejects_unsafe_chars() {
        assert!(validate_suffix("bad/path").is_err());
        assert!(validate_suffix("has space").is_err());
        assert!(validate_suffix("dot.dot").is_err());
        assert!(validate_suffix("semi;colon").is_err());
    }

    // ── media_type_for ────────────────────────────────────────────────────────

    #[test]
    fn media_type_for_image() {
        assert_eq!(media_type_for("jpg"), "image");
        assert_eq!(media_type_for("png"), "image");
        assert_eq!(media_type_for("webp"), "image");
        assert_eq!(media_type_for("heic"), "image");
    }

    #[test]
    fn media_type_for_video() {
        assert_eq!(media_type_for("mp4"), "video");
        assert_eq!(media_type_for("mkv"), "video");
        assert_eq!(media_type_for("webm"), "video");
    }

    #[test]
    fn media_type_for_audio() {
        assert_eq!(media_type_for("mp3"), "audio");
        assert_eq!(media_type_for("flac"), "audio");
        assert_eq!(media_type_for("wav"), "audio");
    }

    #[test]
    fn media_type_for_case_insensitive() {
        assert_eq!(media_type_for("JPG"), "image");
        assert_eq!(media_type_for("MP4"), "video");
        assert_eq!(media_type_for("FLAC"), "audio");
    }

    #[test]
    fn media_type_for_unknown() {
        assert_eq!(media_type_for("xyz"), "unknown");
        assert_eq!(media_type_for(""), "unknown");
    }

    // ── parse_out_time_ms ─────────────────────────────────────────────────────

    #[test]
    fn parse_out_time_ms_parses_microseconds() {
        assert_eq!(parse_out_time_ms("out_time_ms=1000000"), Some(1.0));
        assert_eq!(parse_out_time_ms("out_time_ms=500000"), Some(0.5));
        assert_eq!(parse_out_time_ms("out_time_ms=0"), Some(0.0));
    }

    #[test]
    fn parse_out_time_ms_ignores_other_lines() {
        assert_eq!(parse_out_time_ms("frame=42"), None);
        assert_eq!(parse_out_time_ms("speed=1.0x"), None);
        assert_eq!(parse_out_time_ms(""), None);
    }

    // ── build_image_magick_args ───────────────────────────────────────────────

    #[test]
    fn image_args_basic_quality_strip() {
        let opts = ConvertOptions {
            quality: Some(85),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.webp", &opts);
        assert_eq!(args[0], "in.jpg");
        assert_eq!(args.last().unwrap(), "out.webp");
        assert!(args.contains(&"-quality".to_string()));
        assert!(args.contains(&"85".to_string()));
        assert!(args.contains(&"-strip".to_string()));
    }

    #[test]
    fn image_args_auto_rotate() {
        let opts = ConvertOptions {
            auto_rotate: Some(true),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-auto-orient".to_string()));
    }

    #[test]
    fn image_args_resize_percent() {
        let opts = ConvertOptions {
            resize_mode: Some("percent".to_string()),
            resize_percent: Some(50),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-resize".to_string()));
        assert!(args.contains(&"50%".to_string()));
    }

    #[test]
    fn image_args_resize_pixels() {
        let opts = ConvertOptions {
            resize_mode: Some("pixels".to_string()),
            resize_width: Some(1920),
            resize_height: Some(1080),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-resize".to_string()));
        assert!(args.contains(&"1920x1080".to_string()));
    }

    #[test]
    fn image_args_rotation() {
        let opts = ConvertOptions {
            rotation: Some(90),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-rotate".to_string()));
        assert!(args.contains(&"90".to_string()));
    }

    #[test]
    fn image_args_flip() {
        let opts = ConvertOptions {
            flip_h: Some(true),
            flip_v: Some(true),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-flip".to_string()));
        assert!(args.contains(&"-flop".to_string()));
    }

    #[test]
    fn image_args_crop() {
        let opts = ConvertOptions {
            crop_width: Some(800),
            crop_height: Some(600),
            crop_x: Some(10),
            crop_y: Some(20),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(args.contains(&"-crop".to_string()));
        assert!(args.contains(&"800x600+10+20".to_string()));
        assert!(args.contains(&"+repage".to_string()));
    }

    // ── build_ffmpeg_video_args ───────────────────────────────────────────────

    #[test]
    fn video_args_trim_produces_ss_and_t() {
        let opts = ConvertOptions {
            output_format: "mp4".to_string(),
            trim_start: Some(10.0),
            trim_end: Some(30.0),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        let ss_idx = args.iter().position(|a| a == "-ss").expect("-ss missing");
        assert_eq!(args[ss_idx + 1], "10");
        let t_idx = args.iter().position(|a| a == "-t").expect("-t missing");
        // duration = trim_end - trim_start = 20s
        assert_eq!(args[t_idx + 1], "20");
    }

    #[test]
    fn video_args_h264_codec() {
        let opts = ConvertOptions {
            output_format: "mp4".to_string(),
            codec: Some("h264".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-vcodec".to_string()));
    }

    #[test]
    fn video_args_remove_audio() {
        let opts = ConvertOptions {
            output_format: "mp4".to_string(),
            remove_audio: Some(true),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(args.contains(&"-an".to_string()));
    }

    #[test]
    fn video_args_extract_audio_uses_vn() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            extract_audio: Some(true),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp3", &opts);
        assert!(args.contains(&"-vn".to_string()));
    }

    #[test]
    fn video_args_resolution_scale() {
        let opts = ConvertOptions {
            output_format: "mp4".to_string(),
            resolution: Some("1280x720".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(args.contains(&"-vf".to_string()));
        let vf_idx = args.iter().position(|a| a == "-vf").unwrap();
        assert!(args[vf_idx + 1].contains("1280:720"));
    }

    #[test]
    fn video_args_has_progress_and_output() {
        let opts = ConvertOptions {
            output_format: "mp4".to_string(),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(args.contains(&"-progress".to_string()));
        assert!(args.contains(&"pipe:1".to_string()));
        assert_eq!(args.last().unwrap(), "out.mp4");
    }

    // ── build_ffmpeg_audio_args ───────────────────────────────────────────────

    #[test]
    fn audio_args_bitrate_and_sample_rate() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            bitrate: Some(192),
            sample_rate: Some(44100),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        let br_idx = args.iter().position(|a| a == "-b:a").expect("-b:a missing");
        assert_eq!(args[br_idx + 1], "192k");
        let ar_idx = args.iter().position(|a| a == "-ar").expect("-ar missing");
        assert_eq!(args[ar_idx + 1], "44100");
    }

    #[test]
    fn audio_args_normalize_loudness() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            normalize_loudness: Some(true),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        assert!(args.contains(&"-af".to_string()));
        assert!(args.contains(&"loudnorm".to_string()));
    }

    #[test]
    fn audio_args_trim() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            trim_start: Some(5.0),
            trim_end: Some(60.0),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"-t".to_string()));
        let t_idx = args.iter().position(|a| a == "-t").unwrap();
        assert_eq!(args[t_idx + 1], "55"); // 60 - 5
    }

    #[test]
    fn audio_args_always_has_vn() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        assert!(args.contains(&"-vn".to_string()));
    }

    // ── Integration test (requires real tools, opt-in with --ignored) ─────────

    #[test]
    #[ignore]
    fn integration_image_convert_jpeg_to_png() {
        let input = "src/tests/fixtures/1px.jpg";
        let output = "/tmp/fade_test_out.png";
        let opts = ConvertOptions {
            output_format: "png".to_string(),
            ..Default::default()
        };
        let args = build_image_magick_args(input, output, &opts);
        let status = Command::new("magick")
            .args(&args)
            .status()
            .expect("magick not found");
        assert!(status.success());
        assert!(Path::new(output).exists());
        let _ = std::fs::remove_file(output);
    }
}
