use crate::args::build_image_magick_args;
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ext_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn tool_available(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── ImageMagick runner ────────────────────────────────────────────────────────

fn run_magick(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    let args = build_image_magick_args(input, output, opts);

    let mut child = match Command::new("magick")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return ConvertResult::Error(format!("ImageMagick not found: {e}")),
    };

    let stderr = child.stderr.take();

    {
        let mut map = processes.lock();
        map.insert(job_id.to_string(), child);
        if cancelled.load(Ordering::SeqCst) {
            if let Some(child) = map.get_mut(job_id) {
                let _ = child.kill();
            }
        }
    }

    let stderr_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            let reader = BufReader::new(s);
            for line in reader.lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines.join("\n")
    });

    let child_opt = {
        let mut map = processes.lock();
        map.remove(job_id)
    };

    let success = match child_opt {
        Some(mut child) => child.wait().map(|s| s.success()).unwrap_or(false),
        None => false,
    };

    let stderr_content = stderr_thread.join().unwrap_or_default();

    if cancelled.load(Ordering::SeqCst) {
        return ConvertResult::Cancelled;
    }

    if success {
        ConvertResult::Done
    } else {
        ConvertResult::Error(if stderr_content.trim().is_empty() {
            "ImageMagick convert failed".to_string()
        } else {
            truncate_stderr(&stderr_content)
        })
    }
}

// ── JPEG XL two-tool path ─────────────────────────────────────────────────────
//
// ImageMagick on this platform is built without --with-jxl.  Route JXL
// conversions through the cjxl (encode) / djxl (decode) CLI tools from the
// libjxl package instead.  The bridge step is:
//
//   INPUT  → PNG tmp  (via magick, if input is not already JXL)
//   PNG tmp → OUTPUT  (via cjxl, if output is JXL)
//   JXL INPUT → PNG tmp (via djxl, if input is JXL)
//   PNG tmp → OUTPUT  (via magick, if output is not JXL)

fn run_jxl(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    let in_ext = ext_of(input);
    let out_ext = ext_of(output);

    let input_is_jxl = in_ext == "jxl";
    let output_is_jxl = out_ext == "jxl";

    // Validate tool availability first.
    if output_is_jxl && !tool_available("cjxl") {
        return ConvertResult::Error(
            "JPEG XL output requires cjxl — install with: brew install jpeg-xl".to_string(),
        );
    }
    if input_is_jxl && !tool_available("djxl") {
        return ConvertResult::Error(
            "JPEG XL input requires djxl — install with: brew install jpeg-xl".to_string(),
        );
    }

    // Build a temporary PNG path used as the bridge file.
    let tmp = format!("/tmp/fade_jxl_bridge_{}.png", job_id);

    // Step 1: if input is JXL, decode to PNG via djxl.
    let effective_input: &str = if input_is_jxl {
        let status = Command::new("djxl")
            .args([input, &tmp])
            .status();
        match status {
            Ok(s) if s.success() => &tmp,
            Ok(s) => {
                return ConvertResult::Error(format!("djxl decode failed (exit {})", s));
            }
            Err(e) => return ConvertResult::Error(format!("djxl not found: {e}")),
        }
    } else {
        input
    };

    // If input is not JXL and output is JXL: magick → tmp.png → cjxl.
    // If input is JXL and output is not JXL: djxl bridge → magick.
    // If both JXL: djxl → cjxl directly.

    if output_is_jxl {
        // Encode to JXL.  If input was not JXL we need an intermediate PNG first.
        let png_for_cjxl = if input_is_jxl {
            // Already decoded to tmp above; use it directly.
            tmp.clone()
        } else {
            // Recode input to PNG via ImageMagick.
            let png_opts = ConvertOptions {
                output_format: "png".into(),
                ..opts.clone()
            };
            let result = run_magick(effective_input, &tmp, &png_opts, job_id, Arc::clone(&processes), cancelled);
            match result {
                ConvertResult::Done => {}
                other => {
                    let _ = std::fs::remove_file(&tmp);
                    return other;
                }
            }
            tmp.clone()
        };

        let quality_arg = opts
            .quality
            .map(|q| format!("--quality={}", q.clamp(0, 100)))
            .unwrap_or_else(|| "--quality=90".to_string());

        let status = Command::new("cjxl")
            .args([&png_for_cjxl, output, &quality_arg])
            .status();

        let _ = std::fs::remove_file(&png_for_cjxl);

        match status {
            Ok(s) if s.success() => {
                if cancelled.load(Ordering::SeqCst) {
                    ConvertResult::Cancelled
                } else {
                    ConvertResult::Done
                }
            }
            Ok(s) => ConvertResult::Error(format!("cjxl encode failed (exit {})", s)),
            Err(e) => ConvertResult::Error(format!("cjxl not found: {e}")),
        }
    } else {
        // Input was JXL; effective_input is now the decoded PNG tmp.
        // Pass it through ImageMagick to produce the desired output format.
        let result = run_magick(effective_input, output, opts, job_id, processes, cancelled);
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

// ── EXR (OpenEXR) ─────────────────────────────────────────────────────────────
//
// The installed ImageMagick build does not include OpenEXR support
// (--without-openexr).  Rather than silently fail with a cryptic IM error,
// detect the missing delegate at runtime and return a clear user-facing
// message.

fn check_exr_support() -> Option<ConvertResult> {
    // Probe IM's list of supported delegates for "openexr".
    let output = Command::new("magick")
        .args(["-list", "configure"])
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if text.contains("openexr") {
            return None; // supported
        }
    }
    Some(ConvertResult::Error(
        "EXR requires ImageMagick built with OpenEXR support. \
         Reinstall with: brew reinstall imagemagick --with-openexr \
         (or brew install imagemagick if OpenEXR is available)"
            .to_string(),
    ))
}

// ── RAW camera formats ────────────────────────────────────────────────────────
//
// ImageMagick reads RAW formats via its DNG/dcraw delegate.  The raw formats
// (CR2, CR3, NEF, ARW, DNG, ORF, RW2) are handled by the DNG delegate which
// shells out to dcraw or LibRaw internally.  These are READ-ONLY — you cannot
// write to RAW camera formats.  If the user selects a RAW format as the OUTPUT
// target, ImageMagick will fail; return a clear message.

fn check_raw_output(output_ext: &str) -> Option<ConvertResult> {
    match output_ext {
        "cr2" | "cr3" | "nef" | "arw" | "orf" | "rw2" | "raw" => Some(ConvertResult::Error(
            format!(
                "{} is a read-only camera RAW format — it cannot be used as an output target. \
                 Select a raster format (JPEG, PNG, TIFF) instead.",
                output_ext.to_uppercase()
            ),
        )),
        _ => None,
    }
}

// ── XCF (GIMP native) ────────────────────────────────────────────────────────
//
// XCF is read-only in ImageMagick (r--).  Guard against write attempts.

fn check_xcf_output(output_ext: &str) -> Option<ConvertResult> {
    if output_ext == "xcf" {
        Some(ConvertResult::Error(
            "XCF is a read-only format in ImageMagick — it cannot be used as an output target. \
             Select a raster format (PNG, TIFF) instead."
                .to_string(),
        ))
    } else {
        None
    }
}

// ── Public convert() entry point ──────────────────────────────────────────────

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    job_id: &str,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> ConvertResult {
    progress(ProgressEvent::Started);

    let out_ext = ext_of(output);
    let in_ext = ext_of(input);

    // Guard: output-only restrictions
    if let Some(e) = check_raw_output(&out_ext) {
        return e;
    }
    if let Some(e) = check_xcf_output(&out_ext) {
        return e;
    }

    // Route JXL through cjxl/djxl tools.
    let result = if in_ext == "jxl" || out_ext == "jxl" {
        run_jxl(input, output, opts, job_id, processes, cancelled)
    } else if in_ext == "exr" || out_ext == "exr" {
        if let Some(e) = check_exr_support() {
            return e;
        }
        run_magick(input, output, opts, job_id, processes, cancelled)
    } else {
        run_magick(input, output, opts, job_id, processes, cancelled)
    };

    if let ConvertResult::Done = &result {
        progress(ProgressEvent::Done);
    }
    result
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
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting image…");
    convert(
        input, output, opts, &mut emit, job_id, processes, &cancelled,
    )
}
