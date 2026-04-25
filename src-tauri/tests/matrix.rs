//! Conversion matrix — sweeps every live output format (and a few key
//! settings variants) through Fade's real arg builders, writing outputs
//! to a stable folder so they can be visually inspected after the run.
//!
//! Philosophy: a successful conversion produces a non-empty output file.
//! Anything missing from the output folder is a failure to investigate.
//!
//! Each category gets its own test so `cargo test --test matrix` runs
//! the fast suites by default; the slow video sweep is `#[ignore]`.
//!
//! Run everything (including video):
//!
//!   cargo test --manifest-path src-tauri/Cargo.toml --test matrix \
//!     -- --include-ignored --nocapture
//!
//! Run just one category:
//!
//!   cargo test --manifest-path src-tauri/Cargo.toml --test matrix image_matrix
//!
//! Outputs land in:
//!
//!   test-results/conversion-matrix/<category>/
//!
//! The folder is wiped at the start of each test so stale files from
//! previous runs cannot mask a new failure.

use fade_lib::{
    build_ffmpeg_audio_args, build_ffmpeg_video_args, build_image_magick_args,
    convert::data::{parse_input, write_output},
    ConvertOptions,
};
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Repo-relative output root. Resolved against `CARGO_MANIFEST_DIR/..`
/// so the path is the same regardless of where `cargo test` is invoked from.
fn output_root(category: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("conversion-matrix")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

/// Run a command. Returns Err with stderr on failure rather than panicking,
/// so the matrix can collect every failure instead of stopping at the first.
fn run_cmd(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("spawn `{program}` failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{program}` exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("(no stderr)")
        ));
    }
    Ok(())
}

/// Result of a single conversion attempt.
struct CaseResult {
    name: String,
    output: PathBuf,
    error: Option<String>,
}

impl CaseResult {
    fn ok(name: String, output: PathBuf) -> Self {
        Self {
            name,
            output,
            error: None,
        }
    }
    fn err(name: String, output: PathBuf, error: String) -> Self {
        Self {
            name,
            output,
            error: Some(error),
        }
    }
    fn passed(&self) -> bool {
        self.error.is_none()
            && self.output.exists()
            && self.output.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }
}

/// Print a results table and panic if anything failed.
fn report(category: &str, cases: Vec<CaseResult>) {
    let total = cases.len();
    let failed: Vec<&CaseResult> = cases.iter().filter(|c| !c.passed()).collect();

    println!();
    println!(
        "── {} matrix ─ {} cases, {} failed",
        category,
        total,
        failed.len()
    );
    for c in &cases {
        let status = if c.passed() { "PASS" } else { "FAIL" };
        let size = c.output.metadata().map(|m| m.len()).unwrap_or(0);
        let detail = c.error.as_deref().unwrap_or("");
        println!(
            "  [{}] {:<48} {:>10} bytes  {}",
            status, c.name, size, detail
        );
    }

    if !failed.is_empty() {
        panic!(
            "{}: {} of {} conversions failed (see table above; output dir retained for inspection)",
            category,
            failed.len(),
            total,
        );
    }
}

/// Generate a small PNG fixture.
fn make_png(path: &Path) -> Result<(), String> {
    run_cmd(
        "magick",
        &[
            "-size",
            "32x32",
            "xc:blue",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

/// Generate a 1-second 440 Hz sine WAV fixture.
fn make_wav(path: &Path) -> Result<(), String> {
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

/// Generate a 1-second H.264 MP4 fixture (64x64 blue, 30 fps).
fn make_mp4(path: &Path) -> Result<(), String> {
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:size=64x64:rate=30",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            path.to_str().ok_or("invalid path")?,
        ],
    )
}

// ── Image matrix ─────────────────────────────────────────────────────────────

/// Image: PNG fixture → every live image format, plus a couple of settings
/// variants (JPEG quality, WebP lossless).
#[test]
#[ignore]
fn image_matrix() {
    let dir = output_root("image");
    let fixture = dir.join("_fixture.png");
    if let Err(e) = make_png(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    struct Spec {
        name: &'static str,
        ext: &'static str,
        opts: fn() -> ConvertOptions,
    }

    let specs: &[Spec] = &[
        Spec {
            name: "png_to_jpeg_default",
            ext: "jpg",
            opts: || ConvertOptions {
                output_format: "jpeg".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_jpeg_q25",
            ext: "jpg",
            opts: || ConvertOptions {
                output_format: "jpeg".into(),
                quality: Some(25),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_jpeg_q95",
            ext: "jpg",
            opts: || ConvertOptions {
                output_format: "jpeg".into(),
                quality: Some(95),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_png_default",
            ext: "png",
            opts: || ConvertOptions {
                output_format: "png".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_webp_default",
            ext: "webp",
            opts: || ConvertOptions {
                output_format: "webp".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_webp_lossless",
            ext: "webp",
            opts: || ConvertOptions {
                output_format: "webp".into(),
                webp_lossless: Some(true),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_tiff_default",
            ext: "tiff",
            opts: || ConvertOptions {
                output_format: "tiff".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_bmp_default",
            ext: "bmp",
            opts: || ConvertOptions {
                output_format: "bmp".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "png_to_avif_default",
            ext: "avif",
            opts: || ConvertOptions {
                output_format: "avif".into(),
                ..Default::default()
            },
        },
    ];

    let mut cases = Vec::new();
    for s in specs {
        let out = dir.join(format!("{}.{}", s.name, s.ext));
        let opts = (s.opts)();
        let args = build_image_magick_args(
            fixture.to_str().expect("fixture path"),
            out.to_str().expect("out path"),
            &opts,
        );
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cases.push(match run_cmd("magick", &arg_refs) {
            Ok(()) => CaseResult::ok(s.name.into(), out),
            Err(e) => CaseResult::err(s.name.into(), out, e),
        });
    }

    report("image", cases);
}

// ── Audio matrix ─────────────────────────────────────────────────────────────

/// Audio: WAV fixture → every live audio format, plus a couple of common
/// codec/mode variants (MP3 CBR vs VBR, M4A AAC vs ALAC, OGG VBR).
#[test]
#[ignore]
fn audio_matrix() {
    let dir = output_root("audio");
    let fixture = dir.join("_fixture.wav");
    if let Err(e) = make_wav(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    struct Spec {
        name: &'static str,
        ext: &'static str,
        opts: fn() -> ConvertOptions,
    }

    let specs: &[Spec] = &[
        Spec {
            name: "wav_to_mp3_default",
            ext: "mp3",
            opts: || ConvertOptions {
                output_format: "mp3".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_mp3_cbr_192",
            ext: "mp3",
            opts: || ConvertOptions {
                output_format: "mp3".into(),
                mp3_bitrate_mode: Some("cbr".into()),
                bitrate: Some(192),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_mp3_vbr_q2",
            ext: "mp3",
            opts: || ConvertOptions {
                output_format: "mp3".into(),
                mp3_bitrate_mode: Some("vbr".into()),
                mp3_vbr_quality: Some(2),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_wav_default",
            ext: "wav",
            opts: || ConvertOptions {
                output_format: "wav".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_flac_default",
            ext: "flac",
            opts: || ConvertOptions {
                output_format: "flac".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_flac_max_compression",
            ext: "flac",
            opts: || ConvertOptions {
                output_format: "flac".into(),
                flac_compression: Some(8),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_ogg_vbr_q5",
            ext: "ogg",
            opts: || ConvertOptions {
                output_format: "ogg".into(),
                ogg_bitrate_mode: Some("vbr".into()),
                ogg_vbr_quality: Some(5),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_aac_192",
            ext: "aac",
            opts: || ConvertOptions {
                output_format: "aac".into(),
                bitrate: Some(192),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_opus_vbr_128",
            ext: "opus",
            opts: || ConvertOptions {
                output_format: "opus".into(),
                opus_application: Some("audio".into()),
                opus_vbr: Some(true),
                bitrate: Some(128),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_m4a_aac",
            ext: "m4a",
            opts: || ConvertOptions {
                output_format: "m4a".into(),
                m4a_subcodec: Some("aac".into()),
                bitrate: Some(192),
                ..Default::default()
            },
        },
        Spec {
            name: "wav_to_m4a_alac_24bit",
            ext: "m4a",
            opts: || ConvertOptions {
                output_format: "m4a".into(),
                m4a_subcodec: Some("alac".into()),
                bit_depth: Some(24),
                ..Default::default()
            },
        },
    ];

    let mut cases = Vec::new();
    for s in specs {
        let out = dir.join(format!("{}.{}", s.name, s.ext));
        let opts = (s.opts)();
        let args = build_ffmpeg_audio_args(
            fixture.to_str().expect("fixture path"),
            out.to_str().expect("out path"),
            &opts,
        );
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cases.push(match run_cmd("ffmpeg", &arg_refs) {
            Ok(()) => CaseResult::ok(s.name.into(), out),
            Err(e) => CaseResult::err(s.name.into(), out, e),
        });
    }

    report("audio", cases);
}

// ── Data matrix ──────────────────────────────────────────────────────────────

/// Data: CSV fixture → every supported text format. Pure-Rust path,
/// no external CLI needed.
#[test]
#[ignore]
fn data_matrix() {
    let dir = output_root("data");
    let fixture = dir.join("_fixture.csv");
    std::fs::write(&fixture, "name,value\nalpha,1\nbeta,2\n").expect("write fixture");

    let raw = std::fs::read_to_string(&fixture).expect("read fixture");
    let value = parse_input("csv", &raw).expect("parse csv");

    let targets: &[(&str, &str)] = &[
        ("csv_to_json", "json"),
        ("csv_to_yaml", "yaml"),
        ("csv_to_xml", "xml"),
        ("csv_to_tsv", "tsv"),
        ("csv_to_csv", "csv"),
    ];

    let mut cases = Vec::new();
    for (name, ext) in targets {
        let out = dir.join(format!("{name}.{ext}"));
        let result = write_output(ext, &value, true, b',')
            .and_then(|s| std::fs::write(&out, s).map_err(|e| e.to_string()));
        cases.push(match result {
            Ok(()) => CaseResult::ok((*name).into(), out),
            Err(e) => CaseResult::err((*name).into(), out, e),
        });
    }

    report("data", cases);
}

// ── Video matrix ─────────────────────────────────────────────────────────────

/// Video: MP4 fixture → live containers and core codecs. Slow because every
/// case is a real ffmpeg encode; gated behind `#[ignore]`.
#[test]
#[ignore]
fn video_matrix() {
    let dir = output_root("video");
    let fixture = dir.join("_fixture.mp4");
    if let Err(e) = make_mp4(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    struct Spec {
        name: &'static str,
        ext: &'static str,
        opts: fn() -> ConvertOptions,
    }

    let specs: &[Spec] = &[
        Spec {
            name: "mp4_to_mp4_h264",
            ext: "mp4",
            opts: || ConvertOptions {
                output_format: "mp4".into(),
                codec: Some("h264".into()),
                crf: Some(28),
                preset: Some("ultrafast".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_mp4_h265",
            ext: "mp4",
            opts: || ConvertOptions {
                output_format: "mp4".into(),
                codec: Some("h265".into()),
                crf: Some(30),
                preset: Some("ultrafast".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_webm_vp9",
            ext: "webm",
            opts: || ConvertOptions {
                output_format: "webm".into(),
                codec: Some("vp9".into()),
                webm_bitrate_mode: Some("crf".into()),
                crf: Some(40),
                vp9_speed: Some(8),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_mkv_ffv1",
            ext: "mkv",
            opts: || ConvertOptions {
                output_format: "mkv".into(),
                codec: Some("ffv1".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_mov_prores",
            ext: "mov",
            opts: || ConvertOptions {
                output_format: "mov".into(),
                codec: Some("prores".into()),
                prores_profile: Some(0),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_mov_mjpeg",
            ext: "mov",
            opts: || ConvertOptions {
                output_format: "mov".into(),
                codec: Some("mjpeg".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_avi_xvid",
            ext: "avi",
            opts: || ConvertOptions {
                output_format: "avi".into(),
                codec: Some("mpeg4".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_to_gif_default",
            ext: "gif",
            opts: || ConvertOptions {
                output_format: "gif".into(),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_h264_yuv422p_high",
            ext: "mp4",
            opts: || ConvertOptions {
                output_format: "mp4".into(),
                codec: Some("h264".into()),
                crf: Some(28),
                preset: Some("ultrafast".into()),
                pix_fmt: Some("yuv422p".into()),
                h264_profile: Some("high".into()),
                ..Default::default()
            },
        },
        Spec {
            name: "mp4_h264_yuv444p_high",
            ext: "mp4",
            opts: || ConvertOptions {
                output_format: "mp4".into(),
                codec: Some("h264".into()),
                crf: Some(28),
                preset: Some("ultrafast".into()),
                pix_fmt: Some("yuv444p".into()),
                h264_profile: Some("high".into()),
                ..Default::default()
            },
        },
    ];

    let mut cases = Vec::new();
    for s in specs {
        let out = dir.join(format!("{}.{}", s.name, s.ext));
        let opts = (s.opts)();
        let args = build_ffmpeg_video_args(
            fixture.to_str().expect("fixture path"),
            out.to_str().expect("out path"),
            &opts,
        );
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cases.push(match run_cmd("ffmpeg", &arg_refs) {
            Ok(()) => CaseResult::ok(s.name.into(), out),
            Err(e) => CaseResult::err(s.name.into(), out, e),
        });
    }

    report("video", cases);
}
