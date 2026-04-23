//! Smoke tests: one successful file conversion per media category.
//!
//! Each test:
//!   1. Generates a small fixture file via `ffmpeg` or `imagemagick`.
//!   2. Runs the conversion using Fade's public arg builders (image/video/audio)
//!      or pure-Rust conversion helpers (data) — no Tauri runtime required.
//!   3. Asserts the output file exists and is non-empty.
//!
//! Run with:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test conversions

use fade_lib::{
    build_ffmpeg_audio_args, build_ffmpeg_video_args, build_image_magick_args,
    convert::data::{parse_input, write_output},
    ConvertOptions,
};
use std::process::Command;

/// Run a command, panicking with stdout+stderr on failure.
fn run_cmd(program: &str, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `{program}`: {e}"));

    if !output.status.success() {
        panic!(
            "`{program} {}` exited with {}\nstdout: {}\nstderr: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

// ── Image ─────────────────────────────────────────────────────────────────────

/// Generate a 10x10 PNG with ImageMagick, convert to WebP via Fade's
/// `build_image_magick_args`, assert the output exists and is non-empty.
#[test]
fn image_png_to_webp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("test.png");
    let output = dir.path().join("test.webp");

    // Generate fixture
    run_cmd(
        "magick",
        &[
            "-size",
            "10x10",
            "xc:blue",
            input.to_str().expect("valid path"),
        ],
    );

    assert!(input.exists(), "fixture not created: {:?}", input);

    // Build args using Fade's public helper
    let opts = ConvertOptions {
        output_format: "webp".to_string(),
        ..ConvertOptions::default()
    };
    let args = build_image_magick_args(
        input.to_str().expect("valid path"),
        output.to_str().expect("valid path"),
        &opts,
    );

    // Execute conversion
    run_cmd(
        "magick",
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );

    assert!(output.exists(), "output not found: {:?}", output);
    assert!(
        output.metadata().expect("metadata").len() > 0,
        "output is empty: {:?}",
        output
    );
}

// ── Video ─────────────────────────────────────────────────────────────────────

/// Generate a 1-second H.264 MP4 with ffmpeg, convert to WebM using VP9
/// via Fade's `build_ffmpeg_video_args`, assert the output exists and is non-empty.
#[test]
#[ignore] // slow (5-15 s); included in `cargo test --test conversions --include-ignored`
fn video_mp4_to_webm() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("test.mp4");
    let output = dir.path().join("test.webm");

    // Generate fixture (1-second blue square, H.264)
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
            input.to_str().expect("valid path"),
        ],
    );

    assert!(input.exists(), "fixture not created: {:?}", input);

    // Build args using Fade's public helper (VP9 codec, CRF mode)
    let opts = ConvertOptions {
        output_format: "webm".to_string(),
        codec: Some("vp9".to_string()),
        webm_bitrate_mode: Some("crf".to_string()),
        crf: Some(40),
        ..ConvertOptions::default()
    };
    let args = build_ffmpeg_video_args(
        input.to_str().expect("valid path"),
        output.to_str().expect("valid path"),
        &opts,
    );

    // Execute conversion
    run_cmd(
        "ffmpeg",
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );

    assert!(output.exists(), "output not found: {:?}", output);
    assert!(
        output.metadata().expect("metadata").len() > 0,
        "output is empty: {:?}",
        output
    );
}

// ── Audio ─────────────────────────────────────────────────────────────────────

/// Generate a 1-second 440 Hz sine WAV with ffmpeg, convert to MP3 using
/// Fade's `build_ffmpeg_audio_args`, assert the output exists and is non-empty.
#[test]
fn audio_wav_to_mp3() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("test.wav");
    let output = dir.path().join("test.mp3");

    // Generate fixture
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            input.to_str().expect("valid path"),
        ],
    );

    assert!(input.exists(), "fixture not created: {:?}", input);

    // Build args using Fade's public helper
    let opts = ConvertOptions {
        output_format: "mp3".to_string(),
        ..ConvertOptions::default()
    };
    let args = build_ffmpeg_audio_args(
        input.to_str().expect("valid path"),
        output.to_str().expect("valid path"),
        &opts,
    );

    // Execute conversion
    run_cmd(
        "ffmpeg",
        &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );

    assert!(output.exists(), "output not found: {:?}", output);
    assert!(
        output.metadata().expect("metadata").len() > 0,
        "output is empty: {:?}",
        output
    );
}

// ── Data ──────────────────────────────────────────────────────────────────────

/// Write a small CSV to a temp file, convert to JSON using Fade's pure-Rust
/// `parse_input` + `write_output` helpers, assert the result is valid JSON.
#[test]
fn data_csv_to_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    let input = dir.path().join("test.csv");
    let output = dir.path().join("test.json");

    let csv_content = "name,value\nalpha,1\nbeta,2\n";
    std::fs::write(&input, csv_content).expect("write fixture");

    // Use Fade's pure-Rust conversion helpers (no Tauri runtime needed)
    let raw = std::fs::read_to_string(&input).expect("read fixture");
    let value = parse_input("csv", &raw).expect("parse_input failed");
    let json_str = write_output("json", &value, true, b',').expect("write_output failed");

    std::fs::write(&output, &json_str).expect("write output");

    assert!(output.exists(), "output not found: {:?}", output);
    assert!(
        output.metadata().expect("metadata").len() > 0,
        "output is empty: {:?}",
        output
    );

    // Verify the output is valid JSON and contains expected data
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("output is not valid JSON");
    let rows = parsed.as_array().expect("expected JSON array");
    assert_eq!(rows.len(), 2, "expected 2 rows, got {}", rows.len());
    assert_eq!(rows[0]["name"], "alpha");
    assert_eq!(rows[0]["value"], "1");
    assert_eq!(rows[1]["name"], "beta");
    assert_eq!(rows[1]["value"], "2");
}
