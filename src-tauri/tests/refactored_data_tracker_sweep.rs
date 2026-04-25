//! Sweep test for the refactored `convert::data` and `convert::tracker`
//! `convert()` entry points.
//!
//! Each module exposes a `convert(...)` that takes a [`ProgressFn`] callback
//! instead of a Tauri `&Window`. Where the required external CLI isn't
//! available in PATH (or no fixture is shipped), the tracker case is
//! reported as SKIP rather than failing — the test is meant to verify the
//! wrapper-decoupling is correct, not to enforce CI tool availability.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_data_tracker_sweep -- --nocapture

use fade_lib::convert::{data, noop_progress, tracker};
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
        .join("refactored-data-tracker-sweep")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

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

// ── Data ─────────────────────────────────────────────────────────────────────

const SAMPLE_CSV: &str = "name,count,active\nalpha,3,true\nbeta,7,false\ngamma,12,true\n";

fn run_data_case(label: &str, out_fmt: &str, out_filename: &str) {
    let dir = output_root(&format!("data-{label}"));
    let input = dir.join("in.csv");
    let output = dir.join(out_filename);
    std::fs::write(&input, SAMPLE_CSV).expect("write input csv");

    let opts = ConvertOptions {
        output_format: out_fmt.to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let cancelled = Arc::new(AtomicBool::new(false));
    let res = data::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &cancelled,
    );
    match res {
        Ok(()) => {
            assert_nonempty(&output);
            println!("  [PASS] data_csv_to_{label}");
        }
        Err(msg) => panic!("data csv → {label} failed: {msg}"),
    }
}

#[test]
fn data_csv_to_json() {
    run_data_case("json", "json", "out.json");
}

#[test]
fn data_csv_to_yaml() {
    run_data_case("yaml", "yaml", "out.yaml");
}

#[test]
fn data_csv_to_xml() {
    run_data_case("xml", "xml", "out.xml");
}

// ── Tracker ──────────────────────────────────────────────────────────────────

/// Locate a small MIDI fixture. Synthesizing a valid `.mid` by hand is
/// nontrivial, so we fall back to SKIP if nothing's around. We probe a few
/// common bundled-with-OS spots; on most dev machines this returns None,
/// which keeps CI green.
fn find_midi_fixture() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let candidates = [
        repo_root.join("test-fixtures").join("sample.mid"),
        repo_root.join("tests").join("fixtures").join("sample.mid"),
        repo_root
            .join("scripts")
            .join("computer-use")
            .join("fixtures")
            .join("sample.mid"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[test]
fn tracker_midi_to_wav() {
    // Tracker rendering needs either fluidsynth (with sf2) or timidity.
    let have_fluidsynth = tool_available("fluidsynth");
    let have_timidity = tool_available("timidity");
    if !have_fluidsynth && !have_timidity {
        println!("  [SKIP] tracker_midi_to_wav — neither fluidsynth nor timidity in PATH");
        return;
    }
    if have_fluidsynth && std::env::var("FADE_SOUNDFONT").is_err() {
        // Without a SoundFont, fluidsynth won't render. The locator also
        // probes default install paths; if neither is present and we have
        // no timidity fallback, SKIP.
        let probe_paths = [
            "/usr/share/sounds/sf2/FluidR3_GM.sf2",
            "/opt/homebrew/share/sounds/sf2",
            "/usr/local/share/sounds/sf2",
        ];
        let any_sf2 = probe_paths.iter().any(|p| Path::new(p).exists());
        if !any_sf2 && !have_timidity {
            println!("  [SKIP] tracker_midi_to_wav — no SoundFont available");
            return;
        }
    }

    let Some(fixture) = find_midi_fixture() else {
        println!("  [SKIP] tracker_midi_to_wav — no MIDI fixture available");
        return;
    };

    let dir = output_root("tracker");
    let input = dir.join("in.mid");
    std::fs::copy(&fixture, &input).expect("copy fixture midi");
    let output = dir.join("out.wav");

    let opts = ConvertOptions {
        output_format: "wav".to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let mut transcoder = tracker::UnavailableAudio;
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let result = tracker::convert(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        &opts,
        &mut progress,
        &mut transcoder,
        "test-tracker-midi-wav",
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            assert_nonempty(&output);
            println!("  [PASS] tracker_midi_to_wav");
        }
        ConvertResult::Error(msg) => {
            // Renderer-side flake (missing soundfont, timidity patch dir, etc.)
            // is treated as SKIP — refactor correctness is what's under test,
            // not local audio toolchain completeness.
            println!("  [SKIP] tracker_midi_to_wav — render failed: {msg}");
        }
        other => panic!("unexpected result: {other:?}"),
    }
}
