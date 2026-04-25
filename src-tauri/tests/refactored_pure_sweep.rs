//! Sweep test for the refactored pure `convert()` entry points.
//!
//! Each module exposes a `convert(...)` that takes a [`ProgressFn`] callback
//! instead of a Tauri `&Window`. This test calls those entry points
//! directly with `noop_progress()` and verifies they produce non-empty
//! output. Counterpart to `extra_sweep.rs`, which exercises only the
//! lowest-level helper functions.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_pure_sweep -- --nocapture

use fade_lib::convert::{document, email, noop_progress, subtitle};
use fade_lib::ConvertOptions;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;

// ── Shared ───────────────────────────────────────────────────────────────────

fn output_root(category: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("refactored-pure-sweep")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

fn assert_nonempty(path: &Path) {
    assert!(path.exists(), "expected output file at {}", path.display());
    let len = path
        .metadata()
        .map(|m| m.len())
        .unwrap_or_else(|e| panic!("metadata for {}: {e}", path.display()));
    assert!(len > 0, "output {} is empty", path.display());
}

// Sample fixtures — copied from extra_sweep.rs (tests must not cross-import).

const SAMPLE_EML: &str = "\
From: alice@example.com
To: bob@example.com
Subject: Hello
Date: Mon, 1 Jan 2024 00:00:00 +0000

Hello, this is the body.
Line two of the body.
";

const SAMPLE_SRT: &str = "\
1
00:00:01,200 --> 00:00:03,400
Hello world

2
00:00:04,000 --> 00:00:06,500
Second cue
";

const SAMPLE_MD: &str = "\
# Heading

A paragraph with **bold** and *italic* and `code`.

- bullet one
- bullet two

[link](http://example.com)
";

const SAMPLE_HTML: &str = "\
<!doctype html>
<html><body>
<h1>Heading</h1>
<p>A paragraph with <strong>bold</strong> and <em>italic</em>.</p>
</body></html>
";

// ── Email ────────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn email_convert_eml_to_mbox() {
    let dir = output_root("email");
    let input = dir.join("in.eml");
    let output = dir.join("out.mbox");
    std::fs::write(&input, SAMPLE_EML).expect("write input");

    let opts = ConvertOptions {
        output_format: "mbox".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let cancelled = Arc::new(AtomicBool::new(false));
    email::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &cancelled,
    )
    .expect("eml→mbox conversion");
    assert_nonempty(&output);
}

#[test]
#[ignore]
fn email_convert_mbox_to_eml() {
    let dir = output_root("email-mbox-eml");
    let input = dir.join("in.mbox");
    let output = dir.join("out.eml");
    // Build a minimal mbox by piping the sample EML through eml_to_mbox.
    let mbox_text = email::eml_to_mbox(SAMPLE_EML);
    std::fs::write(&input, mbox_text).expect("write input");

    let opts = ConvertOptions {
        output_format: "eml".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let cancelled = Arc::new(AtomicBool::new(false));
    email::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &cancelled,
    )
    .expect("mbox→eml conversion");
    assert_nonempty(&output);
}

// ── Subtitle ─────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn subtitle_convert_srt_to_sbv() {
    let dir = output_root("subtitle");
    let input = dir.join("in.srt");
    let output = dir.join("out.sbv");
    std::fs::write(&input, SAMPLE_SRT).expect("write input");

    let opts = ConvertOptions {
        output_format: "sbv".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let mut runner = subtitle::UnavailableFfmpeg;
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    subtitle::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &mut runner,
        processes,
        cancelled,
        "test-srt-to-sbv",
    )
    .expect("srt→sbv conversion");
    assert_nonempty(&output);
}

#[test]
#[ignore]
fn subtitle_convert_sbv_to_srt() {
    let dir = output_root("subtitle-sbv-srt");
    let input = dir.join("in.sbv");
    let output = dir.join("out.srt");
    // Derive an sbv from the canonical srt sample.
    let sbv = subtitle::srt_to_sbv(SAMPLE_SRT);
    std::fs::write(&input, sbv).expect("write input");

    let opts = ConvertOptions {
        output_format: "srt".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let mut runner = subtitle::UnavailableFfmpeg;
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    subtitle::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &mut runner,
        processes,
        cancelled,
        "test-sbv-to-srt",
    )
    .expect("sbv→srt conversion");
    assert_nonempty(&output);
}

// ── Document ─────────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn document_convert_md_to_html() {
    let dir = output_root("document-md-html");
    let input = dir.join("in.md");
    let output = dir.join("out.html");
    std::fs::write(&input, SAMPLE_MD).expect("write input");

    let opts = ConvertOptions {
        output_format: "html".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let cancelled = Arc::new(AtomicBool::new(false));
    document::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &cancelled,
    )
    .expect("md→html conversion");
    assert_nonempty(&output);
}

#[test]
#[ignore]
fn document_convert_html_to_txt() {
    let dir = output_root("document-html-txt");
    let input = dir.join("in.html");
    let output = dir.join("out.txt");
    std::fs::write(&input, SAMPLE_HTML).expect("write input");

    let opts = ConvertOptions {
        output_format: "txt".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let cancelled = Arc::new(AtomicBool::new(false));
    document::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &cancelled,
    )
    .expect("html→txt conversion");
    assert_nonempty(&output);
}
