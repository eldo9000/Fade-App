use librewin_common::config::{read_presets, write_presets, FadePreset};
#[allow(unused_imports)]
use librewin_common::media::media_type_for; // used in tests via super::*
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
    pub normalize_lufs:     Option<f64>, // LUFS target (e.g. -16.0), None = default -16.0
    pub normalize_true_peak: Option<f64>, // dBTP ceiling (e.g. -1.0), None = default -1.0
    // DSP
    pub dsp_highpass_freq:  Option<f64>, // Hz — Butterworth 2-pole highpass, None = off
    pub dsp_lowpass_freq:   Option<f64>, // Hz — Butterworth 2-pole lowpass,  None = off
    pub dsp_stereo_width:   Option<f64>, // −100=mono  0=no change  +100=wide, None = off
    pub dsp_limiter_db:     Option<f64>, // dBFS ceiling (e.g. -1.0),          None = off
    // Data
    pub pretty_print: Option<bool>,
    pub csv_delimiter: Option<String>,
    // Archive
    pub archive_operation: Option<String>,
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
            normalize_lufs: None,
            normalize_true_peak: None,
            dsp_highpass_freq: None,
            dsp_lowpass_freq: None,
            dsp_stereo_width: None,
            dsp_limiter_db: None,
            pretty_print: None,
            csv_delimiter: None,
            archive_operation: None,
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

/// Classify a file extension into a media type, covering all types Fade supports.
fn classify_ext(ext: &str) -> &'static str {
    match ext {
        "jpg"|"jpeg"|"png"|"webp"|"tiff"|"tif"|"bmp"|"gif"|"avif"|
        "heic"|"heif"|"psd"|"svg"|"ico"|"raw"|"cr2"|"nef"|"arw"|"dng" => "image",
        "mp4"|"mkv"|"webm"|"avi"|"mov"|"m4v"|"flv"|"wmv"|"ts"|
        "mpg"|"mpeg"|"3gp"|"ogv" => "video",
        "mp3"|"wav"|"flac"|"ogg"|"aac"|"opus"|"m4a"|"wma"|"aiff" => "audio",
        "csv"|"json"|"xml"|"yaml"|"yml"|"toml"|"tsv"|"ndjson"|"jsonl" => "data",
        "md"|"markdown"|"html"|"htm"|"txt" => "document",
        "zip"|"7z"|"tar"|"gz"|"bz2"|"xz"|"tgz"|"rar" => "archive",
        _ => "unknown",
    }
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

    // Build DSP filter chain — order: filters → stereo width → loudnorm → limiter
    let mut filters: Vec<String> = Vec::new();

    if let Some(freq) = opts.dsp_highpass_freq {
        if freq > 0.0 {
            filters.push(format!("highpass=f={freq:.1}:p=2"));
        }
    }
    if let Some(freq) = opts.dsp_lowpass_freq {
        if freq > 0.0 {
            filters.push(format!("lowpass=f={freq:.1}:p=2"));
        }
    }
    if let Some(width_pct) = opts.dsp_stereo_width {
        // width_pct: −100 (mono) … 0 (no change) … +100 (wide)
        // extrastereo expects a multiplier: m = 1 + pct/100
        let m = 1.0 + width_pct / 100.0;
        if m.abs() > 0.01 {
            filters.push(format!("extrastereo=m={m:.3}"));
        }
    }
    if opts.normalize_loudness == Some(true) {
        let lufs = opts.normalize_lufs.unwrap_or(-16.0);
        let tp   = opts.normalize_true_peak.unwrap_or(-1.0);
        filters.push(format!("loudnorm=I={lufs:.1}:TP={tp:.1}:LRA=11"));
    }
    if let Some(db) = opts.dsp_limiter_db {
        let linear = 10.0_f64.powf(db / 20.0);
        filters.push(format!("alimiter=limit={linear:.6}:attack=5:release=50"));
    }

    if !filters.is_empty() {
        args.extend(["-af".to_string(), filters.join(",")]);
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
    let mtype = classify_ext(&ext);

    // Data, document, and archive files don't need ffprobe/identify
    if mtype == "data" || mtype == "document" || mtype == "archive" {
        return Ok(FileInfo {
            duration_secs: None,
            width: None,
            height: None,
            codec: None,
            format: Some(ext.to_string()),
            file_size,
            media_type: mtype.to_string(),
        });
    }

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

    // When extracting audio from video, output extension comes from audio_format, not output_format
    let ext = if options.extract_audio == Some(true) {
        options
            .audio_format
            .as_deref()
            .unwrap_or("mp3")
            .to_lowercase()
    } else {
        options.output_format.to_lowercase()
    };

    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(format!("Invalid output format: {ext}"));
    }

    // Route by input media type when extract_audio is set (input is video, output is audio)
    let mtype = if options.extract_audio == Some(true) {
        "audio"
    } else {
        let t = classify_ext(&ext);
        if t == "unknown" {
            return Err(format!("Unsupported output format: {ext}"));
        }
        t
    };

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
            "data" => run_data_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
            ),
            "document" => run_document_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
            ),
            "archive" => run_archive_convert(
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
            Err(msg) if msg == "__DONE__" => {
                // job-done was emitted directly (e.g. archive extract with folder path)
                write_fade_log(&format_log_entry(&job_id, &input_path, "done", ""));
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
        "ffmpeg":   tool_available("ffmpeg"),
        "ffprobe":  tool_available("ffprobe"),
        "magick":   tool_available("magick"),
        "sevenzip": tool_available("7z"),
    })
}

// ── Waveform extraction with frequency colouring ─────────────────────────────

#[derive(Serialize)]
pub struct WaveformData {
    pub amplitudes: Vec<f32>,
    /// HSL hue (0-360) for each bar, derived from per-chunk dominant frequency.
    pub hues: Vec<u32>,
}

/// Zero-crossing rate → HSL hue.
/// At 8 000 Hz sample rate, ZCR × 4 000 ≈ dominant frequency in Hz.
/// Bass (red/orange) → mids (yellow/green) → hi-hats (cyan/blue).
fn zcr_to_hue(zcr: f32) -> u32 {
    // Clamp to [0,1] then map linearly through the hue range 0–240
    // 0.0 (DC / sub-bass) → hue 0   (red)
    // 0.5 (2 kHz mids)    → hue 120 (green)
    // 1.0 (4 kHz hi-hats) → hue 240 (blue)
    let clamped = zcr.clamp(0.0, 1.0);
    (clamped * 240.0) as u32
}

/// Extract a 500-point RMS waveform plus per-bar frequency hue.
/// Uses zero-crossing rate at 8 000 Hz — fast, no extra deps, works well
/// for distinguishing bass kicks from hi-hats visually.
#[command]
fn get_waveform(path: String) -> Result<WaveformData, String> {
    let output = Command::new("ffmpeg")
        .args(["-i", &path, "-ac", "1", "-ar", "8000", "-f", "f32le", "-"])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if output.stdout.is_empty() {
        return Ok(WaveformData { amplitudes: vec![], hues: vec![] });
    }

    let samples: Vec<f32> = output.stdout
        .chunks_exact(4)
        .filter_map(|c| c.try_into().ok().map(f32::from_le_bytes))
        .collect();

    let n = 500usize;
    let chunk_size = (samples.len() / n).max(1);
    let mut amplitudes = Vec::with_capacity(n);
    let mut hues = Vec::with_capacity(n);

    for chunk in samples.chunks(chunk_size).take(n) {
        // RMS amplitude
        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        amplitudes.push(rms);

        // Zero-crossing rate as frequency proxy
        let crossings = chunk
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        let zcr = crossings as f32 / chunk.len() as f32;
        hues.push(zcr_to_hue(zcr));
    }

    // Normalise amplitudes to [0, 1]
    let max = amplitudes.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for a in &mut amplitudes {
            *a /= max;
        }
    }

    Ok(WaveformData { amplitudes, hues })
}

// ── Spectrogram extraction ────────────────────────────────────────────────────

/// Render a rainbow spectrogram PNG via ffmpeg showspectrumpic and return it as base64.
/// Uses image2pipe + png codec to write the PNG directly to stdout — no temp files.
#[command]
fn get_spectrogram(path: String) -> Result<String, String> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            &path,
            "-lavfi",
            "showspectrumpic=s=800x200:legend=0:color=magma:scale=log:fscale=log",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "spectrogram failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&output.stderr))
        ));
    }

    if output.stdout.is_empty() {
        return Err("spectrogram produced no output".to_string());
    }

    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.encode(&output.stdout))
}

// ── Filmstrip extraction ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct FilmstripFrameEvent {
    id:    String,
    index: usize,
    total: usize,
    data:  String, // base64 JPEG
}

/// Extract `count` evenly-spaced thumbnail frames from a video.
/// Returns immediately — each frame is emitted as a "filmstrip-frame" event
/// as it finishes, so the UI fills in incrementally without blocking.
/// Each frame is a separate fast-seek ffmpeg call at nice -n 19 / 1 thread.
#[command]
fn get_filmstrip(window: Window, path: String, id: String, count: usize, duration: f64) -> Result<(), String> {
    if count == 0 || duration <= 0.0 {
        return Ok(());
    }

    std::thread::spawn(move || {
        use base64::Engine as _;

        for i in 0..count {
            // Centre each sample inside its slot
            let ts = format!("{:.3}", (i as f64 + 0.5) * duration / count as f64);

            // One tiny ffmpeg call per frame: -ss before -i = fast keyframe seek,
            // -threads 1 + nice -n 19 = truly background, no beach-ball.
            let output = Command::new("nice")
                .args([
                    "-n", "19",
                    "ffmpeg",
                    "-ss", &ts,
                    "-i", &path,
                    "-frames:v", "1",
                    "-vf", "scale=160:-2:flags=fast_bilinear",
                    "-threads", "1",
                    "-f", "image2pipe",
                    "-vcodec", "mjpeg",
                    "-q:v", "7",
                    "-",
                ])
                .output();

            let data = match output {
                Ok(o) if !o.stdout.is_empty() =>
                    base64::engine::general_purpose::STANDARD.encode(&o.stdout),
                _ => continue,
            };

            let _ = window.emit("filmstrip-frame", FilmstripFrameEvent {
                id: id.clone(),
                index: i,
                total: count,
                data,
            });
        }
    });

    Ok(())
}

// ── Compression diff preview ──────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct DiffPreview {
    path: String,
    note: String,
}

/// Encode a short snippet of the source with the requested codec/resolution and
/// return a clip showing the per-pixel difference between the original and the
/// re-encoded version (amplified for visibility). The snippet is padded with
/// `handle_secs` of runway on each side so the rate-controller and GOP layout
/// reach steady state before the target region is compared.
#[allow(clippy::too_many_arguments)]
#[command]
fn preview_diff(
    path: String,
    codec: String,
    resolution: Option<String>,
    at_secs: f64,
    duration_secs: Option<f64>,
    handle_secs: Option<f64>,
    amplify: Option<f64>,
) -> Result<DiffPreview, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }

    let dur = duration_secs.unwrap_or(1.0).clamp(0.1, 10.0);
    let handle = handle_secs.unwrap_or(3.0).clamp(0.0, 10.0);
    let amp = amplify.unwrap_or(8.0).clamp(1.0, 32.0);

    let start = (at_secs - handle).max(0.0);
    // How far into the encoded snippet the target region begins (handle minus any
    // clipping at the file head).
    let mid_offset = at_secs - start;
    let total = dur + 2.0 * handle;

    let tmp_dir = std::env::temp_dir();
    let job_id = uuid::Uuid::new_v4().to_string();
    let encoded = tmp_dir.join(format!("fade-diff-enc-{job_id}.mp4"));
    let diff = tmp_dir.join(format!("fade-diff-out-{job_id}.mp4"));

    // Optional spatial scaling — must be applied to BOTH inputs during the diff
    // pass so resolutions match for blend.
    let scale_filter = match resolution.as_deref() {
        Some(r) if r != "original" && !r.is_empty() => Some(resolution_to_scale(r)),
        _ => None,
    };

    // ── Pass 1: encode snippet with handles ──
    let mut enc_args: Vec<String> = vec![
        "-y".to_string(),
        "-ss".to_string(),
        format!("{start:.3}"),
        "-i".to_string(),
        path.clone(),
        "-t".to_string(),
        format!("{total:.3}"),
        "-an".to_string(),
    ];
    if let Some(sf) = scale_filter.as_deref() {
        enc_args.push("-vf".to_string());
        enc_args.push(sf.to_string());
    }
    enc_args.extend(ffmpeg_video_codec_args(&codec));
    enc_args.push(encoded.to_string_lossy().to_string());

    let enc_out = Command::new("ffmpeg")
        .args(&enc_args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;
    if !enc_out.status.success() {
        return Err(format!(
            "encode failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&enc_out.stderr))
        ));
    }

    // ── Pass 2: blend=difference, amplified, grayscale output ──
    //
    // Alignment is the critical detail here. `-ss` before `-i` snaps to the
    // nearest keyframe, which for a short encoded snippet (one GOP) is always
    // frame 0 — so the two streams end up several seconds out of phase and the
    // blend produces black.
    //
    // Fix: use `trim=start=` filters (which operate on decoded PTS, not seeks)
    // so both inputs are cut to the exact same temporal window before blending.
    //
    //   Input 0 (source): seek close with -ss for efficiency; trim=start uses
    //     the absolute source PTS to land on exactly at_secs.
    //   Input 1 (encoded snippet): no seek at all; trim=start={mid_offset}
    //     advances through the decoded clip to the target region.
    let pre_seek = (at_secs - 5.0).max(0.0);
    let src_trim = format!("trim=start={at_secs:.3}:duration={dur:.3},setpts=PTS-STARTPTS");
    let enc_trim = format!("trim=start={mid_offset:.3}:duration={dur:.3},setpts=PTS-STARTPTS");

    let src_chain = match scale_filter.as_deref() {
        Some(sf) => format!("[0:v]{src_trim},{sf},format=yuv420p[a]"),
        None     => format!("[0:v]{src_trim},format=yuv420p[a]"),
    };
    let filter = format!(
        "{src_chain};[1:v]{enc_trim},format=yuv420p[b];[a][b]blend=all_mode=difference,lutyuv=y=val*{amp}:u=128:v=128[o]"
    );

    let diff_args: Vec<String> = vec![
        "-y".to_string(),
        "-ss".to_string(), format!("{pre_seek:.3}"),   // efficient seek for long source
        "-i".to_string(), path.clone(),
        "-i".to_string(), encoded.to_string_lossy().to_string(), // no seek — trim handles it
        "-filter_complex".to_string(), filter,
        "-map".to_string(), "[o]".to_string(),
        "-c:v".to_string(), "libx264".to_string(),
        "-preset".to_string(), "ultrafast".to_string(),
        "-crf".to_string(), "16".to_string(),
        "-pix_fmt".to_string(), "yuv420p".to_string(),
        "-an".to_string(),
        diff.to_string_lossy().to_string(),
    ];

    let diff_out = Command::new("ffmpeg")
        .args(&diff_args)
        .output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;

    // Intermediate encoded file no longer needed.
    let _ = std::fs::remove_file(&encoded);

    if !diff_out.status.success() {
        return Err(format!(
            "diff failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&diff_out.stderr))
        ));
    }

    Ok(DiffPreview {
        path: diff.to_string_lossy().to_string(),
        note: format!("codec={codec} handles={handle:.1}s amp={amp:.0}×"),
    })
}

// ── Image quality diff preview ────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct ImageQualityPreview {
    diff_path: String,
    compressed_path: String,
}

/// Encode the source image at `quality` in `output_format`, then compute a
/// per-pixel difference against the original and return both as temp file paths.
/// Only meaningful for lossy formats (JPEG, WebP, AVIF).
#[command]
fn preview_image_quality(
    path: String,
    quality: u32,
    output_format: String,
) -> Result<ImageQualityPreview, String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err(format!("File not found: {path}"));
    }
    match output_format.as_str() {
        "jpeg" | "jpg" | "webp" | "avif" => {}
        other => return Err(format!("{other} is lossless — no compression artifacts to preview")),
    }

    let tmp_dir = std::env::temp_dir();
    let job_id = uuid::Uuid::new_v4().to_string();
    let ext = if output_format == "jpeg" { "jpg" } else { output_format.as_str() };
    let compressed = tmp_dir.join(format!("fade-imgq-enc-{job_id}.{ext}"));
    let diff = tmp_dir.join(format!("fade-imgq-diff-{job_id}.png"));

    // Pass 1: encode at requested quality (induces lossy compression artifacts)
    let enc_out = Command::new("magick")
        .args([
            path.as_str(),
            "-quality",
            &quality.to_string(),
            compressed.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("magick not found: {e}"))?;
    if !enc_out.status.success() {
        return Err(format!(
            "encode failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&enc_out.stderr))
        ));
    }

    // Pass 2: amplified grayscale difference (original − encoded)
    let diff_out = Command::new("magick")
        .args([
            path.as_str(),
            compressed.to_str().unwrap_or(""),
            "-compose",
            "Difference",
            "-composite",
            "-evaluate",
            "multiply",
            "8",
            "-colorspace",
            "gray",
            diff.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("magick not found: {e}"))?;
    if !diff_out.status.success() {
        let _ = std::fs::remove_file(&compressed);
        return Err(format!(
            "diff failed: {}",
            truncate_stderr(&String::from_utf8_lossy(&diff_out.stderr))
        ));
    }

    Ok(ImageQualityPreview {
        diff_path: diff.to_string_lossy().to_string(),
        compressed_path: compressed.to_string_lossy().to_string(),
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

// ── Data conversion (pure Rust) ───────────────────────────────────────────────

fn run_data_convert(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress { job_id: job_id.to_string(), percent: 0.0, message: "Converting data…".to_string() },
    );

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();
    let pretty = opts.pretty_print.unwrap_or(true);
    let delimiter = opts.csv_delimiter.as_deref().unwrap_or(",");
    let delim_byte = delimiter.as_bytes().first().copied().unwrap_or(b',');

    // Normalise to serde_json::Value as intermediate
    let value: serde_json::Value = match in_ext.as_str() {
        "json" | "ndjson" | "jsonl" => {
            serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {e}"))?
        },
        "yaml" | "yml" => {
            serde_yaml::from_str(&raw).map_err(|e| format!("YAML parse error: {e}"))?
        },
        "toml" => {
            let v: toml::Value = toml::from_str(&raw).map_err(|e| format!("TOML parse error: {e}"))?;
            serde_json::to_value(v).map_err(|e| e.to_string())?
        },
        "csv" | "tsv" => {
            let sep = if in_ext == "tsv" { b'\t' } else { b',' };
            let mut rdr = csv::ReaderBuilder::new().delimiter(sep).from_reader(raw.as_bytes());
            let headers: Vec<String> = rdr.headers()
                .map_err(|e| format!("CSV header error: {e}"))?
                .iter().map(|s| s.to_string()).collect();
            let mut rows = Vec::new();
            for result in rdr.records() {
                let record = result.map_err(|e| format!("CSV row error: {e}"))?;
                let obj: serde_json::Map<String, serde_json::Value> = headers.iter()
                    .zip(record.iter())
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.to_string())))
                    .collect();
                rows.push(serde_json::Value::Object(obj));
            }
            serde_json::Value::Array(rows)
        },
        "xml" => {
            // Parse XML into JSON-like structure via quick-xml
            let mut reader = quick_xml::Reader::from_str(&raw);
            reader.config_mut().trim_text(true);
            let mut stack: Vec<(String, serde_json::Map<String, serde_json::Value>)> = Vec::new();
            let mut root_value: Option<serde_json::Value> = None;
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(quick_xml::events::Event::Start(e)) => {
                        let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                        stack.push((name, serde_json::Map::new()));
                    },
                    Ok(quick_xml::events::Event::End(_)) => {
                        if let Some((name, obj)) = stack.pop() {
                            let val = serde_json::Value::Object(obj);
                            if let Some((_, parent)) = stack.last_mut() {
                                parent.insert(name, val);
                            } else {
                                root_value = Some(val);
                            }
                        }
                    },
                    Ok(quick_xml::events::Event::Text(e)) => {
                        let text = e.unescape().map_err(|e| e.to_string())?.to_string();
                        if !text.trim().is_empty() {
                            if let Some((_, obj)) = stack.last_mut() {
                                obj.insert("#text".to_string(), serde_json::Value::String(text));
                            }
                        }
                    },
                    Ok(quick_xml::events::Event::Eof) => break,
                    Err(e) => return Err(format!("XML parse error: {e}")),
                    _ => {},
                }
                buf.clear();
            }
            root_value.unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
        },
        _ => return Err(format!("Unsupported input format: {in_ext}")),
    };

    // Write output
    let output = match out_fmt.as_str() {
        "json" => {
            if pretty {
                serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
            } else {
                serde_json::to_string(&value).map_err(|e| e.to_string())?
            }
        },
        "yaml" => serde_yaml::to_string(&value).map_err(|e| e.to_string())?,
        "toml" => {
            // TOML requires a table at root; wrap arrays
            let toml_val: toml::Value = if value.is_array() {
                let mut map = toml::map::Map::new();
                let items: toml::Value = serde_json::from_value::<toml::Value>(
                    serde_json::to_value(&value).map_err(|e| e.to_string())?
                ).map_err(|e| e.to_string())?;
                map.insert("items".to_string(), items);
                toml::Value::Table(map)
            } else {
                serde_json::from_value::<toml::Value>(
                    serde_json::to_value(&value).map_err(|e| e.to_string())?
                ).map_err(|e| e.to_string())?
            };
            toml::to_string_pretty(&toml_val).map_err(|e| e.to_string())?
        },
        "csv" | "tsv" => {
            let rows = match &value {
                serde_json::Value::Array(arr) => arr.clone(),
                other => vec![other.clone()],
            };
            let mut wtr = csv::WriterBuilder::new().delimiter(delim_byte).from_writer(Vec::new());
            if let Some(first) = rows.first() {
                if let serde_json::Value::Object(obj) = first {
                    let headers: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                    wtr.write_record(&headers).map_err(|e| e.to_string())?;
                    for row in &rows {
                        if let serde_json::Value::Object(obj) = row {
                            let record: Vec<String> = headers.iter()
                                .map(|h| obj.get(*h).map(|v| match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                }).unwrap_or_default())
                                .collect();
                            wtr.write_record(&record).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            String::from_utf8(wtr.into_inner().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?
        },
        "xml" => {
            fn value_to_xml(key: &str, val: &serde_json::Value, out: &mut String, indent: &str, pretty: bool) {
                let nl = if pretty { "\n" } else { "" };
                let next_indent = if pretty { format!("{}  ", indent) } else { String::new() };
                match val {
                    serde_json::Value::Object(obj) => {
                        out.push_str(&format!("{}<{}>{}",  indent, key, nl));
                        for (k, v) in obj {
                            if k == "#text" {
                                if let serde_json::Value::String(s) = v {
                                    out.push_str(&format!("{}{}{}", next_indent, s, nl));
                                }
                            } else {
                                value_to_xml(k, v, out, &next_indent, pretty);
                            }
                        }
                        out.push_str(&format!("{}</{}>{}",  indent, key, nl));
                    },
                    serde_json::Value::Array(arr) => {
                        for item in arr { value_to_xml(key, item, out, indent, pretty); }
                    },
                    serde_json::Value::String(s) => {
                        out.push_str(&format!("{}<{}>{}</{}>{}",  indent, key, s, key, nl));
                    },
                    other => {
                        out.push_str(&format!("{}<{}>{}</{}>{}",  indent, key, other, key, nl));
                    },
                }
            }
            let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            value_to_xml("root", &value, &mut xml, "", pretty);
            xml
        },
        _ => return Err(format!("Unsupported output format: {out_fmt}")),
    };

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Document conversion (pure Rust) ──────────────────────────────────────────

fn run_document_convert(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
) -> Result<(), String> {
    let _ = window.emit(
        "job-progress",
        JobProgress { job_id: job_id.to_string(), percent: 0.0, message: "Converting document…".to_string() },
    );

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();

    let output = match (in_ext.as_str(), out_fmt.as_str()) {
        ("md" | "markdown", "html") => {
            let parser = pulldown_cmark::Parser::new_ext(&raw, pulldown_cmark::Options::all());
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, parser);
            html
        },
        ("md" | "markdown", "txt") => strip_md(&raw),
        ("md" | "markdown", "md") => raw,
        ("html" | "htm", "txt") => html_to_text(&raw),
        ("html" | "htm", "md") => html_to_md(&raw),
        ("html" | "htm", "html") => raw,
        ("txt", "html") => {
            let escaped = raw.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            let paragraphs: String = escaped
                .split("\n\n")
                .map(|p| format!("<p>{}</p>", p.trim().replace('\n', "<br>")))
                .collect::<Vec<_>>()
                .join("\n");
            format!("<!DOCTYPE html>\n<html><body>\n{}\n</body></html>", paragraphs)
        },
        ("txt", "md") => {
            // Wrap double-newline blocks as paragraphs (no extra syntax)
            raw.clone()
        },
        ("txt", "txt") => raw,
        _ => return Err(format!("Unsupported conversion: {in_ext} → {out_fmt}")),
    };

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;
    Ok(())
}

fn strip_md(raw: &str) -> String {
    let mut txt = raw.to_string();
    // Code fences
    let mut result = String::new();
    let mut in_fence = false;
    for line in txt.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            result.push_str(line);
            result.push('\n');
        }
    }
    txt = result;
    // Headers
    txt = txt.lines().map(|l| {
        let trimmed = l.trim_start_matches('#').trim_start();
        if l.starts_with('#') { trimmed.to_string() } else { l.to_string() }
    }).collect::<Vec<_>>().join("\n");
    // Bold/italic (simple passes)
    for marker in &["**", "__"] {
        while let (Some(s), Some(e)) = (txt.find(marker), txt[txt.find(marker).unwrap_or(0)+marker.len()..].find(marker)) {
            let start = s;
            let end = s + marker.len() + e + marker.len();
            if end <= txt.len() {
                let inner = txt[s+marker.len()..s+marker.len()+e].to_string();
                txt = format!("{}{}{}", &txt[..start], inner, &txt[end..]);
            } else { break; }
        }
    }
    for marker in &["*", "_"] {
        while let (Some(s), Some(e)) = (txt.find(marker), txt.get(txt.find(marker).unwrap_or(0)+marker.len()..).and_then(|t| t.find(marker))) {
            let start = s;
            let end = s + marker.len() + e + marker.len();
            if end <= txt.len() {
                let inner = txt[s+marker.len()..s+marker.len()+e].to_string();
                txt = format!("{}{}{}", &txt[..start], inner, &txt[end..]);
            } else { break; }
        }
    }
    // Inline code
    while let Some(s) = txt.find('`') {
        if let Some(e) = txt[s+1..].find('`') {
            let inner = txt[s+1..s+1+e].to_string();
            txt = format!("{}{}{}", &txt[..s], inner, &txt[s+1+e+1..]);
        } else { break; }
    }
    // Links [text](url)
    while let Some(s) = txt.find('[') {
        if let Some(m) = txt[s+1..].find("](") {
            let text_end = s + 1 + m;
            let text = txt[s+1..text_end].to_string();
            if let Some(url_end) = txt[text_end+2..].find(')') {
                let full_end = text_end + 2 + url_end + 1;
                txt = format!("{}{}{}", &txt[..s], text, &txt[full_end..]);
            } else { break; }
        } else { break; }
    }
    // List markers
    txt = txt.lines().map(|l| {
        let t = l.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            t[2..].to_string()
        } else { l.to_string() }
    }).collect::<Vec<_>>().join("\n");
    txt.trim().to_string()
}

fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => { in_tag = false; out.push(' '); },
            _ if !in_tag => out.push(ch),
            _ => {},
        }
    }
    // Decode basic HTML entities
    out = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
             .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ");
    // Collapse whitespace
    let lines: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    lines.join("\n")
}

fn html_to_md(html: &str) -> String {
    let mut out = html.to_string();
    // Headings
    for n in (1u8..=6).rev() {
        let tag = format!("<h{n}");
        let close = format!("</h{n}>");
        let prefix = "#".repeat(n as usize) + " ";
        while let Some(s) = out.to_lowercase().find(&tag) {
            let tag_end = out[s..].find('>').map(|i| s + i + 1).unwrap_or(s + tag.len() + 1);
            let close_pos = out[tag_end..].to_lowercase().find(&close).map(|i| tag_end + i);
            if let Some(e) = close_pos {
                let inner = out[tag_end..e].to_string();
                out = format!("{}{}{}\n{}", &out[..s], prefix, inner, &out[e+close.len()..]);
            } else { break; }
        }
    }
    // Bold/italic
    for (open, close, md) in &[("<strong>","</strong>","**"),("<b>","</b>","**"),("<em>","</em>","*"),("<i>","</i>","*")] {
        while let Some(s) = out.to_lowercase().find(open) {
            let inner_start = s + open.len();
            if let Some(e) = out[inner_start..].to_lowercase().find(close) {
                let inner = out[inner_start..inner_start+e].to_string();
                out = format!("{}{}{}{}{}", &out[..s], md, inner, md, &out[inner_start+e+close.len()..]);
            } else { break; }
        }
    }
    // Links <a href="url">text</a>
    while let Some(s) = out.to_lowercase().find("<a ") {
        let tag_end = match out[s..].find('>') {
            Some(i) => s + i + 1,
            None => break,
        };
        let href = {
            let tag_str = &out[s..tag_end];
            if let Some(h) = tag_str.to_lowercase().find("href=\"") {
                let start = s + h + 6;
                let end_q = out[start..].find('"').map(|i| start + i).unwrap_or(start);
                out[start..end_q].to_string()
            } else { String::new() }
        };
        if let Some(e) = out[tag_end..].to_lowercase().find("</a>") {
            let text = out[tag_end..tag_end+e].to_string();
            out = format!("{}[{}]({}){}", &out[..s], text, href, &out[tag_end+e+4..]);
        } else { break; }
    }
    // Code
    while let Some(s) = out.to_lowercase().find("<code>") {
        if let Some(e) = out[s+6..].to_lowercase().find("</code>") {
            let inner = out[s+6..s+6+e].to_string();
            out = format!("{}`{}`{}", &out[..s], inner, &out[s+6+e+7..]);
        } else { break; }
    }
    // Paragraphs
    out = out.replace("<p>", "").replace("</p>", "\n\n");
    out = out.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    // List items
    out = out.replace("<li>", "- ").replace("</li>", "\n");
    out = out.replace("<ul>", "").replace("</ul>", "\n");
    out = out.replace("<ol>", "").replace("</ol>", "\n");
    // Strip remaining tags
    html_to_text(&out)
}

// ── Archive conversion (7z) ───────────────────────────────────────────────────

fn run_archive_convert(
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
            message: if operation == "extract" { "Extracting…".to_string() } else { "Repacking…".to_string() },
        },
    );

    if operation == "extract" {
        // Extract to {stem}_extracted/ beside input
        let p = Path::new(input_path);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let parent = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_else(|| ".".to_string());
        let out_dir = opts.output_dir.as_deref().unwrap_or(&parent);
        let extract_folder = format!("{}/{}_extracted", out_dir, stem);

        let mut child = Command::new("7z")
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
                for line in reader.lines().map_while(Result::ok) { lines.push(line); }
            }
            lines.join("\n")
        });

        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit("job-progress", JobProgress {
                        job_id: job_id.to_string(),
                        percent: pct,
                        message: format!("{}%", pct as u32),
                    });
                }
            }
        }

        let error_output = stderr_thread.join().unwrap_or_default();
        let child_opt = { let mut map = processes.lock().unwrap(); map.remove(job_id) };
        let success = match child_opt {
            Some(mut c) => c.wait().map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        if cancelled.load(Ordering::SeqCst) { return Err("CANCELLED".to_string()); }
        if success {
            // Emit job-done with the folder path as output_path
            // We return Ok here; the caller emits job-done with the original output_path.
            // Override by writing the folder path into a sentinel we can't easily thread through.
            // Instead: write a small redirect file and let the window emit handle it.
            // Actually the simplest approach: store the extract folder in a thread-local.
            // But since the thread joins and returns Result<()>, the caller always uses output_path.
            // Workaround: return Err with a special prefix that carry the real path.
            // Better: just return Ok and accept the default output_path is wrong for extract.
            // Per spec: "output_path for extract = the folder path". We need to thread this through.
            // Since we can't change the return type, emit job-done directly here and return a
            // special sentinel error that the caller's match will not re-emit as an error.
            let _ = window.emit("job-done", JobDone {
                job_id: job_id.to_string(),
                output_path: extract_folder,
            });
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
    let extract_result = {
        let mut child = Command::new("7z")
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
                for line in reader.lines().map_while(Result::ok) { lines.push(line); }
            }
            lines.join("\n")
        });

        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit("job-progress", JobProgress {
                        job_id: job_id.to_string(),
                        percent: pct / 2.0,
                        message: format!("Extracting {}%", pct as u32),
                    });
                }
            }
        }

        let error_output = stderr_thread.join().unwrap_or_default();
        let child_opt = { let mut map = processes.lock().unwrap(); map.remove(job_id) };
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
            } else { truncate_stderr(&error_output) });
        }
    };
    let _ = extract_result;

    // Step 2: repack
    let repack_result = {
        let mut child = Command::new("7z")
            .args(["a", output_path, &format!("{}/*", tmp_dir)])
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
                for line in reader.lines().map_while(Result::ok) { lines.push(line); }
            }
            lines.join("\n")
        });

        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(pct) = parse_7z_percent(&line) {
                    let _ = window.emit("job-progress", JobProgress {
                        job_id: job_id.to_string(),
                        percent: 50.0 + pct / 2.0,
                        message: format!("Packing {}%", pct as u32),
                    });
                }
            }
        }

        let error_output = stderr_thread.join().unwrap_or_default();
        let child_opt = { let mut map = processes.lock().unwrap(); map.remove(job_id) };
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
            } else { truncate_stderr(&error_output) });
        }
    };
    let _ = repack_result;

    Ok(())
}

/// Parse 7z progress lines like "  7% - filename.ext"
fn parse_7z_percent(line: &str) -> Option<f32> {
    let trimmed = line.trim();
    let pct_end = trimmed.find('%')?;
    trimmed[..pct_end].trim().parse::<f32>().ok()
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

/// List all files (non-recursive) in a directory. Returns full paths, sorted.
#[command]
fn scan_dir(path: String) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(&path)
        .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            if p.is_file() && !name_str.starts_with('.') {
                p.to_str().map(str::to_owned)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
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
            get_waveform,
            get_spectrogram,
            get_filmstrip,
            preview_diff,
            preview_image_quality,
            get_theme,
            get_accent,
            list_presets,
            save_preset,
            delete_preset,
            scan_dir,
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
        assert!(args.iter().any(|a| a.starts_with("loudnorm=")));
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
