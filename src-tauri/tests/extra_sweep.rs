//! Extra format sweep — exercises the cheap-to-test categories:
//!   - 3D models (assimp arg builder, already pub)
//!   - Subtitle pure-Rust paths (srt ↔ sbv)
//!   - Email pure-Rust paths (eml ↔ mbox)
//!   - Document pure-Rust paths (md/html text reduction)
//!
//! Categories that need a Tauri-runtime refactor (archive, font, ebook,
//! notebook, timeline, document-via-pandoc, model-via-blender) are not
//! covered here. They will land after the `&Window` decoupling refactor.
//!
//! Run:
//!   cargo test --manifest-path src-tauri/Cargo.toml --test extra_sweep \
//!     -- --include-ignored --nocapture
//!
//! Outputs land in `test-results/extra-sweep/<category>/`.

use fade_lib::{
    build_assimp_args,
    convert::{document, email, subtitle},
    ConvertOptions,
};
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Shared ───────────────────────────────────────────────────────────────────

fn output_root(category: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.parent().expect("repo root");
    let dir = repo_root
        .join("test-results")
        .join("extra-sweep")
        .join(category);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear previous run");
    }
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

fn run_cmd(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("spawn `{program}` failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "exit {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("(no stderr)")
                .chars()
                .take(160)
                .collect::<String>()
        ));
    }
    Ok(())
}

struct Outcome {
    name: String,
    output: PathBuf,
    error: Option<String>,
}

impl Outcome {
    fn passed(&self) -> bool {
        self.error.is_none()
            && self.output.exists()
            && self.output.metadata().map(|m| m.len() > 0).unwrap_or(false)
    }
}

fn report(category: &str, outcomes: Vec<Outcome>) {
    let total = outcomes.len();
    let failed = outcomes.iter().filter(|o| !o.passed()).count();
    println!();
    println!(
        "── {} extra sweep ─ {} cases, {} passed, {} failed",
        category,
        total,
        total - failed,
        failed
    );
    for o in &outcomes {
        let status = if o.passed() { "PASS" } else { "FAIL" };
        let size = o.output.metadata().map(|m| m.len()).unwrap_or(0);
        let detail = o.error.as_deref().unwrap_or("");
        println!(
            "  [{}] {:<48} {:>10} bytes  {}",
            status, o.name, size, detail
        );
    }
    println!();
}

// ── 3D model sweep ───────────────────────────────────────────────────────────

/// Minimal cube as Wavefront OBJ. Self-contained so we don't need a
/// fixture binary in the repo.
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

#[test]
#[ignore]
fn model_sweep() {
    let dir = output_root("model");
    let fixture = dir.join("_fixture.obj");
    std::fs::write(&fixture, CUBE_OBJ).expect("write fixture");

    // assimp-supported targets (excludes blend/usd/usdz/abc which use Blender)
    let targets: &[&str] = &[
        "obj", "stl", "ply", "gltf", "glb", "dae", "fbx", "3ds", "x3d",
    ];

    let mut outcomes = Vec::new();
    for ext in targets {
        let out = dir.join(format!("cube_to_{ext}.{ext}"));
        let opts = ConvertOptions {
            output_format: (*ext).to_string(),
            ..ConvertOptions::default()
        };
        let args = build_assimp_args(
            fixture.to_str().expect("fixture path"),
            out.to_str().expect("out path"),
            &opts,
        );
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let err = run_cmd("assimp", &arg_refs).err();
        outcomes.push(Outcome {
            name: format!("cube_to_{ext}"),
            output: out,
            error: err,
        });
    }

    report("model", outcomes);
}

// ── Subtitle pure-Rust sweep ─────────────────────────────────────────────────

const SAMPLE_SRT: &str = "\
1
00:00:01,200 --> 00:00:03,400
Hello world

2
00:00:04,000 --> 00:00:06,500
Second cue
";

const SAMPLE_SBV: &str = "\
0:00:01.200,0:00:03.400
Hello world

0:00:04.000,0:00:06.500
Second cue
";

#[test]
#[ignore]
fn subtitle_sweep() {
    let dir = output_root("subtitle");

    let mut outcomes = Vec::new();

    // SRT → SBV
    let out_sbv = dir.join("srt_to_sbv.sbv");
    let result = std::fs::write(&out_sbv, subtitle::srt_to_sbv(SAMPLE_SRT));
    outcomes.push(Outcome {
        name: "srt_to_sbv".into(),
        output: out_sbv,
        error: result.err().map(|e| e.to_string()),
    });

    // SBV → SRT
    let out_srt = dir.join("sbv_to_srt.srt");
    let result = std::fs::write(&out_srt, subtitle::sbv_to_srt(SAMPLE_SBV));
    outcomes.push(Outcome {
        name: "sbv_to_srt".into(),
        output: out_srt,
        error: result.err().map(|e| e.to_string()),
    });

    // Round-trip srt → sbv → srt should preserve cue text
    let sbv = subtitle::srt_to_sbv(SAMPLE_SRT);
    let srt_back = subtitle::sbv_to_srt(&sbv);
    let out_roundtrip = dir.join("srt_roundtrip.srt");
    std::fs::write(&out_roundtrip, &srt_back).expect("write roundtrip");
    let preserved = srt_back.contains("Hello world") && srt_back.contains("Second cue");
    outcomes.push(Outcome {
        name: "srt_roundtrip_preserves_text".into(),
        output: out_roundtrip,
        error: if preserved {
            None
        } else {
            Some("round-trip lost cue text".into())
        },
    });

    report("subtitle", outcomes);
}

// ── Email pure-Rust sweep ────────────────────────────────────────────────────

const SAMPLE_EML: &str = "\
From: alice@example.com
To: bob@example.com
Subject: Hello
Date: Mon, 1 Jan 2024 00:00:00 +0000

Hello, this is the body.
Line two of the body.
";

#[test]
#[ignore]
fn email_sweep() {
    let dir = output_root("email");

    let mut outcomes = Vec::new();

    // EML → MBOX
    let out_mbox = dir.join("eml_to_mbox.mbox");
    let mbox_text = email::eml_to_mbox(SAMPLE_EML);
    let result = std::fs::write(&out_mbox, &mbox_text);
    outcomes.push(Outcome {
        name: "eml_to_mbox".into(),
        output: out_mbox,
        error: result.err().map(|e| e.to_string()),
    });

    // MBOX → EML
    let out_eml = dir.join("mbox_to_eml.eml");
    match email::mbox_to_eml(&mbox_text) {
        Ok(eml_text) => {
            let result = std::fs::write(&out_eml, eml_text);
            outcomes.push(Outcome {
                name: "mbox_to_eml".into(),
                output: out_eml,
                error: result.err().map(|e| e.to_string()),
            });
        }
        Err(e) => outcomes.push(Outcome {
            name: "mbox_to_eml".into(),
            output: out_eml,
            error: Some(e),
        }),
    }

    report("email", outcomes);
}

// ── Document pure-Rust sweep ─────────────────────────────────────────────────

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
<ul><li>bullet one</li><li>bullet two</li></ul>
<a href=\"http://example.com\">link</a>
</body></html>
";

#[test]
#[ignore]
fn document_text_sweep() {
    let dir = output_root("document-text");

    let mut outcomes = Vec::new();

    // md → txt
    let out_md_txt = dir.join("md_to_txt.txt");
    let txt = document::strip_md(SAMPLE_MD);
    let nonempty = !txt.trim().is_empty();
    let result = std::fs::write(&out_md_txt, txt);
    outcomes.push(Outcome {
        name: "md_to_txt".into(),
        output: out_md_txt,
        error: match result {
            Err(e) => Some(e.to_string()),
            Ok(()) if !nonempty => Some("strip_md produced empty output".into()),
            Ok(()) => None,
        },
    });

    // html → txt
    let out_html_txt = dir.join("html_to_txt.txt");
    let txt = document::html_to_text(SAMPLE_HTML);
    let nonempty = !txt.trim().is_empty();
    let result = std::fs::write(&out_html_txt, txt);
    outcomes.push(Outcome {
        name: "html_to_txt".into(),
        output: out_html_txt,
        error: match result {
            Err(e) => Some(e.to_string()),
            Ok(()) if !nonempty => Some("html_to_text produced empty output".into()),
            Ok(()) => None,
        },
    });

    // html → md
    let out_html_md = dir.join("html_to_md.md");
    let md = document::html_to_md(SAMPLE_HTML);
    let nonempty = !md.trim().is_empty();
    let result = std::fs::write(&out_html_md, md);
    outcomes.push(Outcome {
        name: "html_to_md".into(),
        output: out_html_md,
        error: match result {
            Err(e) => Some(e.to_string()),
            Ok(()) if !nonempty => Some("html_to_md produced empty output".into()),
            Ok(()) => None,
        },
    });

    report("document-text", outcomes);
}
