#[allow(unused_imports)]
use librewin_common::media::media_type_for; // used in tests via super::*
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{command, AppHandle, Emitter, Manager, State, Window};
use ts_rs::TS;

pub mod args;
pub mod convert;
pub mod fs_commands;
pub mod operations;
pub mod presets;
pub mod preview;
pub mod probe;
pub mod theme;
pub use args::{
    assimp_format_id, build_assimp_args, build_ffmpeg_audio_args, build_ffmpeg_video_args,
    build_image_magick_args, ffmpeg_video_codec_args, resolution_to_scale,
};
use convert::{
    run_archive_convert, run_audio_convert, run_data_convert, run_document_convert,
    run_ebook_convert, run_email_convert, run_font_convert, run_image_convert, run_model_convert,
    run_notebook_convert, run_subtitle_convert, run_timeline_convert, run_tracker_convert,
    run_video_convert,
};
pub use fs_commands::{file_exists, scan_dir};
pub use presets::{delete_preset, list_presets, save_preset};
pub use preview::{preview_diff, preview_image_quality};
pub use probe::{
    cancel_filmstrip, get_file_info, get_filmstrip, get_spectrogram, get_waveform, FilmstripCancels,
};
pub use theme::{get_accent, get_theme};

// ── AppState ───────────────────────────────────────────────────────────────────

pub struct AppState {
    pub processes: Arc<Mutex<HashMap<String, Child>>>,
    pub cancellations: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Serialize, Clone, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub(crate) struct JobProgress {
    pub(crate) job_id: String,
    pub(crate) percent: f32,
    pub(crate) message: String,
}

#[derive(Serialize, Clone, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub(crate) struct JobDone {
    pub(crate) job_id: String,
    pub(crate) output_path: String,
}

#[derive(Serialize, Clone, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub(crate) struct JobError {
    pub(crate) job_id: String,
    pub(crate) message: String,
}

#[derive(Serialize, Clone, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub(crate) struct JobCancelled {
    pub(crate) job_id: String,
}

/// Typed terminal outcome for a background job. Replaces the prior
/// `Err("CANCELLED")` / `Err("__DONE__")` string sentinels that crossed the
/// `convert_file` / `run_operation` boundary — the stringly-typed flow let
/// the two finalizers diverge on which sentinel they matched.
#[derive(Debug)]
pub(crate) enum JobOutcome {
    /// Natural completion. Finalizer emits `job-done` and writes the log.
    Done { output_path: String },
    /// Subsystem already emitted `job-done` itself (e.g. archive extract with
    /// a folder path). Finalizer writes the log but does not re-emit.
    DoneEmitted,
    /// User cancelled. Finalizer removes `remove_path` if set, emits
    /// `job-cancelled`, and writes the log.
    Cancelled { remove_path: Option<String> },
    /// Job failed. Finalizer emits `job-error` with `message` and writes the
    /// log with the first line only.
    Error { message: String },
}

/// Typed terminal state for a `convert_file` runner. Replaces the legacy
/// `Result<(), String>` + string sentinels (`"CANCELLED"`, `"__DONE__"`)
/// that used to cross the runner/dispatcher boundary. Every convert/ runner
/// that participates in the `convert_file` path now returns this enum,
/// making cancellation a compiler-enforced state rather than a string
/// pattern match.
#[derive(Debug)]
pub enum ConvertResult {
    /// Natural completion — the dispatcher attaches the output path and
    /// maps to `JobOutcome::Done`.
    Done,
    /// Runner already emitted `job-done` itself (archive extract with a
    /// folder path). Maps to `JobOutcome::DoneEmitted`.
    DoneEmitted,
    /// User cancelled mid-run. Maps to `JobOutcome::Cancelled`; the
    /// dispatcher attaches the partial output path for cleanup.
    Cancelled,
    /// Runner failed. Maps to `JobOutcome::Error`.
    Error(String),
}

/// Convert a `run_operation`-dispatched `Result<(), String>` to a typed
/// `JobOutcome`. Each dispatch arm knows its `output_path`; the `Result`
/// arm determines the terminal state. `Err("CANCELLED")` is the single
/// cancellation sentinel emitted by `run_ffmpeg` in `operations/mod.rs`.
fn op_result(result: Result<(), String>, output_path: String) -> JobOutcome {
    match result {
        Ok(()) => JobOutcome::Done { output_path },
        Err(msg) if msg == "CANCELLED" => JobOutcome::Cancelled { remove_path: None },
        Err(msg) => JobOutcome::Error { message: msg },
    }
}

/// Single finalizer for background jobs. Writes `fade.log` and emits the
/// matching IPC event for every terminal state. Both `convert_file` and
/// `run_operation` route through here so log and event behavior cannot
/// silently diverge (F-05) and so a future sentinel string cannot fall
/// through to `job-error` because one path forgot to match it (F-29).
fn finalize_job(window: &Window, job_id: String, input_path: &str, outcome: JobOutcome) {
    match outcome {
        JobOutcome::Done { output_path } => {
            write_fade_log(&format_log_entry(&job_id, input_path, "done", &output_path));
            let _ = window.emit(
                "job-done",
                JobDone {
                    job_id,
                    output_path,
                },
            );
        }
        JobOutcome::DoneEmitted => {
            write_fade_log(&format_log_entry(&job_id, input_path, "done", ""));
        }
        JobOutcome::Cancelled { remove_path } => {
            if let Some(p) = remove_path.as_deref() {
                let _ = std::fs::remove_file(p);
            }
            write_fade_log(&format_log_entry(&job_id, input_path, "cancelled", ""));
            let _ = window.emit("job-cancelled", JobCancelled { job_id });
        }
        JobOutcome::Error { message } => {
            let first_line = message.lines().next().unwrap_or("").to_string();
            write_fade_log(&format_log_entry(&job_id, input_path, "error", &first_line));
            let _ = window.emit("job-error", JobError { job_id, message });
        }
    }
}

#[derive(Deserialize, Clone, TS)]
#[ts(export, export_to = "../../src/lib/types/generated/")]
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
    pub normalize_lufs: Option<f64>, // LUFS target (e.g. -16.0), None = default -16.0
    pub normalize_true_peak: Option<f64>, // dBTP ceiling (e.g. -1.0), None = default -1.0
    // DSP
    pub dsp_highpass_freq: Option<f64>, // Hz — Butterworth 2-pole highpass, None = off
    pub dsp_lowpass_freq: Option<f64>,  // Hz — Butterworth 2-pole lowpass,  None = off
    pub dsp_stereo_width: Option<f64>,  // −100=mono  0=no change  +100=wide, None = off
    pub dsp_limiter_db: Option<f64>,    // dBFS ceiling (e.g. -1.0),          None = off
    // Data
    pub pretty_print: Option<bool>,
    pub csv_delimiter: Option<String>,
    // Archive
    pub archive_operation: Option<String>,
    pub archive_compression: Option<u32>, // 0-9, zip/gz/7z level
    // Output naming
    pub output_suffix: Option<String>,
    pub output_separator: Option<String>,
    // Metadata — when false, strip EXIF/tags/etc. None or true = preserve.
    // Applies to image (ImageMagick -strip) and video/audio (ffmpeg -map_metadata -1).
    pub preserve_metadata: Option<bool>,

    // ── Format-specific audio controls (see docs/FORMAT-CONTROLS.md §2) ──
    pub channels: Option<String>, // "source" | "mono" | "stereo" | "joint" | "5.1"
    pub bit_depth: Option<u32>,   // 16 | 24 | 32
    pub mp3_bitrate_mode: Option<String>, // "cbr" | "vbr"
    pub mp3_vbr_quality: Option<u32>, // 0-9
    pub flac_compression: Option<u32>, // 0-8
    pub ogg_bitrate_mode: Option<String>, // "vbr" | "cbr" | "abr"
    pub ogg_vbr_quality: Option<i32>, // -1..=10
    pub aac_profile: Option<String>, // "lc" | "he" | "hev2"
    pub opus_application: Option<String>, // "audio" | "voip" | "lowdelay"
    pub opus_vbr: Option<bool>,
    pub m4a_subcodec: Option<String>, // "aac" | "alac"
    pub wma_mode: Option<String>,     // "standard" | "pro" | "lossless"
    pub ac3_bitrate: Option<u32>,     // 192 | 384 | 448 | 640 (kbps)
    pub dts_bitrate: Option<u32>,     // 754 | 1510 (kbps)

    // ── Format-specific video controls ──
    pub crf: Option<u32>,                  // 0-51
    pub preset: Option<String>,            // "ultrafast" | "fast" | "medium" | "slow" | "veryslow"
    pub h264_profile: Option<String>, // "baseline" | "main" | "high". Auto-promoted to high422/high444 when pix_fmt is yuv422p/yuv444p.
    pub pix_fmt: Option<String>,      // "yuv420p" | "yuv422p" | "yuv444p"
    pub tune: Option<String>,         // "none" | "film" | "animation" | "grain"
    pub frame_rate: Option<String>,   // "original" | "24" | "25" | "30" | "60"
    pub webm_bitrate_mode: Option<String>, // "crf" | "cbr" | "cvbr"
    pub webm_video_bitrate: Option<u32>, // kbps, cbr/cvbr only
    pub vp9_speed: Option<u32>,       // 0-5
    pub av1_speed: Option<u32>,       // 0-10
    pub mkv_subtitle: Option<String>, // "none" | "copy" | "burn"
    pub avi_video_bitrate: Option<u32>, // kbps
    pub gif_width: Option<String>,    // "original" | "320" | "480" | "640"
    pub gif_fps: Option<String>,      // "original" | "5" | "10" | "15"
    pub gif_loop: Option<String>,     // "infinite" | "once" | "none"
    pub gif_palette_size: Option<u32>, // 32 | 64 | 128 | 256
    pub gif_dither: Option<String>,   // "none" | "bayer" | "floyd"

    // ── Professional video codec controls ──
    pub hap_format: Option<String>, // "hap" | "hap_alpha" | "hap_q" | "hap_q_alpha"
    /// Requires output resolution ≥ 1280×720; returns error if opts.resolution is set below this.
    pub dnxhr_profile: Option<String>, // "dnxhr_lb" | "dnxhr_sq" | "dnxhr_hq" | "dnxhr_hqx" | "dnxhr_444"
    pub dnxhd_bitrate: Option<u32>, // Mbps: 36 | 115 | 120 | 145 | 175 | 185 | 220
    pub dv_standard: Option<String>, // "ntsc" | "pal"
    pub video_bitrate_mode: Option<String>, // "crf" | "vbr" | "cbr"
    pub video_bitrate: Option<u32>, // kbps — used when mode is "vbr" or "cbr"
    pub prores_profile: Option<u32>, // 0=Proxy 1=LT 2=422 3=HQ 4=4444 5=4444XQ

    // ── Format-specific image controls ──
    pub jpeg_chroma: Option<String>, // "420" | "422" | "444"
    pub jpeg_progressive: Option<bool>,
    pub png_compression: Option<u32>,   // 0-9
    pub png_color_mode: Option<String>, // "rgb" | "rgba" | "gray" | "graya" | "palette"
    pub png_interlaced: Option<bool>,
    pub tiff_compression: Option<String>, // "none" | "lzw" | "deflate" | "packbits"
    pub tiff_bit_depth: Option<u32>,      // 8 | 16 | 32
    pub tiff_color_mode: Option<String>,  // "rgb" | "cmyk" | "gray"
    pub webp_lossless: Option<bool>,
    pub webp_method: Option<u32>,    // 0-6
    pub avif_speed: Option<u32>,     // 0–9; clamped at 9 (libheif limit)
    pub avif_chroma: Option<String>, // "420" | "422" | "444"
    pub bmp_bit_depth: Option<u32>,  // 8 | 16 | 24 | 32

    // ── Silence padding (audio only) ──
    pub pad_front: Option<f64>, // seconds of silence prepended
    pub pad_end: Option<f64>,   // seconds of silence appended
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
            output_separator: None,
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
            webm_video_bitrate: None,
            vp9_speed: None,
            av1_speed: None,
            mkv_subtitle: None,
            avi_video_bitrate: None,
            gif_width: None,
            gif_fps: None,
            gif_loop: None,
            gif_palette_size: None,
            gif_dither: None,
            hap_format: None,
            dnxhr_profile: None,
            dnxhd_bitrate: None,
            dv_standard: None,
            video_bitrate_mode: None,
            video_bitrate: None,
            prores_profile: None,

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

            pad_front: None,
            pad_end: None,
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

/// Probe the first video stream of a file and return `(width, height)`.
/// Returns `None` if ffprobe is unavailable, the file has no video stream,
/// or the JSON is malformed.
pub(crate) fn probe_video_dimensions(path: &str) -> Option<(u32, u32)> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "v:0",
            path,
        ])
        .output()
        .ok()?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let streams = json["streams"].as_array()?;
    let stream = streams.first()?;
    let w = stream["width"].as_u64()? as u32;
    let h = stream["height"].as_u64()? as u32;
    Some((w, h))
}

/// Build the output path: same dir as input (or output_dir), stem + suffix + new ext.
fn build_output_path(
    input: &str,
    new_ext: &str,
    output_dir: Option<&str>,
    suffix: &str,
    separator: &str,
) -> String {
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
        format!("{}/{}{}{}.{}", dir, stem, separator, suffix, new_ext)
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

/// Validate the separator: at most one safe character (alphanumeric, hyphen, underscore, dot).
fn validate_separator(sep: &str) -> Result<(), String> {
    if sep.chars().count() > 1 {
        return Err(format!("Separator '{}' must be a single character", sep));
    }
    if sep
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        Ok(())
    } else {
        Err(format!(
            "Invalid separator '{}': only letters, digits, hyphen, underscore, or dot allowed",
            sep
        ))
    }
}

/// Validate that `path` is a safe output file path:
/// - no `..` traversal in any component
/// - filename stem contains only ASCII alphanumeric, hyphen, underscore, or dot
///
/// Covers the 29 `run_operation` variants that receive full output paths directly
/// (unlike `convert_file`, which validates suffix + separator separately).
pub(crate) fn validate_output_name(path: &str) -> Result<(), String> {
    use std::path::{Component, Path};
    let p = Path::new(path);
    for component in p.components() {
        if component == Component::ParentDir {
            return Err(format!("Path traversal rejected in output path: {}", path));
        }
    }
    let parent = p
        .parent()
        .and_then(|parent| parent.to_str())
        .ok_or_else(|| {
            format!(
                "Cannot determine parent directory from output path: {}",
                path
            )
        })?;
    validate_output_dir(parent)?;
    validate_output_filename(path)
}

fn validate_output_filename(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("Cannot determine filename from output path: {}", path))?;
    if stem.is_empty() {
        return Err(format!("Empty filename in output path: {}", path));
    }
    if stem.starts_with('.')
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!(
            "Invalid output filename '{}': stem must start with a letter or digit and contain only letters, digits, hyphens, underscores, or dots",
            stem
        ));
    }
    Ok(())
}

/// Validate that `dir` is a safe output directory:
/// - no `..` traversal in any component
/// - directory falls within an allowed root (HOME, TMPDIR, /Volumes, /media, /mnt)
///
/// Allowed-root list mirrors `assetProtocol.scope` from B4 (F-08).
pub(crate) fn validate_output_dir(dir: &str) -> Result<(), String> {
    use std::path::{Component, Path};
    let p = Path::new(dir);
    for component in p.components() {
        if component == Component::ParentDir {
            return Err(format!(
                "Path traversal rejected in output directory: {}",
                dir
            ));
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let tmpdir = std::env::temp_dir().to_string_lossy().into_owned();
    let allowed_fixed: &[&str] = &["/Volumes", "/media", "/mnt"];
    let under_allowed = (!home.is_empty() && p.starts_with(&home))
        || (!tmpdir.is_empty() && p.starts_with(&tmpdir))
        || allowed_fixed.iter().any(|root| p.starts_with(root));
    if !under_allowed {
        return Err(format!(
            "Output directory '{}' is outside allowed roots (HOME, TMPDIR, /Volumes, /media, /mnt)",
            dir
        ));
    }
    Ok(())
}

/// Reject any path containing a `..` component. Applied at IPC entry points
/// that accept user-supplied input paths to prevent directory traversal reads.
pub(crate) fn validate_no_traversal(path: &str) -> Result<(), String> {
    use std::path::{Component, Path};
    for component in Path::new(path).components() {
        if component == Component::ParentDir {
            return Err(format!("Path traversal rejected: {}", path));
        }
    }
    Ok(())
}

/// Validate a frontend-supplied input/read path:
/// - no `..` traversal in any component
/// - path or its parent sits inside the same allowed roots as output paths
///
/// This intentionally does not require the file to exist; some tests and
/// callers validate before the external tool creates or probes the path.
pub(crate) fn validate_input_path(path: &str) -> Result<(), String> {
    validate_no_traversal(path)?;
    let p = std::path::Path::new(path);
    let dir_for_check = if p.is_dir() {
        p
    } else {
        p.parent().ok_or_else(|| {
            format!(
                "Cannot determine parent directory from input path: {}",
                path
            )
        })?
    };
    let dir = dir_for_check
        .to_str()
        .ok_or_else(|| format!("Input path is not valid UTF-8: {}", path))?;
    validate_output_dir(dir).map_err(|e| e.replace("Output directory", "Input path directory"))
}

/// Validate the trio of paths the `convert_file` IPC entry assembles before
/// touching the IO layer:
/// - `input_path`: must not contain `..`
/// - `output_dir` (if specified): must be inside an allowed root and free of `..`
/// - `output_path`: file-name component must be a safe stem
///
/// `convert_file` itself runs these checks inline at the call sites where each
/// value becomes available (the seq-dir branch validates its base name before
/// `create_dir_all`, etc.) — this helper exists so unit tests can pin the
/// CLAUDE.md "validate before any CLI arg interpolation" contract for the
/// conversion path without needing a Tauri runtime.
#[cfg(test)]
pub(crate) fn validate_convert_inputs(
    input_path: &str,
    output_dir: Option<&str>,
    output_path: &str,
) -> Result<(), String> {
    validate_no_traversal(input_path)?;
    validate_input_path(input_path)?;
    if let Some(dir) = output_dir {
        validate_output_dir(dir)?;
    }
    if let Some(name) = std::path::Path::new(output_path)
        .file_name()
        .and_then(|n| n.to_str())
    {
        validate_output_filename(name)?;
    }
    Ok(())
}

/// Parse out_time_ms line from ffmpeg -progress output to get elapsed seconds.
pub(crate) fn parse_out_time_ms(line: &str) -> Option<f64> {
    let val = line.strip_prefix("out_time_ms=")?;
    val.trim().parse::<f64>().ok().map(|ms| ms / 1_000_000.0)
}

/// Classify a file extension into a media type, covering all types Fade supports.
pub(crate) fn classify_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "png" | "webp" | "tiff" | "tif" | "bmp" | "gif" | "avif" | "heic"
        | "heif" | "psd" | "svg" | "ico" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng"
        | "orf" | "rw2" | "exr" | "hdr" | "dds" | "xcf" | "jxl" => "image",
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v" | "flv" | "wmv" | "ts" | "mpg" | "mpeg"
        | "3gp" | "ogv" | "divx" | "rmvb" | "asf" => "video",
        "seq_png" | "seq_jpg" | "seq_tiff" => "video",
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "opus" | "m4a" | "wma" | "aiff" => "audio",
        "csv" | "json" | "xml" | "yaml" | "yml" | "toml" | "tsv" | "ndjson" | "jsonl" => "data",
        "md" | "markdown" | "html" | "htm" | "txt" => "document",
        "zip" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "rar" | "iso" | "dmg" | "cbz"
        | "cbr" => "archive",
        // 3D models — routed through assimp CLI. See args/model.rs for
        // the full extension→assimp-format-id mapping.
        "obj" | "stl" | "ply" | "gltf" | "glb" | "dae" | "fbx" | "3ds" | "x3d" => "model",
        // Font containers — routed through fonttools (Python). See
        // convert/font.rs for the ttf/otf/woff/woff2 matrix.
        "ttf" | "otf" | "woff" | "woff2" => "font",
        // Timeline / edit-decision-list formats — routed through
        // OpenTimelineIO's `otioconvert` CLI. `xml` is deliberately not
        // listed here: the extension is ambiguous (Premiere XML vs.
        // generic data XML) and already routes to "data". Premiere XML
        // interop is deferred until we have a disambiguation signal.
        "edl" | "fcpxml" | "otio" | "aaf" => "timeline",
        // Subtitle files — routed through ffmpeg. `ttml` is ffmpeg-native
        // for write. `sbv` is hand-rolled in convert/subtitle.rs (ffmpeg
        // can't read or write it) via a SRT bridge step.
        "srt" | "vtt" | "ass" | "ssa" | "ttml" | "sbv" => "subtitle",
        // Ebooks — Calibre's `ebook-convert` CLI handles the whole matrix.
        // `pdf` stays routed to "document"; Calibre reads PDFs for ebook
        // OUTPUT via the input-extension branch isn't needed because pdf
        // output still targets the document pipeline.
        "epub" | "mobi" | "azw3" | "fb2" | "lit" => "ebook",
        // Email — pure-Rust eml ↔ mbox. `msg` is deferred.
        "eml" | "mbox" => "email",
        // Tracker / MIDI — routed by INPUT extension (see convert_file), but
        // classify_ext is also queried for UI compat matrices. `.sf2` is a
        // soundfont container, not convertible — see convert/tracker.rs.
        "mid" | "midi" | "mod" | "xm" | "it" | "s3m" | "sf2" => "tracker",
        // Data-nerd formats — `.sqlite` and `.parquet` are routed by INPUT
        // extension (see convert_file) into the data pipeline. Listed here so
        // UI-side classification doesn't treat them as unknown.
        "sqlite" | "sqlite3" | "db" | "parquet" => "data",
        _ => "unknown",
    }
}

/// Check whether a tool is available in PATH.
pub(crate) fn tool_available(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Byte-threshold for rotating `fade.log`. Roughly 200+ entries at ~300 B each.
const FADE_LOG_MAX_BYTES: u64 = 64 * 1024;

fn fade_log_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".config")
        .join("librewin")
        .join("fade.log")
}

/// Append an entry to ~/.config/librewin/fade.log. Rotates to `fade.log.1` when the
/// file exceeds FADE_LOG_MAX_BYTES. O_APPEND keeps concurrent finalizer writes safe.
fn write_fade_log(entry: &str) {
    append_fade_log_at(&fade_log_path(), entry);
}

fn append_fade_log_at(path: &std::path::Path, entry: &str) {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > FADE_LOG_MAX_BYTES {
            let rotated = path.with_extension("log.1");
            let _ = std::fs::remove_file(&rotated);
            let _ = std::fs::rename(path, &rotated);
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        // Pre-assemble so the whole line lands in a single atomic write() on
        // append-mode descriptors — `writeln!` splits the format string into
        // multiple write_all calls, which interleave under concurrent finalizers.
        let mut line = String::with_capacity(entry.len() + 1);
        line.push_str(entry);
        line.push('\n');
        let _ = f.write_all(line.as_bytes());
    }
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
    validate_input_path(&input_path)?;

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

    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!("Invalid output format: {ext}"));
    }

    if ext == "mp3" {
        if let Some(sr) = options.sample_rate {
            const MP3_SAMPLE_RATES: &[u32] =
                &[8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];
            if !MP3_SAMPLE_RATES.contains(&sr) {
                return Err(format!("Unsupported MP3 sample rate: {sr} Hz"));
            }
        }
    }

    // Route by INPUT extension first for pipelines where the backend is
    // picked by what we're reading, not what we're writing (ipynb → md/py/html
    // would otherwise route through the document pipeline, which doesn't
    // know how to parse a Jupyter notebook). Mirror the extract_audio
    // pattern below.
    let input_ext = Path::new(&input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Route by input media type when extract_audio is set (input is video, output is audio)
    // `.xml` is ambiguous between Premiere XML (timeline) and generic data
    // XML. Disambiguate by the opposite extension: when either side is a
    // known timeline-native format, the other side's `xml` means Premiere
    // XML. otioconvert auto-picks the fcp_xml adapter from the extension.
    let is_timeline_native = |e: &str| matches!(e, "edl" | "fcpxml" | "otio" | "aaf");
    // Tracker / MIDI lane: input extension determines the pipeline regardless
    // of the audio target (wav/mp3/flac/etc. would otherwise route to "audio"
    // and hand the raw .mid to ffmpeg, which can't decode it).
    let is_tracker_input = |e: &str| matches!(e, "mid" | "midi" | "mod" | "xm" | "it" | "s3m");
    // Data-nerd lane: sqlite/parquet need an input-driven route because their
    // output formats (csv/json/tsv/xml) collide with the generic data pipeline.
    let is_data_nerd_input = |e: &str| matches!(e, "sqlite" | "sqlite3" | "db" | "parquet");
    let mtype = if options.extract_audio == Some(true) {
        "audio"
    } else if input_ext == "ipynb" {
        "notebook"
    } else if is_tracker_input(&input_ext) {
        "tracker"
    } else if is_data_nerd_input(&input_ext) {
        "data"
    } else if (ext == "xml" && is_timeline_native(&input_ext))
        || (input_ext == "xml" && is_timeline_native(&ext))
    {
        "timeline"
    } else {
        let t = classify_ext(&ext);
        if t == "unknown" {
            return Err(format!("Unsupported output format: {ext}"));
        }
        t
    };

    let suffix = options.output_suffix.as_deref().unwrap_or("converted");
    validate_suffix(suffix)?;
    let separator = options.output_separator.as_deref().unwrap_or("_");
    validate_separator(separator)?;
    if let Some(dir) = options.output_dir.as_deref() {
        validate_output_dir(dir)?;
    }

    let output_path = if let Some(real_ext) = ext.strip_prefix("seq_") {
        // Image sequences go to a directory of frames rather than a single file.
        // Build the directory path, create it, and pass it to the converter which
        // will append the frame pattern (frame_%04d.ext) internally.
        // real_ext is "png", "jpg", or "tiff"
        let p = Path::new(&input_path);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        let dir_base = options
            .output_dir
            .as_deref()
            .map(|d| d.to_string())
            .unwrap_or_else(|| {
                p.parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
            });
        let seq_dir = if suffix.is_empty() {
            format!("{dir_base}/{stem}_{real_ext}_frames")
        } else {
            format!("{dir_base}/{stem}{separator}{suffix}_{real_ext}_frames")
        };
        // Validate the directory's base name before creating it on disk —
        // create_dir_all is the IO sink the CLAUDE.md rule guards against.
        if let Some(name) = std::path::Path::new(&seq_dir)
            .file_name()
            .and_then(|n| n.to_str())
        {
            validate_output_filename(name)?;
        }
        std::fs::create_dir_all(&seq_dir)
            .map_err(|e| format!("Cannot create sequence output directory: {e}"))?;
        seq_dir
    } else {
        build_output_path(
            &input_path,
            &ext,
            options.output_dir.as_deref(),
            suffix,
            separator,
        )
    };

    // Validate the assembled output file/directory base name. The seq branch
    // produces a directory whose base name (`{stem}{sep}{suffix}_{ext}_frames`)
    // still needs the same character-set check as a normal output filename —
    // it lands in `std::fs::create_dir_all` upstream of any CLI invocation.
    if ext.starts_with("seq_") {
        validate_output_dir(&output_path)?;
    } else {
        validate_output_name(&output_path)?;
    }

    // Register cancellation flag before spawning the thread
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.clone(), Arc::clone(&cancelled));
    }

    // Clone arcs so they can be moved into the thread
    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        // Bridge for convert/ runners that still return `Result<(), String>`
        // (data, document, email, subtitle — they have no cancellation path
        // since they don't spawn external processes the user can cancel).
        // The `convert_file` runners tracked by TASK-2 return `ConvertResult`
        // directly.
        fn lift(result: Result<(), String>) -> ConvertResult {
            match result {
                Ok(()) => ConvertResult::Done,
                Err(msg) => ConvertResult::Error(msg),
            }
        }

        let result: ConvertResult = match mtype {
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
            "data" => lift(run_data_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
            )),
            "document" => lift(run_document_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
            )),
            "archive" => run_archive_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "model" => run_model_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "font" => run_font_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "tracker" => run_tracker_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "timeline" => run_timeline_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "subtitle" => lift(run_subtitle_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            )),
            "ebook" => run_ebook_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            "email" => lift(run_email_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
            )),
            "notebook" => run_notebook_convert(
                &window,
                &job_id,
                &input_path,
                &output_path,
                &options,
                Arc::clone(&processes),
                Arc::clone(&cancelled),
            ),
            _ => ConvertResult::Error("Unsupported format".to_string()),
        };

        // Clean up cancellation registry entry
        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        // Map the typed runner outcome into the shared `JobOutcome`. For the
        // `Cancelled` arm, convert-file owns its output — remove the partial
        // file on cancel so a re-run cannot pick up a half-encoded output.
        let outcome = match result {
            ConvertResult::Done => JobOutcome::Done {
                output_path: output_path.clone(),
            },
            ConvertResult::DoneEmitted => JobOutcome::DoneEmitted,
            ConvertResult::Cancelled => JobOutcome::Cancelled {
                remove_path: Some(output_path.clone()),
            },
            ConvertResult::Error(msg) => JobOutcome::Error { message: msg },
        };
        finalize_job(&window, job_id, &input_path, outcome);
    });

    Ok(())
}

/// Cancel a running job by killing its subprocess.
fn cancel_job_impl(state: &AppState, job_id: &str) -> Result<(), String> {
    // Set the cancelled flag first so the background thread knows why it stopped.
    {
        let map = state.cancellations.lock();
        if let Some(flag) = map.get(job_id) {
            flag.store(true, Ordering::SeqCst);
        }
    }

    // Kill the child process but leave the handle registered. The worker that
    // inserted it remains responsible for removing it from the map and waiting
    // on it, which prevents killed children from becoming unreaped zombies.
    {
        let mut map = state.processes.lock();
        if let Some(child) = map.get_mut(job_id) {
            let _ = child.kill();
        }
    }

    Ok(())
}

#[command]
fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    cancel_job_impl(&state, &job_id)
}

/// Check whether required external tools are available in PATH.
#[command]
fn check_tools() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "ffmpeg":   tool_available("ffmpeg"),
        "ffprobe":  tool_available("ffprobe"),
        "magick":   tool_available("magick"),
        "sevenzip": tool_available("7z") || tool_available("7zz"),
    }))
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

// ── Persistent diagnostics ───────────────────────────────────────────────────
// Entries written as JSON Lines (one JSON object per line) under the Tauri
// app-log directory. The frontend ring buffer mirrors what's on disk so the
// Diagnostics panel shows history across restarts. Rotation: when the file
// exceeds DIAG_MAX_BYTES it's renamed to `.1` (the previous `.1` is dropped),
// so at most ~2× the cap exists at any time. Local-only — no network.

const DIAG_FILE: &str = "diagnostics.jsonl";
const DIAG_MAX_BYTES: u64 = 5 * 1024 * 1024;
const DIAG_LOAD_MAX: usize = 100;

fn diag_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("resolve app_log_dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    Ok(dir.join(DIAG_FILE))
}

#[command]
fn diag_append(app: AppHandle, entry: serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    let path = diag_path(&app)?;
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > DIAG_MAX_BYTES {
            let rotated = path.with_extension("jsonl.1");
            let _ = std::fs::remove_file(&rotated);
            let _ = std::fs::rename(&path, &rotated);
        }
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
fn diag_load(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    use std::io::{BufRead, BufReader};
    let path = diag_path(&app)?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let f = std::fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let lines: Vec<String> = BufReader::new(f).lines().map_while(Result::ok).collect();
    let start = lines.len().saturating_sub(DIAG_LOAD_MAX);
    let mut out = Vec::with_capacity(lines.len() - start);
    for l in &lines[start..] {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
            out.push(v);
        }
    }
    Ok(out)
}

#[command]
fn diag_clear(app: AppHandle) -> Result<(), String> {
    let path = diag_path(&app)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    let rotated = path.with_extension("jsonl.1");
    if rotated.exists() {
        let _ = std::fs::remove_file(&rotated);
    }
    Ok(())
}

// ── Operations ────────────────────────────────────────────────────────────────

#[derive(Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, export_to = "../../src/lib/types/generated/")]
pub(crate) enum OperationPayload {
    Rewrap {
        input_path: String,
        output_path: String,
    },
    Extract {
        input_path: String,
        stream_index: u32,
        stream_type: String,
        output_path: String,
    },
    ExtractMulti {
        input_path: String,
        streams: Vec<operations::extract::ExtractStreamSpec>,
    },
    Cut {
        input_path: String,
        start_secs: Option<f64>,
        end_secs: Option<f64>,
        output_path: String,
    },
    Split {
        input_path: String,
        timecodes_secs: Vec<f64>,
        output_dir: String,
    },
    AudioOffset {
        input_path: String,
        offset_ms: i32,
        output_path: String,
    },
    ReplaceAudio {
        video_path: String,
        audio_path: String,
        output_path: String,
    },
    Merge {
        input_paths: Vec<String>,
        output_path: String,
    },
    AudioNormalize {
        input_path: String,
        output_path: String,
        mode: operations::analysis::audio_norm::NormMode,
        target_i: f64,
        target_tp: f64,
        target_lra: f64,
        linear: bool,
    },
    SilenceRemove {
        input_path: String,
        output_path: String,
        threshold_db: f64,
        min_silence_s: f64,
        pad_ms: u32,
    },
    Conform {
        input_path: String,
        output_path: String,
        /// Target framerate as a UI-friendly string ("23.976", "60", ...).
        /// `None` = keep source rate.
        fps: Option<String>,
        /// Target resolution "WxH". `None` = keep source.
        resolution: Option<String>,
        /// Target pixel format ("yuv420p", "yuv420p10le", ...). `None` = keep source.
        pix_fmt: Option<String>,
        fps_algo: operations::conform::FpsAlgo,
        scale_algo: operations::conform::ScaleAlgo,
        dither: bool,
    },
    RemoveAudio {
        input_path: String,
        output_path: String,
    },
    RemoveVideo {
        input_path: String,
        output_path: String,
    },
    MetadataStrip {
        input_path: String,
        output_path: String,
        /// "all" strips everything; "title" strips everything but re-writes
        /// the container title tag with `title_value`.
        mode: String,
        title_value: Option<String>,
    },
    Loop {
        input_path: String,
        output_path: String,
        /// Total playthroughs (2..=50).
        count: u32,
    },
    RotateFlip {
        input_path: String,
        output_path: String,
        /// "cw90" | "ccw90" | "180" | "hflip" | "vflip"
        mode: String,
    },
    Reverse {
        input_path: String,
        output_path: String,
    },
    Speed {
        input_path: String,
        output_path: String,
        rate: f64,
    },
    Fade {
        input_path: String,
        output_path: String,
        fade_in: f64,
        fade_out: f64,
    },
    Deinterlace {
        input_path: String,
        output_path: String,
        /// "yadif" | "yadif_double" | "bwdif"
        mode: String,
    },
    Denoise {
        input_path: String,
        output_path: String,
        /// "light" | "medium" | "strong"
        preset: String,
    },
    Thumbnail {
        input_path: String,
        output_path: String,
        time_spec: String,
        format: String, // jpeg · png · webp
    },
    ContactSheet {
        input_path: String,
        output_path: String,
        cols: u32,
        rows: u32,
        frames: u32,
    },
    FrameExport {
        input_path: String,
        output_dir: String,
        /// "fps" | "interval"
        mode: String,
        value: f64,
        format: String, // jpeg · png · webp
    },
    Watermark {
        input_path: String,
        output_path: String,
        watermark_path: String,
        corner: String,
        opacity: f64,
        scale_pct: f64,
    },
    Volume {
        input_path: String,
        output_path: String,
        gain_db: f64,
    },
    ChannelTools {
        input_path: String,
        output_path: String,
        /// stereo_to_mono · swap · mute_l · mute_r · mono_to_stereo
        mode: String,
    },
    PadSilence {
        input_path: String,
        output_path: String,
        head_s: f64,
        tail_s: f64,
    },
    ChromaKey {
        input_path: String,
        output_path: String,
        algo: operations::chroma_key::ChromaAlgo,
        color_hex: String,
        similarity: f64,
        blend: f64,
        despill: bool,
        despill_mix: f64,
        upsample: bool,
        output_target: operations::chroma_key::ChromaOutput,
        trim_start: Option<f64>,
        trim_end: Option<f64>,
    },
}

impl OperationPayload {
    /// Validate all input/read paths in this payload before any subprocess sees
    /// them. Additional paths like watermark/audio replacement inputs are read
    /// boundaries too, so keep them in this pass rather than only checking the
    /// primary media file.
    pub(crate) fn validate_inputs(&self) -> Result<(), String> {
        use OperationPayload::*;
        match self {
            Rewrap { input_path, .. }
            | Extract { input_path, .. }
            | ExtractMulti { input_path, .. }
            | Cut { input_path, .. }
            | Split { input_path, .. }
            | AudioOffset { input_path, .. }
            | AudioNormalize { input_path, .. }
            | SilenceRemove { input_path, .. }
            | Conform { input_path, .. }
            | RemoveAudio { input_path, .. }
            | RemoveVideo { input_path, .. }
            | MetadataStrip { input_path, .. }
            | Loop { input_path, .. }
            | RotateFlip { input_path, .. }
            | Reverse { input_path, .. }
            | Speed { input_path, .. }
            | Fade { input_path, .. }
            | Deinterlace { input_path, .. }
            | Denoise { input_path, .. }
            | Thumbnail { input_path, .. }
            | ContactSheet { input_path, .. }
            | FrameExport { input_path, .. }
            | Volume { input_path, .. }
            | ChannelTools { input_path, .. }
            | PadSilence { input_path, .. }
            | ChromaKey { input_path, .. } => validate_input_path(input_path),

            ReplaceAudio {
                video_path,
                audio_path,
                ..
            } => {
                validate_input_path(video_path)?;
                validate_input_path(audio_path)
            }

            Merge { input_paths, .. } => {
                for input_path in input_paths {
                    validate_input_path(input_path)?;
                }
                Ok(())
            }

            Watermark {
                input_path,
                watermark_path,
                ..
            } => {
                validate_input_path(input_path)?;
                validate_input_path(watermark_path)
            }
        }
    }

    /// Validate all output paths/dirs in this payload before the worker thread
    /// is spawned. Returns the first validation error encountered.
    pub(crate) fn validate_outputs(&self) -> Result<(), String> {
        use OperationPayload::*;
        match self {
            Rewrap { output_path, .. }
            | Extract { output_path, .. }
            | Cut { output_path, .. }
            | AudioOffset { output_path, .. }
            | ReplaceAudio { output_path, .. }
            | Merge { output_path, .. }
            | AudioNormalize { output_path, .. }
            | SilenceRemove { output_path, .. }
            | Conform { output_path, .. }
            | RemoveAudio { output_path, .. }
            | RemoveVideo { output_path, .. }
            | MetadataStrip { output_path, .. }
            | Loop { output_path, .. }
            | RotateFlip { output_path, .. }
            | Reverse { output_path, .. }
            | Speed { output_path, .. }
            | Fade { output_path, .. }
            | Deinterlace { output_path, .. }
            | Denoise { output_path, .. }
            | Thumbnail { output_path, .. }
            | ContactSheet { output_path, .. }
            | Watermark { output_path, .. }
            | Volume { output_path, .. }
            | ChannelTools { output_path, .. }
            | PadSilence { output_path, .. } => validate_output_name(output_path),

            ChromaKey {
                output_path,
                output_target,
                ..
            } => {
                if matches!(
                    output_target,
                    operations::chroma_key::ChromaOutput::PngSequence
                ) {
                    validate_output_dir(output_path)
                } else {
                    validate_output_name(output_path)
                }
            }

            Split { output_dir, .. } | FrameExport { output_dir, .. } => {
                validate_output_dir(output_dir)
            }

            ExtractMulti { streams, .. } => {
                for spec in streams {
                    validate_output_name(&spec.output_path)?;
                }
                Ok(())
            }
        }
    }

    /// Return the primary input path for logging. `Merge` uses the first
    /// input, `ReplaceAudio` uses the video track — picking either is a
    /// judgment call, but a log entry with an empty path is strictly worse
    /// than one with a representative one.
    pub(crate) fn primary_input(&self) -> &str {
        use OperationPayload::*;
        match self {
            Rewrap { input_path, .. }
            | Extract { input_path, .. }
            | ExtractMulti { input_path, .. }
            | Cut { input_path, .. }
            | Split { input_path, .. }
            | AudioOffset { input_path, .. }
            | AudioNormalize { input_path, .. }
            | SilenceRemove { input_path, .. }
            | Conform { input_path, .. }
            | RemoveAudio { input_path, .. }
            | RemoveVideo { input_path, .. }
            | MetadataStrip { input_path, .. }
            | Loop { input_path, .. }
            | RotateFlip { input_path, .. }
            | Reverse { input_path, .. }
            | Speed { input_path, .. }
            | Fade { input_path, .. }
            | Deinterlace { input_path, .. }
            | Denoise { input_path, .. }
            | Thumbnail { input_path, .. }
            | ContactSheet { input_path, .. }
            | FrameExport { input_path, .. }
            | Watermark { input_path, .. }
            | Volume { input_path, .. }
            | ChannelTools { input_path, .. }
            | PadSilence { input_path, .. }
            | ChromaKey { input_path, .. } => input_path,
            ReplaceAudio { video_path, .. } => video_path,
            Merge { input_paths, .. } => input_paths.first().map(String::as_str).unwrap_or(""),
        }
    }
}

fn register_operation_cancellation_after_validation(
    state: &AppState,
    job_id: &str,
    operation: &OperationPayload,
) -> Result<Arc<AtomicBool>, String> {
    operation.validate_inputs()?;
    operation.validate_outputs()?;

    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut map = state.cancellations.lock();
        map.insert(job_id.to_string(), Arc::clone(&cancelled));
    }

    Ok(cancelled)
}

/// Run a mechanical video/audio operation.
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[command]
fn run_operation(
    window: Window,
    state: State<'_, AppState>,
    job_id: String,
    operation: OperationPayload,
) -> Result<(), String> {
    let cancelled = register_operation_cancellation_after_validation(&state, &job_id, &operation)?;

    let processes = Arc::clone(&state.processes);
    let cancellations = Arc::clone(&state.cancellations);

    std::thread::spawn(move || {
        let outcome: JobOutcome = match &operation {
            OperationPayload::Rewrap {
                input_path,
                output_path,
            } => op_result(
                operations::rewrap::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Extract {
                input_path,
                stream_index,
                stream_type,
                output_path,
            } => op_result(
                operations::extract::run(
                    &window,
                    &job_id,
                    input_path,
                    *stream_index,
                    stream_type,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::ExtractMulti {
                input_path,
                streams,
            } => {
                let primary = streams
                    .first()
                    .map(|s| s.output_path.clone())
                    .unwrap_or_default();
                op_result(
                    operations::extract::run_multi(
                        &window,
                        &job_id,
                        input_path,
                        streams,
                        Arc::clone(&processes),
                        Arc::clone(&cancelled),
                    ),
                    primary,
                )
            }

            OperationPayload::Cut {
                input_path,
                start_secs,
                end_secs,
                output_path,
            } => op_result(
                operations::cut::run(
                    &window,
                    &job_id,
                    input_path,
                    *start_secs,
                    *end_secs,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Split {
                input_path,
                timecodes_secs,
                output_dir,
            } => op_result(
                operations::split::run(
                    &window,
                    &job_id,
                    input_path,
                    timecodes_secs,
                    output_dir,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_dir.clone(),
            ),

            OperationPayload::AudioOffset {
                input_path,
                offset_ms,
                output_path,
            } => op_result(
                operations::audio_offset::run(
                    &window,
                    &job_id,
                    input_path,
                    *offset_ms,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::ReplaceAudio {
                video_path,
                audio_path,
                output_path,
            } => op_result(
                operations::replace_audio::run(
                    &window,
                    &job_id,
                    video_path,
                    audio_path,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Merge {
                input_paths,
                output_path,
            } => op_result(
                operations::merge::run(
                    &window,
                    &job_id,
                    input_paths,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::AudioNormalize {
                input_path,
                output_path,
                mode,
                target_i,
                target_tp,
                target_lra,
                linear,
            } => op_result(
                operations::analysis::audio_norm::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *mode,
                    *target_i,
                    *target_tp,
                    *target_lra,
                    *linear,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::SilenceRemove {
                input_path,
                output_path,
                threshold_db,
                min_silence_s,
                pad_ms,
            } => op_result(
                operations::silence_remove::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *threshold_db,
                    *min_silence_s,
                    *pad_ms,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::RemoveAudio {
                input_path,
                output_path,
            } => op_result(
                operations::remove_audio::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::RemoveVideo {
                input_path,
                output_path,
            } => op_result(
                operations::remove_video::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::MetadataStrip {
                input_path,
                output_path,
                mode,
                title_value,
            } => op_result(
                operations::metadata_strip::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    mode,
                    title_value.as_deref(),
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Loop {
                input_path,
                output_path,
                count,
            } => op_result(
                operations::loop_op::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *count,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::RotateFlip {
                input_path,
                output_path,
                mode,
            } => op_result(
                operations::video_filters::run_rotate_flip(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    mode,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Reverse {
                input_path,
                output_path,
            } => op_result(
                operations::video_filters::run_reverse(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Speed {
                input_path,
                output_path,
                rate,
            } => op_result(
                operations::video_filters::run_speed(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *rate,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Fade {
                input_path,
                output_path,
                fade_in,
                fade_out,
            } => op_result(
                operations::video_filters::run_fade(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *fade_in,
                    *fade_out,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Deinterlace {
                input_path,
                output_path,
                mode,
            } => op_result(
                operations::video_filters::run_deinterlace(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    mode,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Denoise {
                input_path,
                output_path,
                preset,
            } => op_result(
                operations::video_filters::run_denoise(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    preset,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Thumbnail {
                input_path,
                output_path,
                time_spec,
                format,
            } => op_result(
                operations::frame_ops::run_thumbnail(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    time_spec,
                    format,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::ContactSheet {
                input_path,
                output_path,
                cols,
                rows,
                frames,
            } => op_result(
                operations::frame_ops::run_contact_sheet(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *cols,
                    *rows,
                    *frames,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::FrameExport {
                input_path,
                output_dir,
                mode,
                value,
                format,
            } => op_result(
                operations::frame_ops::run_frame_export(
                    &window,
                    &job_id,
                    input_path,
                    output_dir,
                    mode,
                    *value,
                    format,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_dir.clone(),
            ),

            OperationPayload::Watermark {
                input_path,
                output_path,
                watermark_path,
                corner,
                opacity,
                scale_pct,
            } => op_result(
                operations::frame_ops::run_watermark(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    watermark_path,
                    corner,
                    *opacity,
                    *scale_pct,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Volume {
                input_path,
                output_path,
                gain_db,
            } => op_result(
                operations::audio_filters::run_volume(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *gain_db,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::ChannelTools {
                input_path,
                output_path,
                mode,
            } => op_result(
                operations::audio_filters::run_channel_tools(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    mode,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::PadSilence {
                input_path,
                output_path,
                head_s,
                tail_s,
            } => op_result(
                operations::audio_filters::run_pad_silence(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *head_s,
                    *tail_s,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::Conform {
                input_path,
                output_path,
                fps,
                resolution,
                pix_fmt,
                fps_algo,
                scale_algo,
                dither,
            } => op_result(
                operations::conform::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    fps.clone(),
                    resolution.clone(),
                    pix_fmt.clone(),
                    *fps_algo,
                    *scale_algo,
                    *dither,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),

            OperationPayload::ChromaKey {
                input_path,
                output_path,
                algo,
                color_hex,
                similarity,
                blend,
                despill,
                despill_mix,
                upsample,
                output_target,
                trim_start,
                trim_end,
            } => op_result(
                operations::chroma_key::run(
                    &window,
                    &job_id,
                    input_path,
                    output_path,
                    *algo,
                    color_hex,
                    *similarity,
                    *blend,
                    *despill,
                    *despill_mix,
                    *upsample,
                    *output_target,
                    *trim_start,
                    *trim_end,
                    Arc::clone(&processes),
                    Arc::clone(&cancelled),
                ),
                output_path.clone(),
            ),
        };

        {
            let mut map = cancellations.lock();
            map.remove(&job_id);
        }

        let input_path = operation.primary_input().to_string();
        finalize_job(&window, job_id, &input_path, outcome);
    });

    Ok(())
}

/// Return the list of streams in a media file (video, audio, subtitle, data).
#[command]
async fn get_streams(input_path: String) -> Result<Vec<operations::StreamInfo>, String> {
    validate_input_path(&input_path)?;
    tokio::task::spawn_blocking(move || -> Result<Vec<operations::StreamInfo>, String> {
        operations::extract::get_streams(&input_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Maximum accepted URL length for `open_url`. Generous for real-world links.
const OPEN_URL_MAX_LEN: usize = 4096;

/// Schemes accepted by `open_url`. Anything else (file://, javascript:,
/// vbscript:, custom URI handlers, …) is rejected before spawning a process.
const OPEN_URL_ALLOWED_SCHEMES: &[&str] = &["http", "https", "mailto"];

/// Validate a URL handed to `open_url`: enforce a length cap, reject control
/// characters and whitespace, and require an allowlisted scheme. Defense in
/// depth — even though `Command::arg` doesn't shell-interpret on Unix, the
/// Windows path goes through `cmd /C start` which re-parses the argument.
fn validate_url_scheme(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("URL is empty".to_string());
    }
    if url.len() > OPEN_URL_MAX_LEN {
        return Err(format!(
            "URL exceeds maximum length ({} > {} bytes)",
            url.len(),
            OPEN_URL_MAX_LEN
        ));
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("URL contains whitespace or control characters".to_string());
    }
    let scheme_end = url
        .find(':')
        .ok_or_else(|| "URL has no scheme".to_string())?;
    let scheme = &url[..scheme_end];
    if scheme.is_empty() {
        return Err("URL has empty scheme".to_string());
    }
    let scheme_lower = scheme.to_ascii_lowercase();
    if !OPEN_URL_ALLOWED_SCHEMES.contains(&scheme_lower.as_str()) {
        return Err(format!(
            "URL scheme '{}' not allowed (allowed: {})",
            scheme,
            OPEN_URL_ALLOWED_SCHEMES.join(", ")
        ));
    }
    Ok(())
}

/// Open a URL in the user's default browser.
/// Used for "download update" on platforms where in-place updates
/// are disabled (macOS/Windows without codesigning).
///
/// Only `http`, `https`, and `mailto` schemes are accepted; other schemes
/// (`file://`, `javascript:`, custom protocol handlers) are rejected
/// before any process is spawned.
#[command]
fn open_url(url: String) -> Result<(), String> {
    validate_url_scheme(&url)?;
    #[cfg(target_os = "macos")]
    let res = Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let res = Command::new("cmd").args(["/C", "start", "", &url]).spawn();
    #[cfg(target_os = "linux")]
    let res = Command::new("xdg-open").arg(&url).spawn();
    res.map(|_| ()).map_err(|e| e.to_string())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            processes: Arc::new(Mutex::new(HashMap::new())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        })
        .manage(FilmstripCancels::default())
        .invoke_handler(tauri::generate_handler![
            get_file_info,
            convert_file,
            cancel_job,
            check_tools,
            get_waveform,
            get_spectrogram,
            get_filmstrip,
            cancel_filmstrip,
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
            open_url,
            diag_append,
            diag_load,
            diag_clear,
            run_operation,
            get_streams,
            operations::analysis::loudness::analyze_loudness,
            operations::analysis::cut_detect::analyze_cut_detect,
            operations::analysis::black_detect::analyze_black_detect,
            operations::analysis::vmaf::analyze_vmaf,
            operations::analysis::framemd5::analyze_framemd5,
            operations::subtitling::probe::probe_subtitles,
            operations::subtitling::lint::lint_subtitle,
            operations::subtitling::diff::diff_subtitle,
            operations::chroma_key::chroma_key_preview,
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
        let result = build_output_path("/home/user/video.mp4", "mkv", None, "converted", "_");
        assert_eq!(result, "/home/user/video_converted.mkv");
    }

    #[test]
    fn build_output_path_empty_suffix() {
        let result = build_output_path("/home/user/video.mp4", "mkv", None, "", "_");
        assert_eq!(result, "/home/user/video.mkv");
    }

    #[test]
    fn build_output_path_custom_output_dir() {
        let result = build_output_path(
            "/home/user/video.mp4",
            "mp3",
            Some("/tmp/out"),
            "converted",
            "_",
        );
        assert_eq!(result, "/tmp/out/video_converted.mp3");
    }

    #[test]
    fn build_output_path_custom_separator() {
        let result = build_output_path("/home/user/video.mp4", "mkv", None, "proxy", "-");
        assert_eq!(result, "/home/user/video-proxy.mkv");
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
    fn image_args_basic_quality_no_strip_by_default() {
        // Default preserve_metadata=None → metadata kept (no -strip).
        let opts = ConvertOptions {
            quality: Some(85),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.webp", &opts);
        assert_eq!(args[0], "in.jpg");
        assert_eq!(args.last().unwrap(), "out.webp");
        assert!(args.contains(&"-quality".to_string()));
        assert!(args.contains(&"85".to_string()));
        assert!(!args.contains(&"-strip".to_string()));
    }

    #[test]
    fn image_args_strip_when_preserve_metadata_false() {
        let opts = ConvertOptions {
            quality: Some(85),
            preserve_metadata: Some(false),
            ..Default::default()
        };
        let args = build_image_magick_args("in.jpg", "out.webp", &opts);
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

    fn find_pair(args: &[String], flag: &str, value: &str) -> bool {
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
        // av1_speed=6 maps to preset=round(6*13/10)=round(7.8)=8
        assert!(find_pair(&args, "-preset", "8"));
        assert!(!args.contains(&"-cpu-used".to_string()));
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
    fn video_args_webm_video_bitrate_overrides_audio_bitrate() {
        // webm_video_bitrate should drive -b:v; `bitrate` should remain as -b:a.
        let opts = ConvertOptions {
            output_format: "webm".into(),
            codec: Some("vp9".into()),
            webm_bitrate_mode: Some("cbr".into()),
            webm_video_bitrate: Some(4000),
            bitrate: Some(192),
            ..Default::default()
        };
        let args = build_ffmpeg_video_args("in.mp4", "out.webm", &opts);
        assert!(find_pair(&args, "-b:v", "4000k"));
        assert!(find_pair(&args, "-minrate", "4000k"));
        assert!(find_pair(&args, "-maxrate", "4000k"));
        assert!(find_pair(&args, "-b:a", "192k"));
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
        assert!(vf_contains(&args, "subtitles='in.mkv'"));
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
        assert!(vf_contains(&args, "subtitles='in.mkv'"));
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
        assert!(
            !args.contains(&"-b:a".to_string()),
            "VBR must not emit -b:a"
        );
        let q_idx = args.iter().position(|a| a == "-q:a").expect("-q:a missing");
        assert_eq!(args[q_idx + 1], "3");
        assert!(args
            .windows(2)
            .any(|w| w[0] == "-c:a" && w[1] == "libmp3lame"));
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
    fn audio_args_opus_forces_48k_regardless_of_sample_rate() {
        // libopus rejects 44100/96000/192000 Hz; Rust must override to 48000.
        for sr in [44100u32, 96000, 192000, 48000] {
            let opts = ConvertOptions {
                output_format: "opus".to_string(),
                sample_rate: Some(sr),
                ..Default::default()
            };
            let args = build_ffmpeg_audio_args("in.wav", "out.opus", &opts);
            let ar_idx = args.iter().position(|a| a == "-ar").expect("-ar missing");
            assert_eq!(
                args[ar_idx + 1],
                "48000",
                "sample_rate {sr} should be forced to 48000"
            );
        }
    }

    #[test]
    fn audio_args_opus_inserts_48k_when_no_sample_rate() {
        let opts = ConvertOptions {
            output_format: "opus".to_string(),
            sample_rate: None,
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.opus", &opts);
        let ar_idx = args.iter().position(|a| a == "-ar").expect("-ar missing");
        assert_eq!(args[ar_idx + 1], "48000");
    }

    #[test]
    fn audio_args_aac_no_profile_flag() {
        // Native `aac` encoder only supports LC (default); we never emit -profile:a.
        let opts = ConvertOptions {
            output_format: "aac".to_string(),
            aac_profile: Some("hev2".to_string()), // ignored — UI no longer offers it
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("in.wav", "out.aac", &opts);
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        assert!(!args.contains(&"-profile:a".to_string()));
    }

    #[test]
    fn audio_args_wav_bit_depth_sample_fmt() {
        // 24-bit WAV: explicit pcm_s24le codec (s24 is not a valid sample_fmt)
        let opts24 = ConvertOptions {
            output_format: "wav".to_string(),
            bit_depth: Some(24),
            ..Default::default()
        };
        let args24 = build_ffmpeg_audio_args("in.wav", "out.wav", &opts24);
        assert!(args24
            .windows(2)
            .any(|w| w[0] == "-c:a" && w[1] == "pcm_s24le"));
        assert!(!args24.contains(&"-sample_fmt".to_string()));
        assert!(!args24.contains(&"-b:a".to_string()));

        // 32-bit float WAV: explicit pcm_f32le codec
        let opts32 = ConvertOptions {
            output_format: "wav".to_string(),
            bit_depth: Some(32),
            ..Default::default()
        };
        let args32 = build_ffmpeg_audio_args("in.wav", "out.wav", &opts32);
        assert!(args32
            .windows(2)
            .any(|w| w[0] == "-c:a" && w[1] == "pcm_f32le"));
        assert!(!args32.contains(&"-sample_fmt".to_string()));

        // 16-bit WAV: -sample_fmt s16 (no explicit codec needed)
        let opts16 = ConvertOptions {
            output_format: "wav".to_string(),
            bit_depth: Some(16),
            ..Default::default()
        };
        let args16 = build_ffmpeg_audio_args("in.wav", "out.wav", &opts16);
        assert!(args16
            .windows(2)
            .any(|w| w[0] == "-sample_fmt" && w[1] == "s16"));
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
    fn audio_args_ogg_cbr() {
        let opts = ConvertOptions {
            output_format: "ogg".to_string(),
            bitrate: Some(192),
            ogg_bitrate_mode: Some("cbr".to_string()),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.ogg", &opts);
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(args.windows(2).any(|w| w[0] == "-vbr" && w[1] == "off"));
        let br_idx = args.iter().position(|a| a == "-b:a").expect("-b:a missing");
        assert_eq!(args[br_idx + 1], "192k");
        assert!(!args.contains(&"-minrate".to_string()));
        assert!(!args.contains(&"-maxrate".to_string()));
    }

    #[test]
    fn audio_args_ogg_vbr_quality_to_bitrate() {
        let opts = ConvertOptions {
            output_format: "ogg".to_string(),
            ogg_bitrate_mode: Some("vbr".to_string()),
            ogg_vbr_quality: Some(5),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.ogg", &opts);
        assert!(args.windows(2).any(|w| w[0] == "-c:a" && w[1] == "libopus"));
        assert!(args.windows(2).any(|w| w[0] == "-vbr" && w[1] == "on"));
        let br_idx = args.iter().position(|a| a == "-b:a").expect("-b:a missing");
        assert_eq!(args[br_idx + 1], "128k"); // q=5 → 128 kbps
    }

    #[test]
    fn audio_args_ogg_forces_48k_like_opus() {
        // OGG uses libopus and must also reject 44100 via the sample-rate override.
        let opts = ConvertOptions {
            output_format: "ogg".to_string(),
            sample_rate: Some(44100),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.ogg", &opts);
        let ar_idx = args.iter().position(|a| a == "-ar").expect("-ar missing");
        assert_eq!(args[ar_idx + 1], "48000");
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
        // ALAC 24-bit content must be stored as s32p (alac rejects s24p).
        assert_eq!(args[sf_idx + 1], "s32p");
    }

    #[test]
    fn audio_args_alac_24bit_uses_s32p() {
        let opts = ConvertOptions {
            output_format: "alac".to_string(),
            bit_depth: Some(24),
            ..Default::default()
        };
        let args = build_ffmpeg_audio_args("i", "o.m4a", &opts);
        let sf_idx = args
            .iter()
            .position(|a| a == "-sample_fmt")
            .expect("-sample_fmt missing");
        assert_eq!(args[sf_idx + 1], "s32p");
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

    // ── write_fade_log ────────────────────────────────────────────────────────

    fn unique_log_path(tag: &str) -> std::path::PathBuf {
        let pid = std::process::id();
        let nonce = uuid::Uuid::new_v4();
        std::env::temp_dir().join(format!("fade-test-{tag}-{pid}-{nonce}.log"))
    }

    #[test]
    fn append_fade_log_appends_without_rmw_drops_under_concurrency() {
        let path = unique_log_path("concurrency");
        let _ = std::fs::remove_file(&path);

        let threads: Vec<_> = (0..20)
            .map(|t| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for i in 0..10 {
                        append_fade_log_at(&path, &format!("t{t}-i{i}"));
                    }
                })
            })
            .collect();
        for h in threads {
            h.join().expect("log writer thread panicked");
        }

        let contents = std::fs::read_to_string(&path).expect("log file written");
        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(
            lines.len(),
            200,
            "expected 200 lines from 20×10 concurrent writers, got {}",
            lines.len()
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_fade_log_rotates_when_over_threshold() {
        let path = unique_log_path("rotate");
        let rotated = path.with_extension("log.1");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&rotated);

        let filler = "x".repeat(1024);
        let target_lines = (FADE_LOG_MAX_BYTES / 1025) as usize + 4;
        for _ in 0..target_lines {
            append_fade_log_at(&path, &filler);
        }
        append_fade_log_at(&path, "post-rotation-marker");

        assert!(
            rotated.exists(),
            "rotated file should exist after threshold crossed"
        );
        let live = std::fs::read_to_string(&path).expect("live log readable");
        assert!(
            live.contains("post-rotation-marker"),
            "post-rotation write should land in fresh file"
        );
        assert!(
            live.len() < FADE_LOG_MAX_BYTES as usize,
            "fresh file should be well under threshold immediately after rotation"
        );

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&rotated);
    }

    // ── validate_output_name ─────────────────────────────────────────────────

    #[test]
    fn output_name_accepts_safe_stems() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(validate_output_name(&format!("{tmp}/video.mp4")).is_ok());
        assert!(validate_output_name(&format!("{tmp}/my-clip_converted.mkv")).is_ok());
        assert!(validate_output_name(&format!("{tmp}/my.archive.backup.mp4")).is_ok());
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            assert!(validate_output_name(&format!("{home}/Desktop/output.mp4")).is_ok());
        }
    }

    #[test]
    fn output_name_rejects_traversal() {
        assert!(validate_output_name("../../../etc/passwd").is_err());
        assert!(validate_output_name("/out/../etc/passwd").is_err());
        assert!(validate_output_name("foo/../../bad.mp4").is_err());
    }

    #[test]
    fn output_name_rejects_shell_metachars_in_stem() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(validate_output_name(&format!("{tmp}/bad;ls.mp4")).is_err());
        assert!(validate_output_name(&format!("{tmp}/$(whoami).mp4")).is_err());
        assert!(validate_output_name(&format!("{tmp}/file name.mp4")).is_err());
        assert!(validate_output_name(&format!("{tmp}/file|pipe.mp4")).is_err());
        assert!(validate_output_name(&format!("{tmp}/file&bg.mp4")).is_err());
    }

    #[test]
    fn output_name_rejects_empty_stem() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(validate_output_name(&format!("{tmp}/.mp4")).is_err());
    }

    #[test]
    fn output_name_rejects_parent_outside_allowed_roots() {
        assert!(validate_output_name("/etc/fade-output.mp4").is_err());
    }

    // ── validate_output_dir ──────────────────────────────────────────────────

    #[test]
    fn output_dir_rejects_traversal() {
        assert!(validate_output_dir("/home/user/../../../etc").is_err());
        assert!(validate_output_dir("../relative").is_err());
    }

    #[test]
    fn output_dir_accepts_temp_dir() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        assert!(validate_output_dir(&tmp).is_ok());
        assert!(validate_output_dir(&format!("{tmp}/fade-frames")).is_ok());
    }

    #[test]
    fn output_dir_accepts_home_subdir() {
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            assert!(validate_output_dir(&format!("{home}/Desktop")).is_ok());
        }
    }

    #[test]
    fn output_dir_rejects_system_dirs() {
        assert!(validate_output_dir("/etc").is_err());
        assert!(validate_output_dir("/usr/bin").is_err());
        assert!(validate_output_dir("/System/Library").is_err());
    }

    // ── validate_no_traversal ────────────────────────────────────────────────

    #[test]
    fn no_traversal_accepts_normal_paths() {
        assert!(validate_no_traversal("/Users/foo/Videos/clip.mp4").is_ok());
        assert!(validate_no_traversal("/tmp/subtitle.srt").is_ok());
    }

    #[test]
    fn input_path_rejects_parent_outside_allowed_roots() {
        assert!(validate_input_path("/etc/passwd").is_err());
    }

    #[test]
    fn input_path_accepts_temp_file_path() {
        let path = std::env::temp_dir().join("fade-input.mp4");
        assert!(validate_input_path(&path.to_string_lossy()).is_ok());
    }

    #[test]
    fn no_traversal_rejects_dotdot() {
        assert!(validate_no_traversal("../../etc/passwd").is_err());
        assert!(validate_no_traversal("/Users/foo/../../../etc").is_err());
    }

    // ── validate_url_scheme ───────────────────────────────────────────────────

    #[test]
    fn url_scheme_accepts_allowed_schemes() {
        assert!(validate_url_scheme("https://example.com").is_ok());
        assert!(validate_url_scheme("http://example.com/path?x=1&y=2").is_ok());
        assert!(validate_url_scheme("mailto:foo@bar.com").is_ok());
        // Scheme match is case-insensitive.
        assert!(validate_url_scheme("HTTPS://example.com").is_ok());
    }

    #[test]
    fn url_scheme_rejects_file_and_script_schemes() {
        assert!(validate_url_scheme("file:///etc/passwd").is_err());
        assert!(validate_url_scheme("javascript:alert(1)").is_err());
        assert!(validate_url_scheme("vbscript:msgbox(1)").is_err());
        assert!(validate_url_scheme("ftp://example.com").is_err());
        assert!(validate_url_scheme("data:text/html,<script>").is_err());
        // Custom URI handlers (e.g. slack://, steam://) also rejected.
        assert!(validate_url_scheme("slack://open").is_err());
    }

    #[test]
    fn url_scheme_rejects_malformed() {
        assert!(validate_url_scheme("").is_err());
        assert!(validate_url_scheme("no-scheme-here").is_err());
        assert!(validate_url_scheme(":empty-scheme").is_err());
    }

    #[test]
    fn url_scheme_rejects_whitespace_and_control_chars() {
        assert!(validate_url_scheme("https://example.com/ evil").is_err());
        assert!(validate_url_scheme("https://example.com\nfoo").is_err());
        assert!(validate_url_scheme("https://example.com\tfoo").is_err());
        assert!(validate_url_scheme("https://example.com\0foo").is_err());
    }

    #[test]
    fn url_scheme_rejects_oversized() {
        let long = format!("https://example.com/{}", "a".repeat(OPEN_URL_MAX_LEN));
        assert!(validate_url_scheme(&long).is_err());
    }

    // ── op_result — terminal-emission invariant ──────────────────────────────
    //
    // Every `run_operation` dispatch arm calls `op_result(result, output_path)`.
    // `op_result` is total and always returns exactly one of Done/Cancelled/Error,
    // so the terminal-emission invariant (exactly one of job-done/job-error/
    // job-cancelled per dispatched job_id) is compile-enforced for all 29
    // OperationPayload variants.  The exhaustive `match &operation` in
    // `run_operation` guarantees no variant escapes the dispatch without routing
    // through `op_result` → `finalize_job`.

    #[test]
    fn op_result_ok_produces_done_with_output_path() {
        let outcome = op_result(Ok(()), "/out/x.mp4".to_string());
        match outcome {
            JobOutcome::Done { output_path } => assert_eq!(output_path, "/out/x.mp4"),
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn op_result_cancelled_sentinel_produces_cancelled_no_remove() {
        // The single source of Err("CANCELLED") is run_ffmpeg in operations/mod.rs.
        let outcome = op_result(Err("CANCELLED".to_string()), "/out/x.mp4".to_string());
        match outcome {
            JobOutcome::Cancelled { remove_path } => assert!(remove_path.is_none()),
            other => panic!("expected Cancelled, got {other:?}"),
        }
    }

    #[test]
    fn op_result_arbitrary_error_produces_error_variant() {
        let outcome = op_result(Err("ffmpeg exited 1".to_string()), "/out/x.mp4".to_string());
        match outcome {
            JobOutcome::Error { message } => assert_eq!(message, "ffmpeg exited 1"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn op_result_cancelled_is_exact_match_not_substring() {
        // "CANCELLED" embedded in a longer error message must not become Cancelled.
        let outcome = op_result(
            Err("ffmpeg: CANCELLED by signal".to_string()),
            "/out/x.mp4".to_string(),
        );
        assert!(matches!(outcome, JobOutcome::Error { .. }));
    }

    // ── cancellation/process lifecycle ───────────────────────────────────────

    fn test_app_state() -> AppState {
        AppState {
            processes: Arc::new(Mutex::new(HashMap::new())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn spawn_long_running_child() -> Child {
        #[cfg(unix)]
        let mut cmd = Command::new("sleep");
        #[cfg(unix)]
        cmd.arg("30");
        #[cfg(windows)]
        let mut cmd = Command::new("cmd");
        #[cfg(windows)]
        cmd.args(["/C", "ping", "-n", "60", "127.0.0.1"]);

        cmd.spawn()
            .expect("spawn a long-running child for cancellation test")
    }

    #[test]
    fn cancel_job_kills_child_but_leaves_handle_for_worker_reap() {
        let state = test_app_state();
        let cancelled = Arc::new(AtomicBool::new(false));
        state
            .cancellations
            .lock()
            .insert("job-cancel".to_string(), Arc::clone(&cancelled));
        state
            .processes
            .lock()
            .insert("job-cancel".to_string(), spawn_long_running_child());

        cancel_job_impl(&state, "job-cancel").expect("cancel job");

        assert!(cancelled.load(Ordering::SeqCst));
        assert!(
            state.processes.lock().contains_key("job-cancel"),
            "cancel must leave process removal and wait to the worker"
        );

        let mut child = state.processes.lock().remove("job-cancel").unwrap();
        let status = child.wait().expect("worker-style wait after cancel kill");
        assert!(!status.success(), "cancelled child must not report success");
    }

    #[test]
    fn operation_validation_error_does_not_register_cancellation() {
        let state = test_app_state();
        let operation = OperationPayload::Rewrap {
            input_path: "/tmp/input.mkv".to_string(),
            output_path: "/tmp/bad;name.mp4".to_string(),
        };

        assert!(register_operation_cancellation_after_validation(
            &state,
            "job-invalid",
            &operation
        )
        .is_err());
        assert!(
            state.cancellations.lock().is_empty(),
            "failed validation must not leave a stale cancellation flag"
        );
    }

    // ── OperationPayload::primary_input ───────────────────────────────────────

    #[test]
    fn operation_primary_input_returns_input_path_for_standard_variants() {
        let op = OperationPayload::Rewrap {
            input_path: "/in/a.mkv".to_string(),
            output_path: "/out/a.mp4".to_string(),
        };
        assert_eq!(op.primary_input(), "/in/a.mkv");
    }

    #[test]
    fn operation_primary_input_returns_video_path_for_replace_audio() {
        let op = OperationPayload::ReplaceAudio {
            video_path: "/in/video.mp4".to_string(),
            audio_path: "/in/track.wav".to_string(),
            output_path: "/out/combined.mp4".to_string(),
        };
        assert_eq!(op.primary_input(), "/in/video.mp4");
    }

    #[test]
    fn operation_primary_input_returns_first_input_for_merge() {
        let op = OperationPayload::Merge {
            input_paths: vec!["/in/a.mp4".to_string(), "/in/b.mp4".to_string()],
            output_path: "/out/joined.mp4".to_string(),
        };
        assert_eq!(op.primary_input(), "/in/a.mp4");
    }

    #[test]
    fn operation_primary_input_empty_merge_returns_empty_str() {
        let op = OperationPayload::Merge {
            input_paths: vec![],
            output_path: "/out/joined.mp4".to_string(),
        };
        assert_eq!(op.primary_input(), "");
    }

    // ── convert_file input/output validation ─────────────────────────────────
    // `convert_file` itself takes a Window + State and can't be invoked from a
    // unit test without a Tauri runtime. The `validate_convert_inputs` helper
    // is the same validator chain that `convert_file` runs inline before any
    // IO sink (create_dir_all, ffmpeg arg interpolation, etc.) — exercising
    // it here pins the CLAUDE.md "validate before any CLI arg interpolation"
    // contract for the conversion path.

    #[test]
    fn convert_file_rejects_traversal_in_input_path() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let out = format!("{tmp}/out.mp4");
        assert!(validate_convert_inputs("../../../etc/passwd", Some(&tmp), &out).is_err());
        assert!(validate_convert_inputs("/Users/foo/../../etc/passwd", Some(&tmp), &out).is_err());
    }

    #[test]
    fn convert_file_rejects_output_dir_outside_allowed_roots() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let in_path = format!("{tmp}/input.mp4");
        let out = "/etc/output.mp4".to_string();
        assert!(validate_convert_inputs(&in_path, Some("/etc"), &out).is_err());
        assert!(validate_convert_inputs(&in_path, Some("/usr/bin"), &out).is_err());
        assert!(validate_convert_inputs(&in_path, Some("/System/Library"), &out).is_err());
    }

    #[test]
    fn convert_file_rejects_output_dir_with_parent_component() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let in_path = format!("{tmp}/input.mp4");
        let out = format!("{tmp}/out.mp4");
        assert!(validate_convert_inputs(&in_path, Some("/home/user/../../../etc"), &out).is_err());
        assert!(validate_convert_inputs(&in_path, Some("../relative"), &out).is_err());
    }

    #[test]
    fn convert_file_accepts_safe_paths() {
        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        let in_path = format!("{tmp}/input.mp4");
        let out = format!("{tmp}/output_converted.mp4");
        assert!(validate_convert_inputs(&in_path, Some(&tmp), &out).is_ok());
        // None output_dir is also fine — convert_file allows the source
        // directory as the implicit destination.
        assert!(validate_convert_inputs(&in_path, None, &out).is_ok());
        // Sequence-style directory base names (no extension) are accepted.
        let seq = format!("{tmp}/clip_converted_png_frames");
        assert!(validate_convert_inputs(&in_path, Some(&tmp), &seq).is_ok());
    }
}
