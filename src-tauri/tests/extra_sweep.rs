//! Extra format sweep — exercises the cheap-to-test categories:
//!   - 3D models (assimp arg builder, already pub)
//!   - Subtitle pure-Rust paths (srt ↔ sbv)
//!   - Email pure-Rust paths (eml ↔ mbox)
//!   - Document pure-Rust paths (md/html text reduction)
//!   - Office document conversions (LibreOffice headless)
//!   - MSG email conversion (msgconvert / pst-convert)
//!
//! Categories that need a Tauri-runtime refactor (archive, font, ebook,
//! notebook, timeline, model-via-blender) are not covered here. They will
//! land after the `&Window` decoupling refactor.
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

// ── Office document sweep (LibreOffice headless) ──────────────────────────────
//
// Requires LibreOffice: brew install --cask libreoffice
// Skips automatically if `soffice` / `libreoffice` binary not found.

/// Minimal DOCX-compatible ZIP archive with a single paragraph.
/// Built from raw bytes so the test has zero binary fixtures in the repo.
fn write_minimal_docx(path: &std::path::Path) {
    use std::io::Write;
    // Minimal well-formed .docx (Word XML Open Packaging Convention)
    // Just enough for LibreOffice to open it.
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#;
    let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello from Fade test fixture.</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let buf = std::io::BufWriter::new(std::fs::File::create(path).expect("create docx"));
    let mut zip = zip::ZipWriter::new(buf);
    let opts: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", opts).unwrap();
    zip.write_all(content_types.as_bytes()).unwrap();
    zip.start_file("_rels/.rels", opts).unwrap();
    zip.write_all(rels.as_bytes()).unwrap();
    // word/_rels/document.xml.rels (minimal)
    zip.add_directory("word/", opts).unwrap();
    zip.add_directory("word/_rels/", opts).unwrap();
    let word_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;
    zip.start_file("word/_rels/document.xml.rels", opts)
        .unwrap();
    zip.write_all(word_rels.as_bytes()).unwrap();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();
    zip.finish().unwrap();
}

#[test]
#[ignore]
fn office_word_sweep() {
    // Skip if LibreOffice not installed
    if fade_lib::convert::document::find_soffice().is_none() {
        println!("[SKIP] office_word_sweep — LibreOffice not found");
        return;
    }

    let dir = output_root("office-word");
    let fixture = dir.join("_fixture.docx");
    write_minimal_docx(&fixture);

    let targets: &[(&str, &str)] = &[
        ("pdf", "docx_to_pdf.pdf"),
        ("html", "docx_to_html.html"),
        ("txt", "docx_to_txt.txt"),
        ("odt", "docx_to_odt.odt"),
        ("rtf", "docx_to_rtf.rtf"),
    ];

    let mut outcomes = Vec::new();
    for (fmt, filename) in targets {
        let out = dir.join(filename);
        let opts = ConvertOptions {
            output_format: fmt.to_string(),
            ..ConvertOptions::default()
        };
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut noop = fade_lib::convert::noop_progress();
        let err = document::convert(
            fixture.to_str().unwrap(),
            out.to_str().unwrap(),
            &opts,
            &mut noop,
            &cancelled,
        )
        .err();
        outcomes.push(Outcome {
            name: format!("docx_to_{fmt}"),
            output: out,
            error: err,
        });
    }

    report("office-word", outcomes);
}

#[test]
#[ignore]
fn office_spreadsheet_sweep() {
    if fade_lib::convert::document::find_soffice().is_none() {
        println!("[SKIP] office_spreadsheet_sweep — LibreOffice not found");
        return;
    }

    let dir = output_root("office-spreadsheet");

    // Build a minimal ODS (also ZIP/XML) fixture
    let ods_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.3">
  <office:body>
    <office:spreadsheet>
      <table:table table:name="Sheet1">
        <table:table-row>
          <table:table-cell><text:p>Hello</text:p></table:table-cell>
          <table:table-cell><text:p>World</text:p></table:table-cell>
        </table:table-row>
      </table:table>
    </office:spreadsheet>
  </office:body>
</office:document-content>"#;
    let fixture = dir.join("_fixture.ods");
    // Write raw ODS bytes via zip
    {
        use std::io::Write;
        let buf = std::io::BufWriter::new(std::fs::File::create(&fixture).expect("create ods"));
        let mut zip = zip::ZipWriter::new(buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("content.xml", opts).unwrap();
        zip.write_all(ods_content.as_bytes()).unwrap();
        let mimetype = "application/vnd.oasis.opendocument.spreadsheet";
        zip.start_file("mimetype", opts).unwrap();
        zip.write_all(mimetype.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    let targets: &[(&str, &str)] = &[
        ("pdf", "ods_to_pdf.pdf"),
        ("xlsx", "ods_to_xlsx.xlsx"),
        ("csv", "ods_to_csv.csv"),
    ];

    let mut outcomes = Vec::new();
    for (fmt, filename) in targets {
        let out = dir.join(filename);
        let opts = ConvertOptions {
            output_format: fmt.to_string(),
            ..ConvertOptions::default()
        };
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut noop = fade_lib::convert::noop_progress();
        let err = document::convert(
            fixture.to_str().unwrap(),
            out.to_str().unwrap(),
            &opts,
            &mut noop,
            &cancelled,
        )
        .err();
        outcomes.push(Outcome {
            name: format!("ods_to_{fmt}"),
            output: out,
            error: err,
        });
    }

    report("office-spreadsheet", outcomes);
}

#[test]
#[ignore]
fn office_presentation_sweep() {
    if fade_lib::convert::document::find_soffice().is_none() {
        println!("[SKIP] office_presentation_sweep — LibreOffice not found");
        return;
    }

    let dir = output_root("office-presentation");

    // Build a minimal ODP fixture
    let odp_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.3">
  <office:body>
    <office:presentation>
      <draw:page draw:name="Slide1">
        <draw:frame draw:x="1cm" draw:y="1cm" draw:width="10cm" draw:height="3cm">
          <draw:text-box><text:p>Fade Test Slide</text:p></draw:text-box>
        </draw:frame>
      </draw:page>
    </office:presentation>
  </office:body>
</office:document-content>"#;
    let fixture = dir.join("_fixture.odp");
    {
        use std::io::Write;
        let buf = std::io::BufWriter::new(std::fs::File::create(&fixture).expect("create odp"));
        let mut zip = zip::ZipWriter::new(buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("content.xml", opts).unwrap();
        zip.write_all(odp_content.as_bytes()).unwrap();
        let mimetype = "application/vnd.oasis.opendocument.presentation";
        zip.start_file("mimetype", opts).unwrap();
        zip.write_all(mimetype.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    let targets: &[(&str, &str)] = &[("pdf", "odp_to_pdf.pdf"), ("pptx", "odp_to_pptx.pptx")];

    let mut outcomes = Vec::new();
    for (fmt, filename) in targets {
        let out = dir.join(filename);
        let opts = ConvertOptions {
            output_format: fmt.to_string(),
            ..ConvertOptions::default()
        };
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut noop = fade_lib::convert::noop_progress();
        let err = document::convert(
            fixture.to_str().unwrap(),
            out.to_str().unwrap(),
            &opts,
            &mut noop,
            &cancelled,
        )
        .err();
        outcomes.push(Outcome {
            name: format!("odp_to_{fmt}"),
            output: out,
            error: err,
        });
    }

    report("office-presentation", outcomes);
}

// ── MSG email sweep ───────────────────────────────────────────────────────────
//
// Requires msgconvert (libemail-outlook-message-perl) or pst-convert (libpst).
// Skips automatically when neither tool is found.

#[test]
#[ignore]
fn msg_email_sweep() {
    use std::process::Command;

    let has_msgconvert = Command::new("which")
        .arg("msgconvert")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_pst_convert = Command::new("which")
        .arg("pst-convert")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_msgconvert && !has_pst_convert {
        println!("[SKIP] msg_email_sweep — neither msgconvert nor pst-convert found");
        return;
    }

    let dir = output_root("email-msg");

    // A minimal well-formed MSG file cannot be easily synthesised without the
    // full CFB (Compound File Binary) format. We test the error-path instead:
    // pass a text file with a .msg extension — msgconvert will fail, and we
    // verify the error is non-empty (pipeline is wired, not silently swallowed).
    let fixture = dir.join("_fixture.msg");
    std::fs::write(&fixture, b"Not a real MSG file - error path test").expect("write fixture");

    let out = dir.join("msg_to_eml.eml");
    let opts = ConvertOptions {
        output_format: "eml".to_string(),
        ..ConvertOptions::default()
    };
    let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut noop = fade_lib::convert::noop_progress();
    let result = email::convert(
        fixture.to_str().unwrap(),
        out.to_str().unwrap(),
        &opts,
        &mut noop,
        &cancelled,
    );

    // We expect either success (if the tool tolerates the fake file) or
    // a non-empty error (confirming the pipeline is wired and returns errors).
    let mut outcomes = Vec::new();
    match result {
        Ok(()) => {
            outcomes.push(Outcome {
                name: "msg_to_eml (fake file, accepted)".into(),
                output: out,
                error: None,
            });
        }
        Err(e) => {
            // Error is expected — the fixture is not a real MSG. What we're
            // testing is that the pipeline is wired and returns a non-trivial
            // error rather than panicking or returning a confusing message.
            let is_wired = !e.contains("MSG output is not supported")
                && !e.contains("Unsupported email conversion");
            outcomes.push(Outcome {
                name: "msg_to_eml (wired, expected error)".into(),
                // No output file — we synthesise a dummy path to satisfy the reporter
                output: dir.join("msg_pipeline_wired.sentinel"),
                error: if is_wired {
                    // Write the sentinel so the reporter sees a 0-byte file
                    let _ = std::fs::write(dir.join("msg_pipeline_wired.sentinel"), &e);
                    None
                } else {
                    Some(format!("MSG pipeline not wired — got old error: {e}"))
                },
            });
        }
    }

    report("email-msg", outcomes);
}
