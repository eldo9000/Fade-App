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
    // OBJ → USD, USDZ, ABC, BLEND; BLEND → OBJ (blend input); ABC → OBJ
    let blender_present = tool_available("blender");

    // OBJ → Blender-native formats
    let blender_out_targets: &[&str] = &["usd", "usdz", "abc", "blend"];
    for ext in blender_out_targets {
        let label = format!("cube_to_{ext}");
        if !blender_present {
            rows.push(Row::Skip(label, "blender not in PATH".to_string()));
            continue;
        }
        let out = dir.join(format!("cube_to_{ext}.{ext}"));
        rows.push(convert_one(&fixture, &out, ext, &label));
    }

    // ABC → OBJ (Alembic input via Blender)
    {
        let label = "abc_to_obj";
        if !blender_present {
            rows.push(Row::Skip(
                label.to_string(),
                "blender not in PATH".to_string(),
            ));
        } else {
            // Produce an ABC fixture from the OBJ we already made (if present).
            let abc_fixture = dir.join("cube_to_abc.abc");
            if abc_fixture.exists() {
                let out = dir.join("abc_to_obj.obj");
                let opts = ConvertOptions {
                    output_format: "obj".to_string(),
                    ..ConvertOptions::default()
                };
                let mut progress = noop_progress();
                let processes = Arc::new(Mutex::new(HashMap::new()));
                let cancelled = Arc::new(AtomicBool::new(false));
                let result = model::convert(
                    abc_fixture.to_str().unwrap(),
                    out.to_str().unwrap(),
                    &opts,
                    &mut progress,
                    "test-model-abc_to_obj",
                    processes,
                    &cancelled,
                );
                rows.push(match result {
                    ConvertResult::Done => Row::Pass(label.to_string()),
                    ConvertResult::Error(msg) => Row::Fail(label.to_string(), msg),
                    other => Row::Fail(label.to_string(), format!("unexpected: {other:?}")),
                });
            } else {
                rows.push(Row::Skip(
                    label.to_string(),
                    "abc fixture not produced (cube_to_abc failed)".to_string(),
                ));
            }
        }
    }

    // ── FreeCAD path (STEP/IGES) ──
    // Try `FreeCAD --version` then `freecad --version`.
    let freecad_present = tool_available("FreeCAD") || tool_available("freecad");

    // OBJ is not a valid CAD input — use STEP↔IGES cross-conversion.
    // We write a minimal valid ASCII STEP file as a fixture.
    const MINIMAL_STEP: &str = "ISO-10303-21;\n\
HEADER;\nFILE_DESCRIPTION(('Fade test'),'2;1');\n\
FILE_NAME('test.stp','2024-01-01T00:00:00',(''),(''),'','','');\n\
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\n\
ENDSEC;\nDATA;\n#1=PRODUCT('part','part','',(#2));\n\
#2=PRODUCT_CONTEXT('',#3,'mechanical');\n\
#3=APPLICATION_CONTEXT('automotive design');\n\
ENDSEC;\nEND-ISO-10303-21;\n";

    let step_fixture = dir.join("_fixture.stp");
    std::fs::write(&step_fixture, MINIMAL_STEP).expect("write step fixture");

    // STP → IGES
    {
        let label = "stp_to_iges";
        if !freecad_present {
            rows.push(Row::Skip(
                label.to_string(),
                "FreeCAD not in PATH (freecad.org)".to_string(),
            ));
        } else {
            let out = dir.join("stp_to_iges.iges");
            rows.push(convert_one(&step_fixture, &out, "iges", label));
        }
    }

    // IGES → STP  (round-trip: only if STP→IGES above produced output)
    {
        let label = "iges_to_stp";
        let iges_fixture = dir.join("stp_to_iges.iges");
        if !freecad_present {
            rows.push(Row::Skip(
                label.to_string(),
                "FreeCAD not in PATH (freecad.org)".to_string(),
            ));
        } else if iges_fixture.exists() {
            let out = dir.join("iges_to_stp.stp");
            rows.push(convert_one(&iges_fixture, &out, "stp", label));
        } else {
            rows.push(Row::Skip(
                label.to_string(),
                "iges fixture not produced (stp_to_iges failed)".to_string(),
            ));
        }
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
