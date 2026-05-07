//! 3D-model conversion pipeline — shells out to `assimp` (Open Asset Import
//! Library CLI). Follows the same shape as `convert::image` since assimp,
//! like ImageMagick, doesn't emit progress — it runs to completion and we
//! capture stderr for failure diagnosis.
//!
//! Formats that assimp cannot handle (USD, USDZ, Alembic, Blender native)
//! are delegated to `model_blender::convert` via `needs_blender`.
//!
//! CAD formats (STEP/IGES) are delegated to `convert_step_iges` which shells
//! out to FreeCAD's headless Python interface.

use crate::args::build_assimp_args;
use crate::args::model_blender::needs_blender;
use crate::convert::model_blender;
use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{truncate_stderr, ConvertOptions, ConvertResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Window;

/// Returns `true` if either the input or output extension requires FreeCAD
/// (STEP and IGES families).
fn needs_cad(input_ext: &str, output_ext: &str) -> bool {
    const CAD_FORMATS: &[&str] = &["stp", "step", "igs", "iges"];
    let i = input_ext.to_ascii_lowercase();
    let o = output_ext.to_ascii_lowercase();
    CAD_FORMATS.contains(&i.as_str()) || CAD_FORMATS.contains(&o.as_str())
}

/// Locate the FreeCAD binary. Tries `FreeCAD` then `freecad` in PATH,
/// then common install locations on macOS and Linux.
fn find_freecad() -> Option<std::path::PathBuf> {
    // 1. PATH lookup
    for name in &["FreeCAD", "freecad"] {
        #[cfg(windows)]
        let which_cmd = "where";
        #[cfg(not(windows))]
        let which_cmd = "which";
        if let Ok(out) = Command::new(which_cmd).arg(name).output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                let first = s.lines().next().unwrap_or("").trim().to_string();
                if !first.is_empty() {
                    return Some(std::path::PathBuf::from(first));
                }
            }
        }
    }

    // 2. macOS app bundle
    #[cfg(target_os = "macos")]
    {
        let system = std::path::PathBuf::from("/Applications/FreeCAD.app/Contents/MacOS/FreeCAD");
        if system.exists() {
            return Some(system);
        }
        if let Some(home) = std::env::var_os("HOME") {
            let user = std::path::PathBuf::from(home)
                .join("Applications/FreeCAD.app/Contents/MacOS/FreeCAD");
            if user.exists() {
                return Some(user);
            }
        }
    }

    // 3. Linux known paths
    #[cfg(target_os = "linux")]
    {
        for p in &[
            "/usr/bin/FreeCAD",
            "/usr/bin/freecad",
            "/usr/local/bin/FreeCAD",
        ] {
            let pb = std::path::PathBuf::from(p);
            if pb.exists() {
                return Some(pb);
            }
        }
    }

    None
}

/// Convert between CAD formats (STEP, IGES) using FreeCAD's headless Python
/// interface (`freecad --console`). Emits `ConvertResult::Error` with an
/// install hint if FreeCAD is not found.
pub fn convert_step_iges(input: &str, output: &str, progress: ProgressFn<'_>) -> ConvertResult {
    progress(ProgressEvent::Started);

    let freecad = match find_freecad() {
        Some(b) => b,
        None => {
            return ConvertResult::Error(
                "CAD formats (STEP/IGES) require FreeCAD.\n\n\
                 Install from https://freecad.org\n\
                 macOS:   brew install --cask freecad\n\
                 Linux:   apt install freecad  (or equivalent)\n\
                 Windows: download from https://freecad.org"
                    .to_string(),
            );
        }
    };

    let in_ext = Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let out_ext = Path::new(output)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    // Inline Python: import with FreeCAD Part workbench, export to target.
    let script = format!(
        r#"import FreeCAD, Part, sys
doc = FreeCAD.newDocument("fade_cad")
try:
    shape = Part.read("{input}")
except Exception as e:
    sys.stderr.write("FreeCAD import failed: " + str(e) + "\n")
    sys.exit(1)
Part.export([shape], "{output}")
print("FADE_FREECAD_OK")
sys.exit(0)
"#,
        input = input.replace('\\', "/"),
        output = output.replace('\\', "/"),
    );

    // Validate that input/output ext combo is sensible (both CAD or cross-CAD).
    let supported = ["stp", "step", "igs", "iges"];
    if !supported.contains(&in_ext.as_str()) && !supported.contains(&out_ext.as_str()) {
        return ConvertResult::Error(format!(
            "convert_step_iges called with unsupported pair: {in_ext} → {out_ext}"
        ));
    }

    let mut child = match Command::new(&freecad)
        .args(["--console"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("failed to launch FreeCAD: {e}")),
    };

    // Write the script to FreeCAD's stdin.
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        let _ = stdin.write_all(script.as_bytes());
        // stdin drops here, closing the pipe so FreeCAD exits after the script.
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Drain stdout and watch for the OK sentinel.
    let sentinel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sentinel_clone = Arc::clone(&sentinel_flag);
    let stdout_handle = stdout.map(move |s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                if line.trim() == "FADE_FREECAD_OK" {
                    sentinel_clone.store(true, Ordering::SeqCst);
                }
            }
        })
    });

    let stderr_content = {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    };

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }

    let exit_success = child.wait().map(|s| s.success()).unwrap_or(false);
    let sentinel_seen = sentinel_flag.load(Ordering::SeqCst);

    if exit_success && sentinel_seen {
        progress(ProgressEvent::Done);
        ConvertResult::Done
    } else if exit_success && !sentinel_seen {
        ConvertResult::Error(
            "FreeCAD conversion finished but produced no output sentinel".to_string(),
        )
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "FreeCAD conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// Dispatches to `model_blender::convert` for formats assimp cannot handle.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    let input_ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let output_ext = opts.output_format.as_str();

    if needs_blender(input_ext, output_ext) {
        return model_blender::convert(input, output, opts, progress, job_id, processes, cancelled);
    }

    if needs_cad(input_ext, output_ext) {
        return convert_step_iges(input, output, progress);
    }

    progress(ProgressEvent::Started);

    let args = build_assimp_args(input, output, opts);

    let mut child = match Command::new("assimp")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("assimp not found: {e}\n\nInstall with:\n  macOS:   brew install assimp\n  Linux:   apt install assimp-utils  (or equivalent)\n  Windows: scoop install assimp")),
    };

    // Assimp writes progress-ish info to stdout and errors to stderr.
    // Drain both so the child can't block on a full pipe. We only surface
    // stderr on failure.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
    }

    // Drain stdout in a background thread so we never deadlock on a full
    // pipe while blocked reading stderr.
    let stdout_handle = stdout.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for _ in reader.lines().map_while(Result::ok) {
                // assimp stdout is chatty ("export took approx. 0.001s") —
                // we don't surface it. Just keep the pipe flowing.
            }
        })
    });

    let stderr_content = {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    };

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }

    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }

    if success {
        progress(ProgressEvent::Done);
        ConvertResult::Done
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "assimp conversion failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}

pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting 3D model…");
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
