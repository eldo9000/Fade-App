//! Comprehensive 3D-model sweep against the refactored `convert::model` and
//! `convert::model_blender` entry points.
//!
//! Each pair calls `model::convert(...)` directly (the dispatcher routes
//! blender-required pairs into `model_blender::convert(...)`). When the
//! required external tool isn't installed, the row reports SKIP rather than
//! failing — the test verifies wrapper-decoupling correctness, not CI tool
//! provisioning.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test refactored_model_sweep -- --nocapture

use fade_lib::convert::{model, noop_progress};
use fade_lib::{ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Minimal cube as Wavefront OBJ. Self-contained so we don't need a fixture
/// binary in the repo. Copy of the constant in `extra_sweep.rs` (do NOT
/// cross-import test files).
const CUBE_OBJ: &str = "\
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
v 0.0 0.0 1.0
v 1.0 0.0 1.0
v 1.0 1.0 1.0
v 0.0 1.0 1.0
f 1 2 3 4
f 5 6 7 8
f 1 2 6 5
f 2 3 7 6
f 3 4 8 7
f 4 1 5 8
";

fn output_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("refactored-model-sweep");
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

#[derive(Debug)]
enum Row {
    Pass(String),
    Skip(String, String),
    Fail(String, String),
}

fn convert_one(fixture: &Path, out: &Path, ext: &str, label: &str) -> Row {
    let opts = ConvertOptions {
        output_format: ext.to_string(),
        ..ConvertOptions::default()
    };
    let mut progress = noop_progress();
    let processes = Arc::new(Mutex::new(HashMap::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let job_id = format!("test-model-{label}");
    let result = model::convert(
        fixture.to_str().unwrap(),
        out.to_str().unwrap(),
        &opts,
        &mut progress,
        &job_id,
        processes,
        &cancelled,
    );
    match result {
        ConvertResult::Done => {
            if !out.exists() {
                return Row::Fail(
                    label.to_string(),
                    format!("output missing at {}", out.display()),
                );
            }
            // Some assimp output formats may produce a directory or sidecar
            // files; we only require the primary path the module reported.
            let len = out.metadata().map(|m| m.len()).unwrap_or(0);
            if len == 0 && out.is_file() {
                return Row::Fail(label.to_string(), "output is empty".to_string());
            }
            Row::Pass(label.to_string())
        }
        ConvertResult::Error(msg) => Row::Fail(label.to_string(), msg),
        other => Row::Fail(label.to_string(), format!("unexpected result: {other:?}")),
    }
}

#[test]
#[ignore]
fn refactored_model_sweep() {
    let dir = output_root();
    let fixture = dir.join("_fixture.obj");
    std::fs::write(&fixture, CUBE_OBJ).expect("write fixture");

    let mut rows: Vec<Row> = Vec::new();

    // ── assimp path ──
    let assimp_targets: &[&str] = &[
        "obj", "stl", "ply", "gltf", "glb", "dae", "fbx", "3ds", "x3d",
    ];
    let assimp_present = tool_available("assimp");
    for ext in assimp_targets {
        let label = format!("cube_to_{ext}");
        if !assimp_present {
            rows.push(Row::Skip(label, "assimp not in PATH".to_string()));
            continue;
        }
        let out = dir.join(format!("cube_to_{ext}.{ext}"));
        rows.push(convert_one(&fixture, &out, ext, &label));
    }

    // ── blender path ──
    let blender_targets: &[&str] = &["usd", "usdz", "abc", "blend"];
    let blender_present = tool_available("blender");
    for ext in blender_targets {
        let label = format!("cube_to_{ext}");
        if !blender_present {
            rows.push(Row::Skip(label, "blender not in PATH".to_string()));
            continue;
        }
        let out = dir.join(format!("cube_to_{ext}.{ext}"));
        rows.push(convert_one(&fixture, &out, ext, &label));
    }

    // ── Report ──
    let mut pass = 0;
    let mut skip = 0;
    let mut fail = 0;
    for row in &rows {
        match row {
            Row::Pass(name) => {
                pass += 1;
                println!("  [PASS] {name}");
            }
            Row::Skip(name, why) => {
                skip += 1;
                println!("  [SKIP] {name} — {why}");
            }
            Row::Fail(name, msg) => {
                fail += 1;
                println!("  [FAIL] {name} — {msg}");
            }
        }
    }
    println!(
        "refactored_model_sweep: {pass} pass, {skip} skip, {fail} fail (of {})",
        rows.len()
    );

    if fail > 0 {
        panic!("{fail} model conversion(s) failed");
    }
}
