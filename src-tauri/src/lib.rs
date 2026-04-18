#[allow(unused_imports)]
use librewin_common::media::media_type_for; // used in tests via super::*
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{command, Emitter, State, Window};

pub mod args;
pub mod convert;
pub mod fs_commands;
pub mod presets;
pub mod preview;
pub mod probe;
pub mod theme;
pub use args::{
    build_ffmpeg_audio_args, build_ffmpeg_video_args, build_image_magick_args,
    ffmpeg_video_codec_args, resolution_to_scale,
};
pub use fs_commands::{file_exists, scan_dir};
pub use presets::{delete_preset, list_presets, save_preset};
pub use preview::{preview_diff, preview_image_quality};
pub use probe::{get_file_info, get_filmstrip, get_spectrogram, get_waveform};
pub use theme::{get_accent, get_theme};
use convert::{
    run_archive_convert, run_audio_convert, run_data_convert, run_document_convert,
    run_image_convert, run_video_convert,
};

// ── AppState ───────────────────────────────────────────────────────────────────

pub struct AppState {
    pub processes: Arc<Mutex<HashMap<String, Child>>>,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub(crate) struct JobProgress {
    pub(crate) job_id: String,
    pub(crate) percent: f32,
    pub(crate) message: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct JobDone {
    pub(crate) job_id: String,
    pub(crate) output_path: String,
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
    pub archive_compression: Option<u32>, // 0-9, zip/gz/7z level
    // Output naming
    pub output_suffix: Option<String>,
    // Metadata — when false, strip EXIF/tags/etc. None or true = preserve.
    // Applies to image (ImageMagick -strip) and video/audio (ffmpeg -map_metadata -1).
    pub preserve_metadata: Option<bool>,

    // ── Format-specific audio controls (see docs/FORMAT-CONTROLS.md §2) ──
    pub channels: Option<String>,           // "source" | "mono" | "stereo" | "joint" | "5.1"
    pub bit_depth: Option<u32>,             // 16 | 24 | 32
    pub mp3_bitrate_mode: Option<String>,   // "cbr" | "vbr"
    pub mp3_vbr_quality: Option<u32>,       // 0-9
    pub flac_compression: Option<u32>,      // 0-8
    pub ogg_bitrate_mode: Option<String>,   // "vbr" | "cbr" | "abr"
    pub ogg_vbr_quality: Option<i32>,       // -1..=10
    pub aac_profile: Option<String>,        // "lc" | "he" | "hev2"
    pub opus_application: Option<String>,   // "audio" | "voip" | "lowdelay"
    pub opus_vbr: Option<bool>,
    pub m4a_subcodec: Option<String>,       // "aac" | "alac"
    pub wma_mode: Option<String>,           // "standard" | "pro" | "lossless"
    pub ac3_bitrate: Option<u32>,           // 192 | 384 | 448 | 640 (kbps)
    pub dts_bitrate: Option<u32>,           // 754 | 1510 (kbps)

    // ── Format-specific video controls ──
    pub crf: Option<u32>,                   // 0-51
    pub preset: Option<String>,             // "ultrafast" | "fast" | "medium" | "slow" | "veryslow"
    pub h264_profile: Option<String>,       // "baseline" | "main" | "high"
    pub pix_fmt: Option<String>,            // "yuv420p" | "yuv422p" | "yuv444p"
    pub tune: Option<String>,               // "none" | "film" | "animation" | "grain"
    pub frame_rate: Option<String>,         // "original" | "24" | "25" | "30" | "60"
    pub webm_bitrate_mode: Option<String>,  // "crf" | "cbr" | "cvbr"
    pub vp9_speed: Option<u32>,             // 0-5
    pub av1_speed: Option<u32>,             // 0-10
    pub mkv_subtitle: Option<String>,       // "none" | "copy" | "burn"
    pub avi_video_bitrate: Option<u32>,     // kbps
    pub gif_width: Option<String>,          // "original" | "320" | "480" | "640"
    pub gif_fps: Option<String>,            // "original" | "5" | "10" | "15"
    pub gif_loop: Option<String>,           // "infinite" | "once" | "none"
    pub gif_palette_size: Option<u32>,      // 32 | 64 | 128 | 256
    pub gif_dither: Option<String>,         // "none" | "bayer" | "floyd"

    // ── Format-specific image controls ──
    pub jpeg_chroma: Option<String>,        // "420" | "422" | "444"
    pub jpeg_progressive: Option<bool>,
    pub png_compression: Option<u32>,       // 0-9
    pub png_color_mode: Option<String>,     // "rgb" | "rgba" | "gray" | "graya" | "palette"
    pub png_interlaced: Option<bool>,
    pub tiff_compression: Option<String>,   // "none" | "lzw" | "deflate" | "packbits"
    pub tiff_bit_depth: Option<u32>,        // 8 | 16 | 32
    pub tiff_color_mode: Option<String>,    // "rgb" | "cmyk" | "gray"
    pub webp_lossless: Option<bool>,
    pub webp_method: Option<u32>,           // 0-6
    pub avif_speed: Option<u32>,            // 0-10
    pub avif_chroma: Option<String>,        // "420" | "422" | "444"
    pub bmp_bit_depth: Option<u32>,         // 8 | 16 | 24 | 32
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
            archive_compression: None,
            output_suffix: None,
            preserve_metadata: None,

            channels: None,
            bit_depth: None,
            mp3_bitrate_mode: None,
            mp3_vbr_quality: None,
            flac_compression: None,
            ogg_bitrate_mode: None,
            ogg_vbr_quality: None,
            aac_profile: None,
            opus_application: None,
            opus_vbr: None,
            m4a_subcodec: None,
            wma_mode: None,
            ac3_bitrate: None,
            dts_bitrate: None,

            crf: None,
            preset: None,
            h264_profile: None,
            pix_fmt: None,
            tune: None,
            frame_rate: None,
            webm_bitrate_mode: None,
            vp9_speed: None,
            av1_speed: None,
            mkv_subtitle: None,
            avi_video_bitrate: None,
            gif_width: None,
            gif_fps: None,
            gif_loop: None,
            gif_palette_size: None,
            gif_dither: None,

            jpeg_chroma: None,
            jpeg_progressive: None,
            png_compression: None,
            png_color_mode: None,
            png_interlaced: None,
            tiff_compression: None,
            tiff_bit_depth: None,
            tiff_color_mode: None,
            webp_lossless: None,
            webp_method: None,
            avif_speed: None,
            avif_chroma: None,
            bmp_bit_depth: None,
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
pub(crate) fn probe_duration(path: &str) -> Option<f64> {
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
pub(crate) fn parse_out_time_ms(line: &str) -> Option<f64> {
    let val = line.strip_prefix("out_time_ms=")?;
    val.trim().parse::<f64>().ok().map(|ms| ms / 1_000_000.0)
}

/// Classify a file extension into a media type, covering all types Fade supports.
pub(crate) fn classify_ext(ext: &str) -> &'static str {
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
pub(crate) fn truncate_stderr(s: &str) -> String {
    if s.len() > 2000 {
        s[s.len() - 2000..].to_string()
    } else {
        s.to_string()
    }
}


// ── Commands ──────────────────────────────────────────────────────────────────


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
        "sevenzip": tool_available("7z") || tool_available("7zz"),
    })
}

/// Warp the OS cursor to screen coordinates (physical pixels).
/// Used after zoom button clicks so the cursor stays on target
/// after the button's pixel position shifts due to UI rescale.
#[command]
fn set_cursor_position(window: Window, x: i32, y: i32) -> Result<(), String> {
    window
        .set_cursor_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())
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
            file_exists,
            set_cursor_position,
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

    // ── format-specific image args ────────────────────────────────────────────

    fn find_pair<'a>(args: &'a [String], flag: &str, value: &str) -> bool {
        args.windows(2).any(|w| w[0] == flag && w[1] == value)
    }

    #[test]
    fn image_args_jpeg_chroma() {
        let opts = ConvertOptions {
            output_format: "jpg".into(),
            jpeg_chroma: Some("444".into()),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpg", &opts);
        assert!(find_pair(&args, "-sampling-factor", "4:4:4"));
        assert_eq!(args.last().unwrap(), "out.jpg");
    }

    #[test]
    fn image_args_jpeg_progressive_and_jpeg_alias() {
        let opts = ConvertOptions {
            output_format: "jpeg".into(),
            jpeg_progressive: Some(true),
            jpeg_chroma: Some("420".into()),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.jpeg", &opts);
        assert!(find_pair(&args, "-sampling-factor", "4:2:0"));
        assert!(find_pair(&args, "-interlace", "Plane"));
    }

    #[test]
    fn image_args_png_all() {
        let opts = ConvertOptions {
            output_format: "png".into(),
            png_compression: Some(9),
            png_color_mode: Some("rgba".into()),
            png_interlaced: Some(true),
            ..Default::default()
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        assert!(find_pair(&args, "-define", "png:compression-level=9"));
        assert!(find_pair(&args, "-define", "png:color-type=6"));
        assert!(find_pair(&args, "-interlace", "Plane"));
    }

    #[test]
    fn image_args_webp_lossless_method() {
        let opts = ConvertOptions {
            output_format: "webp".into(),
            webp_lossless: Some(true),
            webp_method: Some(6),
            ..Default::default()
        };
        let args = build_image_magick_args("in.png", "out.webp", &opts);
        assert!(find_pair(&args, "-define", "webp:lossless=true"));
        assert!(find_pair(&args, "-define", "webp:method=6"));
    }

    #[test]
    fn image_args_tiff_all() {
        let opts = ConvertOptions {
            output_format: "tiff".into(),
            tiff_compression: Some("deflate".into()),
            tiff_bit_depth: Some(32),
            tiff_color_mode: Some("cmyk".into()),
            ..Default::default()
        };
        let args = build_image_magick_args("in.tif", "out.tiff", &opts);
        assert!(find_pair(&args, "-compress", "Zip"));
        assert!(find_pair(&args, "-depth", "32"));
        assert!(find_pair(&args, "-define", "quantum:format=floating-point"));
        assert!(find_pair(&args, "-colorspace", "CMYK"));
    }

    #[test]
    fn image_args_tif_alias() {
        let opts = ConvertOptions {
            output_format: "tif".into(),
            tiff_compression: Some("lzw".into()),
            ..Default::default()
        };
        let args = build_image_magick_args("in.tif", "out.tif", &opts);
        assert!(find_pair(&args, "-compress", "LZW"));
    }

    #[test]
    fn image_args_bmp_bit_depth() {
        let opts = ConvertOptions {
            output_format: "bmp".into(),
            bmp_bit_depth: Some(24),
            ..Default::default()
        };
        let args = build_image_magick_args("in.png", "out.bmp", &opts);
        assert!(find_pair(&args, "-depth", "24"));
    }

    #[test]
    fn image_args_avif_speed_and_chroma() {
        let opts = ConvertOptions {
            output_format: "avif".into(),
            avif_speed: Some(6),
            avif_chroma: Some("422".into()),
            ..Default::default()
        };
        let args = build_image_magick_args("in.png", "out.avif", &opts);
        assert!(find_pair(&args, "-define", "heic:speed=6"));
        assert!(find_pair(&args, "-sampling-factor", "4:2:2"));
    }

    #[test]
    fn image_args_format_specific_none_when_unset() {
        let opts = ConvertOptions {
            output_format: "png".into(),
            ..Default::default()
        };
        let args = build_image_magick_args("in.png", "out.png", &opts);
        // No format-specific flags emitted
        assert!(!args.iter().any(|a| a.starts_with("png:")));
        assert!(!args.iter().any(|a| a == "-interlace"));
        assert_eq!(args.last().unwrap(), "out.png");
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

    // ── video format-specific args ────────────────────────────────────────────

    fn vf_contains(args: &[String], needle: &str) -> bool {
        if let Some(i) = args.iter().position(|a| a == "-vf") {
            args.get(i + 1).map(|s| s.contains(needle)).unwrap_or(false)
        } else {
            false
        }
    }

    #[test]
    fn video_args_h264_crf_preset() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            crf: Some(18),
            preset: Some("slow".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-crf", "18"));
        assert!(find_pair(&args, "-preset", "slow"));
    }

    #[test]
    fn video_args_h265_profile_main() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h265".into()),
            h264_profile: Some("main".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-profile:v", "main"));
        assert!(args.contains(&"libx265".to_string()));
    }

    #[test]
    fn video_args_h264_tune_none_omitted() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            tune: Some("none".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(!args.contains(&"-tune".to_string()));
    }

    #[test]
    fn video_args_h264_tune_film() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            tune: Some("film".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-tune", "film"));
    }

    #[test]
    fn video_args_h264_pix_fmt() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            pix_fmt: Some("yuv444p".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-pix_fmt", "yuv444p"));
    }

    #[test]
    fn video_args_vp9_speed() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("vp9".into()),
            vp9_speed: Some(2),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-deadline", "good"));
        assert!(find_pair(&args, "-cpu-used", "2"));
    }

    #[test]
    fn video_args_av1_speed() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("av1".into()),
            av1_speed: Some(6),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-cpu-used", "6"));
    }

    #[test]
    fn video_args_frame_rate_30() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            frame_rate: Some("30".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(find_pair(&args, "-r", "30"));
    }

    #[test]
    fn video_args_frame_rate_original_omitted() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("h264".into()),
            frame_rate: Some("original".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(!args.contains(&"-r".to_string()));
    }

    #[test]
    fn video_args_copy_suppresses_new_flags() {
        let opts = ConvertOptions {
            output_format: "mp4".into(),
            codec: Some("copy".into()),
            crf: Some(18),
            preset: Some("slow".into()),
            h264_profile: Some("high".into()),
            tune: Some("film".into()),
            pix_fmt: Some("yuv420p".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.mp4", &opts);
        assert!(!args.contains(&"-crf".to_string()));
        assert!(!args.contains(&"-preset".to_string()));
        assert!(!args.contains(&"-profile:v".to_string()));
        assert!(!args.contains(&"-tune".to_string()));
        assert!(!args.contains(&"-pix_fmt".to_string()));
        assert!(find_pair(&args, "-c", "copy"));
    }

    #[test]
    fn video_args_webm_crf_mode() {
        let opts = ConvertOptions {
            output_format: "webm".into(),
            codec: Some("vp9".into()),
            crf: Some(33),
            webm_bitrate_mode: Some("crf".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "0"));
        assert!(find_pair(&args, "-crf", "33"));
    }

    #[test]
    fn video_args_webm_cbr_mode() {
        let opts = ConvertOptions {
            output_format: "webm".into(),
            codec: Some("vp9".into()),
            webm_bitrate_mode: Some("cbr".into()),
            bitrate: Some(2500),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "2500k"));
        assert!(find_pair(&args, "-minrate", "2500k"));
        assert!(find_pair(&args, "-maxrate", "2500k"));
    }

    #[test]
    fn video_args_webm_cvbr_mode() {
        let opts = ConvertOptions {
            output_format: "webm".into(),
            codec: Some("vp9".into()),
            webm_bitrate_mode: Some("cvbr".into()),
            bitrate: Some(2000),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "2000k"));
        assert!(find_pair(&args, "-maxrate", "3000k"));
    }

    #[test]
    fn video_args_mkv_subtitle_copy() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            mkv_subtitle: Some("copy".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        assert!(find_pair(&args, "-c:s", "copy"));
    }

    #[test]
    fn video_args_mkv_subtitle_none() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            mkv_subtitle: Some("none".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        assert!(args.contains(&"-sn".to_string()));
    }

    #[test]
    fn video_args_mkv_subtitle_burn_merges_with_scale() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            resolution: Some("1280x720".into()),
            mkv_subtitle: Some("burn".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        assert!(vf_contains(&args, "1280:720"));
        assert!(vf_contains(&args, "subtitles=in.mkv"));
    }

    #[test]
    fn video_args_mkv_subtitle_burn_without_scale() {
        let opts = ConvertOptions {
            output_format: "mkv".into(),
            codec: Some("h264".into()),
            mkv_subtitle: Some("burn".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mkv", "out.mkv", &opts);
        assert!(vf_contains(&args, "subtitles=in.mkv"));
    }

    #[test]
    fn video_args_avi_video_bitrate() {
        let opts = ConvertOptions {
            output_format: "avi".into(),
            codec: Some("h264".into()),
            avi_video_bitrate: Some(8000),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.avi", &opts);
        assert!(find_pair(&args, "-b:v", "8000k"));
    }

    #[test]
    fn video_args_gif_palette_pipeline() {
        let opts = ConvertOptions {
            output_format: "gif".into(),
            gif_width: Some("480".into()),
            gif_fps: Some("15".into()),
            gif_palette_size: Some(128),
            gif_dither: Some("floyd".into()),
            gif_loop: Some("infinite".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.gif", &opts);
        assert!(vf_contains(&args, "fps=15"));
        assert!(vf_contains(&args, "scale=480:-1:flags=lanczos"));
        assert!(vf_contains(&args, "palettegen=max_colors=128"));
        assert!(vf_contains(&args, "paletteuse=dither=floyd_steinberg"));
        assert!(find_pair(&args, "-loop", "0"));
        // No codec/crf/preset for gif
        assert!(!args.contains(&"-crf".to_string()));
        assert!(!args.contains(&"-preset".to_string()));
        assert!(!args.contains(&"-vcodec".to_string()));
    }

    #[test]
    fn video_args_gif_loop_once() {
        let opts = ConvertOptions {
            output_format: "gif".into(),
            gif_loop: Some("once".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.gif", &opts);
        assert!(find_pair(&args, "-loop", "1"));
    }

    #[test]
    fn video_args_gif_loop_none() {
        let opts = ConvertOptions {
            output_format: "gif".into(),
            gif_loop: Some("none".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.gif", &opts);
        assert!(find_pair(&args, "-loop", "-1"));
    }

    #[test]
    fn video_args_gif_original_width_omits_scale() {
        let opts = ConvertOptions {
            output_format: "gif".into(),
            gif_width: Some("original".into()),
            gif_fps: Some("original".into()),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.gif", &opts);
        assert!(!vf_contains(&args, "scale="));
        assert!(!vf_contains(&args, "fps="));
        // But palettegen/paletteuse still present
        assert!(vf_contains(&args, "palettegen"));
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

    // ── Format-specific audio controls ────────────────────────────────────────

    #[test]
    fn audio_args_mp3_vbr_omits_ba_and_emits_qa() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            bitrate: Some(192),
            mp3_bitrate_mode: Some("vbr".to_string()),
            mp3_vbr_quality: Some(3),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        assert!(!args.contains(&"-b:a".to_string()), "VBR must not emit -b:a");
        let q_idx = args.iter().position(|a| a == "-q:a").expect("-q:a missing");
        assert_eq!(args[q_idx + 1], "3");
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libmp3lame"));
    }

    #[test]
    fn audio_args_mp3_cbr_keeps_ba() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            bitrate: Some(256),
            mp3_bitrate_mode: Some("cbr".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        let br_idx = args.iter().position(|a| a == "-b:a").expect("-b:a missing");
        assert_eq!(args[br_idx + 1], "256k");
        assert!(!args.contains(&"-q:a".to_string()));
    }

    #[test]
    fn audio_args_mp3_joint_stereo() {
        let opts = ConvertOptions {
            output_format: "mp3".to_string(),
            channels: Some("joint".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.mp3", &opts);
        let js_idx = args
            .iter()
            .position(|a| a == "-joint_stereo")
            .expect("-joint_stereo missing");
        assert_eq!(args[js_idx + 1], "1");
        let ac_idx = args.iter().position(|a| a == "-ac").unwrap();
        assert_eq!(args[ac_idx + 1], "2");
    }

    #[test]
    fn audio_args_flac_compression_no_bitrate() {
        let opts = ConvertOptions {
            output_format: "flac".to_string(),
            bitrate: Some(320),
            flac_compression: Some(8),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.flac", &opts);
        assert!(!args.contains(&"-b:a".to_string()));
        let cl_idx = args
            .iter()
            .position(|a| a == "-compression_level")
            .expect("-compression_level missing");
        assert_eq!(args[cl_idx + 1], "8");
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
    }

    #[test]
    fn audio_args_opus_application_and_vbr() {
        let opts = ConvertOptions {
            output_format: "opus".to_string(),
            opus_application: Some("voip".to_string()),
            opus_vbr: Some(true),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.opus", &opts);
        let ap_idx = args
            .iter()
            .position(|a| a == "-application")
            .expect("-application missing");
        assert_eq!(args[ap_idx + 1], "voip");
        let vbr_idx = args.iter().position(|a| a == "-vbr").expect("-vbr missing");
        assert_eq!(args[vbr_idx + 1], "on");
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
    }

    #[test]
    fn audio_args_aac_profile_hev2() {
        let opts = ConvertOptions {
            output_format: "aac".to_string(),
            aac_profile: Some("hev2".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.aac", &opts);
        let p_idx = args
            .iter()
            .position(|a| a == "-profile:a")
            .expect("-profile:a missing");
        assert_eq!(args[p_idx + 1], "aac_he_v2");
    }

    #[test]
    fn audio_args_wav_bit_depth_sample_fmt() {
        let opts = ConvertOptions {
            output_format: "wav".to_string(),
            bit_depth: Some(24),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.wav", &opts);
        let sf_idx = args
            .iter()
            .position(|a| a == "-sample_fmt")
            .expect("-sample_fmt missing");
        assert_eq!(args[sf_idx + 1], "s24");
        assert!(!args.contains(&"-b:a".to_string()));
    }

    #[test]
    fn audio_args_channels_mono_stereo_surround() {
        let mk = |ch: &str| ConvertOptions {
            output_format: "wav".to_string(),
            channels: Some(ch.to_string()),
            ..Default::default()
        };
        let mono = build_ffmpeg_audio_args("i", "o", &mk("mono"));
        let stereo = build_ffmpeg_audio_args("i", "o", &mk("stereo"));
        let surround = build_ffmpeg_audio_args("i", "o", &mk("5.1"));
        let source = build_ffmpeg_audio_args("i", "o", &mk("source"));

        let ac = |a: &Vec<String>| a.iter().position(|s| s == "-ac").map(|i| a[i + 1].clone());
        assert_eq!(ac(&mono).as_deref(), Some("1"));
        assert_eq!(ac(&stereo).as_deref(), Some("2"));
        assert_eq!(ac(&surround).as_deref(), Some("6"));
        assert_eq!(ac(&source), None);
    }

    #[test]
    fn audio_args_ogg_cbr_minmax() {
        let opts = ConvertOptions {
            output_format: "ogg".to_string(),
            bitrate: Some(192),
            ogg_bitrate_mode: Some("cbr".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.ogg", &opts);
        let br_idx = args.iter().position(|a| a == "-b:a").expect("-b:a missing");
        assert_eq!(args[br_idx + 1], "192k");
        let min_idx = args
            .iter()
            .position(|a| a == "-minrate")
            .expect("minrate missing");
        assert_eq!(args[min_idx + 1], "192k");
        let max_idx = args
            .iter()
            .position(|a| a == "-maxrate")
            .expect("maxrate missing");
        assert_eq!(args[max_idx + 1], "192k");
    }

    #[test]
    fn audio_args_m4a_alac_suppresses_bitrate() {
        let opts = ConvertOptions {
            output_format: "m4a".to_string(),
            bitrate: Some(256),
            m4a_subcodec: Some("alac".to_string()),
            bit_depth: Some(24),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.m4a", &opts);
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "alac"));
        assert!(!args.contains(&"-b:a".to_string()));
        let sf_idx = args
            .iter()
            .position(|a| a == "-sample_fmt")
            .expect("-sample_fmt missing");
        assert_eq!(args[sf_idx + 1], "s24p");
    }

    #[test]
    fn audio_args_ac3_dts_bitrate() {
        let ac3 = build_ffmpeg_audio_args(
            "i",
            "o.ac3",
            &ConvertOptions {
                output_format: "ac3".to_string(),
                ac3_bitrate: Some(448),
                ..Default::default()
            },
        );
        assert!(ac3.windows(2).any(|w| w[0] == "-c:a" && w[1] == "ac3"));
        let idx = ac3.iter().position(|a| a == "-b:a").unwrap();
        assert_eq!(ac3[idx + 1], "448k");

        let dts = build_ffmpeg_audio_args(
            "i",
            "o.dts",
            &ConvertOptions {
                output_format: "dts".to_string(),
                dts_bitrate: Some(1510),
                ..Default::default()
            },
        );
        assert!(dts.windows(2).any(|w| w[0] == "-c:a" && w[1] == "dca"));
        assert!(dts.contains(&"-strict".to_string()));
        let idx = dts.iter().position(|a| a == "-b:a").unwrap();
        assert_eq!(dts[idx + 1], "1510k");
    }

    #[test]
    fn audio_args_output_remains_last() {
        let opts = ConvertOptions {
            output_format: "opus".to_string(),
            opus_application: Some("audio".to_string()),
            opus_vbr: Some(false),
            channels: Some("stereo".to_string()),
            bitrate: Some(128),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.opus", &opts);
        assert_eq!(args.last().unwrap(), "out.opus");
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
