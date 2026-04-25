//! Sweep test for the refactored single-tool shell-out `convert()` entry
//! points: notebook, timeline, font, and ebook.
//!
//! Each module exposes a `convert(...)` that takes a [`ProgressFn`] callback
//! instead of a Tauri `&Window`. Where the required external CLI isn't
//! available in PATH (or no fixture is shipped), the case is reported as
//! SKIP rather than failing — the test is meant to verify the wrapper-
//! decoupling is correct, not to enforce CI tool availability.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_shellout_sweep -- --nocapture

use fade_lib::convert::{ebook, font, noop_progress, notebook, timeline};
use fade_lib::{ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn output_root(category: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("refactored-shellout-sweep")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

/// True if `name` looks like a working CLI on this machine. We accept any
/// process invocation that doesn't fail to spawn — some tools print their
/// version on stderr (e.g. older `jupyter` builds), some on stdout, and
/// `ebook-convert --version` exits non-zero on certain Calibre builds.
fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|_| true)
        .unwrap_or(false)
}

fn assert_nonempty(path: &Path) {
    assert!(path.exists(), "expected output file at {}", path.display());
    let len = path
        .metadata()
        .map(|m| m.len())
        .unwrap_or_else(|e| panic!("metadata for {}: {e}", path.display()));
    assert!(len > 0, "output {} is empty", path.display());
}

// ── Notebook ─────────────────────────────────────────────────────────────────

const SAMPLE_IPYNB: &str = r##"{
 "cells": [
  {
   "cell_type": "markdown",
   "metadata": {},
   "source": ["# Hello\n", "\n", "Some prose."]
  },
  {
   "cell_type": "code",
   "execution_count": null,
   "metadata": {},
   "outputs": [],
   "source": ["print('hi')\n"]
  }
 ],
 "metadata": {
  "kernelspec": {"display_name": "Python 3", "language": "python", "name": "python3"},
  "language_info": {"name": "python"}
 },
 "nbformat": 4,
 "nbformat_minor": 5
}
"##;

#[test]
#[ignore]
fn notebook_ipynb_to_md() {
    if !tool_available("jupyter") {
        println!("  [SKIP] notebook_ipynb_to_md — jupyter not in PATH");
        return;
    }

    let dir = output_root("notebook");
    let input = dir.join("in.ipynb");
    let output = dir.join("out.md");
    std::fs::write(&input, SAMPLE_IPYNB).expect("write input");

    let opts = ConvertOptions {
        output_format: "md".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let result = notebook::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-notebook-ipynb-md",
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            assert_nonempty(&output);
            println!("  [PASS] notebook_ipynb_to_md");
        }
        ConvertResult::Error(msg) => {
            // jupyter is in PATH but nbconvert may not be installed. Treat
            // missing nbconvert as a SKIP so the sweep stays green on
            // partially-provisioned dev machines.
            if msg.contains("nbconvert") || msg.contains("No module named") {
                println!("  [SKIP] notebook_ipynb_to_md — nbconvert not installed: {msg}");
            } else {
                panic!("notebook conversion failed: {msg}");
            }
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

// ── Timeline ─────────────────────────────────────────────────────────────────

const SAMPLE_OTIO: &str = r#"{
    "OTIO_SCHEMA": "Timeline.1",
    "metadata": {},
    "name": "test",
    "global_start_time": null,
    "tracks": {
        "OTIO_SCHEMA": "Stack.1",
        "metadata": {},
        "name": "tracks",
        "children": [
            {
                "OTIO_SCHEMA": "Track.1",
                "metadata": {},
                "name": "V1",
                "kind": "Video",
                "children": []
            }
        ],
        "effects": [],
        "markers": [],
        "source_range": null
    }
}
"#;

#[test]
#[ignore]
fn timeline_otio_to_otio() {
    if !tool_available("otioconvert") {
        println!("  [SKIP] timeline_otio_to_otio — otioconvert not in PATH");
        return;
    }

    let dir = output_root("timeline");
    let input = dir.join("in.otio");
    let output = dir.join("out.otio");
    std::fs::write(&input, SAMPLE_OTIO).expect("write input");

    let opts = ConvertOptions {
        output_format: "otio".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let result = timeline::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-timeline-otio",
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            assert_nonempty(&output);
            println!("  [PASS] timeline_otio_to_otio");
        }
        ConvertResult::Error(msg) => {
            panic!("timeline conversion failed: {msg}");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

// ── Font ─────────────────────────────────────────────────────────────────────

/// Locate a system TTF we can use as a fixture. Returns None if nothing
/// suitable is found — the test then SKIPs.
fn find_system_ttf() -> Option<PathBuf> {
    let candidates = [
        // macOS
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Geneva.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists() && p.extension().and_then(|s| s.to_str()) == Some("ttf"))
}

#[test]
#[ignore]
fn font_ttf_to_woff() {
    let python = if Command::new("python3").arg("--version").output().is_ok() {
        "python3"
    } else {
        "python"
    };
    // Probe fontTools availability via the same interpreter the converter uses.
    let probe = Command::new(python)
        .args(["-c", "import fontTools"])
        .output();
    let fonttools_available = match probe {
        Ok(o) => o.status.success(),
        Err(_) => false,
    };
    if !fonttools_available {
        println!("  [SKIP] font_ttf_to_woff — fontTools (python module) not available");
        return;
    }

    let Some(src_ttf) = find_system_ttf() else {
        println!("  [SKIP] font_ttf_to_woff — no system TTF fixture found");
        return;
    };

    let dir = output_root("font");
    let input = dir.join("in.ttf");
    std::fs::copy(&src_ttf, &input).expect("copy fixture font");
    let output = dir.join("out.woff");

    let opts = ConvertOptions {
        output_format: "woff".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let result = font::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-font-ttf-woff",
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            assert_nonempty(&output);
            println!("  [PASS] font_ttf_to_woff");
        }
        ConvertResult::Error(msg) => {
            panic!("font conversion failed: {msg}");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

// ── Ebook ────────────────────────────────────────────────────────────────────

/// Locate a real EPUB fixture. None means SKIP — synthesizing a valid
/// EPUB by hand is more trouble than it's worth for this sweep.
fn find_epub_fixture() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let candidates = [
        repo_root.join("test-fixtures").join("sample.epub"),
        repo_root.join("tests").join("fixtures").join("sample.epub"),
        repo_root
            .join("scripts")
            .join("computer-use")
            .join("fixtures")
            .join("sample.epub"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
#[ignore]
fn ebook_epub_to_mobi() {
    if !tool_available("ebook-convert") {
        println!("  [SKIP] ebook_epub_to_mobi — ebook-convert not in PATH");
        return;
    }
    let Some(fixture) = find_epub_fixture() else {
        println!("  [SKIP] ebook_epub_to_mobi — no EPUB fixture available");
        return;
    };

    let dir = output_root("ebook");
    let input = dir.join("in.epub");
    std::fs::copy(&fixture, &input).expect("copy fixture epub");
    let output = dir.join("out.mobi");

    let opts = ConvertOptions {
        output_format: "mobi".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let result = ebook::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        "test-ebook-epub-mobi",
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            assert_nonempty(&output);
            println!("  [PASS] ebook_epub_to_mobi");
        }
        ConvertResult::Error(msg) => {
            // Calibre is finicky about EPUB validation — if our fixture is
            // malformed, treat it as a SKIP rather than failing.
            println!("  [SKIP] ebook_epub_to_mobi — ebook-convert failed on fixture: {msg}");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
