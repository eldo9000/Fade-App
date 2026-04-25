//! Refactored sweep against the new `convert::image::convert`,
//! `convert::audio::convert`, and `convert::video::convert` entry points.
//! Verifies the wrapper-decoupling refactor (TASK-6) — each pure `convert()`
//! is callable without `&Window`, with `noop_progress()` and an empty
//! processes map. The video case is `#[ignore]` because of encode time.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_av_sweep -- --nocapture
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_av_sweep -- --include-ignored --nocapture

use fade_lib::convert::{audio, image, noop_progress, video};
use fade_lib::{ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn output_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root.join("test-results").join("refactored-av-sweep");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("{bin} spawn failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{bin} {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(())
}

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

#[derive(Debug)]
enum Row {
    Pass(String),
    Skip(String, String),
    Fail(String, String),
}

fn report(name: &str, rows: &[Row]) {
    let mut pass = 0;
    let mut skip = 0;
    let mut fail = 0;
    for row in rows {
        match row {
            Row::Pass(label) => {
                pass += 1;
                println!("  [PASS] {label}");
            }
            Row::Skip(label, why) => {
                skip += 1;
                println!("  [SKIP] {label} — {why}");
            }
            Row::Fail(label, msg) => {
                fail += 1;
                println!("  [FAIL] {label} — {msg}");
            }
        }
    }
    println!(
        "{name}: {pass} pass, {skip} skip, {fail} fail (of {})",
        rows.len()
    );
    if fail > 0 {
        panic!("{fail} conversion(s) failed in {name}");
    }
}

#[test]
fn refactored_image_sweep() {
    let dir = output_root();
    let mut rows: Vec<Row> = Vec::new();
    let label = "png_to_webp".to_string();

    if !tool_available("magick") {
        rows.push(Row::Skip(label, "magick not in PATH".to_string()));
        report("refactored_image_sweep", &rows);
        return;
    }

    let fixture = dir.join("_fixture.png");
    if let Err(e) = make_png(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    let out = dir.join("png_to_webp.webp");
    let opts = ConvertOptions {
        output_format: "webp".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));

    let result = image::convert(
        fixture.to_str().unwrap(),
        out.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-image-1",
        processes,
        &cancelled,
    );

    rows.push(match result {
        ConvertResult::Done => {
            if !out.exists() {
                Row::Fail(label, format!("output missing at {}", out.display()))
            } else if out.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
                Row::Fail(label, "output is empty".to_string())
            } else {
                Row::Pass(label)
            }
        }
        ConvertResult::Error(msg) => Row::Fail(label, msg),
        other => Row::Fail(label, format!("unexpected result: {other:?}")),
    });

    report("refactored_image_sweep", &rows);
}

#[test]
fn refactored_audio_sweep() {
    let dir = output_root();
    let mut rows: Vec<Row> = Vec::new();
    let label = "wav_to_mp3".to_string();

    if !tool_available("ffmpeg") {
        rows.push(Row::Skip(label, "ffmpeg not in PATH".to_string()));
        report("refactored_audio_sweep", &rows);
        return;
    }

    let fixture = dir.join("_fixture.wav");
    if let Err(e) = make_wav(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    let out = dir.join("wav_to_mp3.mp3");
    let opts = ConvertOptions {
        output_format: "mp3".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));

    let result = audio::convert(
        fixture.to_str().unwrap(),
        out.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-audio-1",
        processes,
        &cancelled,
    );

    rows.push(match result {
        ConvertResult::Done => {
            if !out.exists() {
                Row::Fail(label, format!("output missing at {}", out.display()))
            } else if out.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
                Row::Fail(label, "output is empty".to_string())
            } else {
                Row::Pass(label)
            }
        }
        ConvertResult::Error(msg) => Row::Fail(label, msg),
        other => Row::Fail(label, format!("unexpected result: {other:?}")),
    });

    report("refactored_audio_sweep", &rows);
}

#[test]
#[ignore]
fn refactored_video_sweep() {
    let dir = output_root();
    let mut rows: Vec<Row> = Vec::new();
    let label = "mp4_to_webm".to_string();

    if !tool_available("ffmpeg") {
        rows.push(Row::Skip(label, "ffmpeg not in PATH".to_string()));
        report("refactored_video_sweep", &rows);
        return;
    }

    let fixture = dir.join("_fixture.mp4");
    if let Err(e) = make_mp4(&fixture) {
        panic!("fixture creation failed: {e}");
    }

    let out = dir.join("mp4_to_webm.webm");
    let opts = ConvertOptions {
        output_format: "webm".to_string(),
        codec: Some("vp9".into()),
        webm_bitrate_mode: Some("crf".into()),
        crf: Some(40),
        vp9_speed: Some(8),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));

    let result = video::convert(
        fixture.to_str().unwrap(),
        out.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-video-1",
        processes,
        &cancelled,
    );

    rows.push(match result {
        ConvertResult::Done => {
            if !out.exists() {
                Row::Fail(label, format!("output missing at {}", out.display()))
            } else if out.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
                Row::Fail(label, "output is empty".to_string())
            } else {
                Row::Pass(label)
            }
        }
        ConvertResult::Error(msg) => Row::Fail(label, msg),
        other => Row::Fail(label, format!("unexpected result: {other:?}")),
    });

    report("refactored_video_sweep", &rows);
}
