use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::ConvertOptions;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

// ── LibreOffice binary detection ──────────────────────────────────────────────

/// Locate the `soffice` binary. Returns `None` when LibreOffice is not found.
///
/// Search order:
/// 1. `libreoffice` in PATH
/// 2. `soffice` in PATH
/// 3. macOS bundle `/Applications/LibreOffice.app/Contents/MacOS/soffice`
/// 4. Windows default `C:\Program Files\LibreOffice\program\soffice.exe`
pub fn find_soffice() -> Option<String> {
    let candidates: &[&str] = &[
        "libreoffice",
        "soffice",
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        r"C:\Program Files\LibreOffice\program\soffice.exe",
    ];
    for candidate in candidates {
        if std::path::Path::new(candidate).is_absolute() {
            if std::path::Path::new(candidate).exists() {
                return Some(candidate.to_string());
            }
        } else {
            // Try via `which`/`where`
            let found = if cfg!(windows) {
                Command::new("where").arg(candidate).output().ok()
            } else {
                Command::new("which").arg(candidate).output().ok()
            };
            if found.map(|o| o.status.success()).unwrap_or(false) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

/// Convert a document using LibreOffice headless.
///
/// LibreOffice always writes `<outdir>/<input_stem>.<fmt>` — we rename the
/// result to `output_path` after the command completes.
///
/// Returns `Ok(())` on success, `Err(message)` on failure or missing binary.
pub fn libreoffice_convert(
    input: &str,
    output_format: &str,
    output_path: &str,
    progress: ProgressFn<'_>,
) -> Result<(), String> {
    let soffice = find_soffice().ok_or_else(|| {
        "Office conversion requires LibreOffice (https://www.libreoffice.org)".to_string()
    })?;

    // LibreOffice writes to `<outdir>/<input_stem>.<fmt>`.
    let out_dir = Path::new(output_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_string_lossy()
        .to_string();

    progress(ProgressEvent::Phase(format!(
        "Converting to {output_format} via LibreOffice…"
    )));

    let output = Command::new(&soffice)
        .args([
            "--headless",
            "--convert-to",
            output_format,
            "--outdir",
            &out_dir,
            input,
        ])
        .output()
        .map_err(|e| format!("Failed to spawn LibreOffice: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.to_string()
        } else {
            stdout.to_string()
        };
        return Err(format!(
            "LibreOffice conversion failed:\n{}",
            crate::truncate_stderr(&detail)
        ));
    }

    // Build the path LibreOffice wrote to: <outdir>/<input_stem>.<fmt>
    let input_stem = Path::new(input)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let lo_output = Path::new(&out_dir).join(format!("{input_stem}.{output_format}"));

    if !lo_output.exists() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "LibreOffice ran but output file was not created.\nExpected: {}\nStderr: {}",
            lo_output.display(),
            stderr
        ));
    }

    // Only rename if LibreOffice wrote somewhere different from our target.
    let target = Path::new(output_path);
    if lo_output != target {
        std::fs::rename(&lo_output, target)
            .map_err(|e| format!("Could not move LibreOffice output to target: {e}"))?;
    }

    progress(ProgressEvent::Done);
    Ok(())
}

/// Convert a document via pandoc CLI.
///
/// Used for paths LibreOffice handles poorly (e.g. DOCX → Markdown).
pub fn pandoc_convert(
    input: &str,
    output_format: &str,
    output_path: &str,
    progress: ProgressFn<'_>,
) -> Result<(), String> {
    let found = if cfg!(windows) {
        Command::new("where").arg("pandoc").output().ok()
    } else {
        Command::new("which").arg("pandoc").output().ok()
    };
    if !found.map(|o| o.status.success()).unwrap_or(false) {
        return Err("DOCX → Markdown conversion requires pandoc (https://pandoc.org)".to_string());
    }

    progress(ProgressEvent::Phase("Converting via pandoc…".to_string()));

    let output = Command::new("pandoc")
        .args([input, "-t", output_format, "-o", output_path])
        .output()
        .map_err(|e| format!("Failed to spawn pandoc: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pandoc failed:\n{}",
            crate::truncate_stderr(&stderr)
        ));
    }

    progress(ProgressEvent::Done);
    Ok(())
}

// ── macOS textutil helper (Pages → HTML) ─────────────────────────────────────

#[cfg(target_os = "macos")]
fn textutil_convert(
    input: &str,
    output_path: &str,
    progress: ProgressFn<'_>,
) -> Result<(), String> {
    progress(ProgressEvent::Phase("Converting via textutil…".to_string()));
    let output = Command::new("textutil")
        .args(["-convert", "html", "-output", output_path, input])
        .output()
        .map_err(|e| format!("Failed to spawn textutil: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("textutil failed:\n{}", stderr));
    }
    progress(ProgressEvent::Done);
    Ok(())
}

// ── Main conversion dispatcher ───────────────────────────────────────────────

/// Pure conversion. Used directly by tests and any future non-Tauri caller.
/// `cancelled` is accepted for signature parity with other modules but is
/// not consulted — current document conversions are pure-Rust and instant.
pub fn convert(
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
    progress: ProgressFn<'_>,
    _cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    progress(ProgressEvent::Started);

    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();

    // ── LibreOffice-backed office formats ─────────────────────────────────────

    // Word processing (DOCX → MD handled by pandoc; everything else via LO)
    if matches!(in_ext.as_str(), "docx" | "doc" | "rtf" | "odt") {
        if out_fmt == "md" {
            return pandoc_convert(input_path, "markdown", output_path, progress);
        }
        // Map output format to LibreOffice's --convert-to token
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "html" => "html",
            "txt" => "txt",
            "odt" => "odt",
            "docx" => "docx",
            "rtf" => "rtf",
            other => return Err(format!("Unsupported Word output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // Spreadsheets
    if matches!(in_ext.as_str(), "xlsx" | "xls" | "ods") {
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "ods" => "ods",
            "xlsx" => "xlsx",
            "csv" => "csv",
            other => return Err(format!("Unsupported spreadsheet output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // Presentations
    if matches!(in_ext.as_str(), "pptx" | "ppt" | "odp") {
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "odp" => "odp",
            "pptx" => "pptx",
            "png" => "png",
            other => return Err(format!("Unsupported presentation output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // Apple iWork — Pages
    if in_ext == "pages" {
        #[cfg(target_os = "macos")]
        if out_fmt == "html" {
            return textutil_convert(input_path, output_path, progress);
        }
        // For other output formats (or non-macOS), fall through to LibreOffice
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "html" => "html",
            "txt" => "txt",
            "docx" => "docx",
            other => return Err(format!("Unsupported Pages output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // Apple iWork — Numbers
    if in_ext == "numbers" {
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "xlsx" => "xlsx",
            "csv" => "csv",
            "ods" => "ods",
            other => return Err(format!("Unsupported Numbers output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // Apple iWork — Keynote
    if in_ext == "key" {
        let lo_fmt = match out_fmt.as_str() {
            "pdf" => "pdf",
            "pptx" => "pptx",
            "odp" => "odp",
            "png" => "png",
            other => return Err(format!("Unsupported Keynote output format: {other}")),
        };
        return libreoffice_convert(input_path, lo_fmt, output_path, progress);
    }

    // ── Pure-Rust text conversions ────────────────────────────────────────────

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;

    let output = match (in_ext.as_str(), out_fmt.as_str()) {
        ("md" | "markdown", "html") => {
            let parser = pulldown_cmark::Parser::new_ext(&raw, pulldown_cmark::Options::all());
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, parser);
            html
        }
        ("md" | "markdown", "txt") => strip_md(&raw),
        ("md" | "markdown", "md") => raw,
        ("html" | "htm", "txt") => html_to_text(&raw),
        ("html" | "htm", "md") => html_to_md(&raw),
        ("html" | "htm", "html") => raw,
        ("txt", "html") => {
            let escaped = raw
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let paragraphs: String = escaped
                .split("\n\n")
                .map(|p| format!("<p>{}</p>", p.trim().replace('\n', "<br>")))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "<!DOCTYPE html>\n<html><body>\n{}\n</body></html>",
                paragraphs
            )
        }
        ("txt", "md") => raw.clone(),
        ("txt", "txt") => raw,
        _ => return Err(format!("Unsupported conversion: {in_ext} → {out_fmt}")),
    };

    std::fs::write(output_path, output).map_err(|e| e.to_string())?;

    progress(ProgressEvent::Done);
    Ok(())
}

pub fn run(
    window: &Window,
    job_id: &str,
    input_path: &str,
    output_path: &str,
    opts: &ConvertOptions,
) -> Result<(), String> {
    let mut emit = crate::convert::window_progress_emitter(window, job_id, "Converting document…");
    let cancelled = Arc::new(AtomicBool::new(false));
    convert(input_path, output_path, opts, &mut emit, &cancelled)
}

pub fn strip_md(raw: &str) -> String {
    let mut txt = raw.to_string();
    // Code fences
    let mut result = String::new();
    let mut in_fence = false;
    for line in txt.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            result.push_str(line);
            result.push('\n');
        }
    }
    txt = result;
    // Headers
    txt = txt
        .lines()
        .map(|l| {
            let trimmed = l.trim_start_matches('#').trim_start();
            if l.starts_with('#') {
                trimmed.to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Bold/italic (simple passes)
    for marker in &["**", "__"] {
        while let (Some(s), Some(e)) = (
            txt.find(marker),
            txt[txt.find(marker).unwrap_or(0) + marker.len()..].find(marker),
        ) {
            let start = s;
            let end = s + marker.len() + e + marker.len();
            if end <= txt.len() {
                let inner = txt[s + marker.len()..s + marker.len() + e].to_string();
                txt = format!("{}{}{}", &txt[..start], inner, &txt[end..]);
            } else {
                break;
            }
        }
    }
    for marker in &["*", "_"] {
        while let (Some(s), Some(e)) = (
            txt.find(marker),
            txt.get(txt.find(marker).unwrap_or(0) + marker.len()..)
                .and_then(|t| t.find(marker)),
        ) {
            let start = s;
            let end = s + marker.len() + e + marker.len();
            if end <= txt.len() {
                let inner = txt[s + marker.len()..s + marker.len() + e].to_string();
                txt = format!("{}{}{}", &txt[..start], inner, &txt[end..]);
            } else {
                break;
            }
        }
    }
    // Inline code
    while let Some(s) = txt.find('`') {
        if let Some(e) = txt[s + 1..].find('`') {
            let inner = txt[s + 1..s + 1 + e].to_string();
            txt = format!("{}{}{}", &txt[..s], inner, &txt[s + 1 + e + 1..]);
        } else {
            break;
        }
    }
    // Links [text](url)
    while let Some(s) = txt.find('[') {
        if let Some(m) = txt[s + 1..].find("](") {
            let text_end = s + 1 + m;
            let text = txt[s + 1..text_end].to_string();
            if let Some(url_end) = txt[text_end + 2..].find(')') {
                let full_end = text_end + 2 + url_end + 1;
                txt = format!("{}{}{}", &txt[..s], text, &txt[full_end..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    // List markers
    txt = txt
        .lines()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
                t[2..].to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    txt.trim().to_string()
}

pub fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    // Decode basic HTML entities
    out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse whitespace
    let lines: Vec<&str> = out
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    lines.join("\n")
}

pub fn html_to_md(html: &str) -> String {
    let mut out = html.to_string();
    // Headings
    for n in (1u8..=6).rev() {
        let tag = format!("<h{n}");
        let close = format!("</h{n}>");
        let prefix = "#".repeat(n as usize) + " ";
        while let Some(s) = out.to_lowercase().find(&tag) {
            let tag_end = out[s..]
                .find('>')
                .map(|i| s + i + 1)
                .unwrap_or(s + tag.len() + 1);
            let close_pos = out[tag_end..]
                .to_lowercase()
                .find(&close)
                .map(|i| tag_end + i);
            if let Some(e) = close_pos {
                let inner = out[tag_end..e].to_string();
                out = format!(
                    "{}{}{}\n{}",
                    &out[..s],
                    prefix,
                    inner,
                    &out[e + close.len()..]
                );
            } else {
                break;
            }
        }
    }
    // Bold/italic
    for (open, close, md) in &[
        ("<strong>", "</strong>", "**"),
        ("<b>", "</b>", "**"),
        ("<em>", "</em>", "*"),
        ("<i>", "</i>", "*"),
    ] {
        while let Some(s) = out.to_lowercase().find(open) {
            let inner_start = s + open.len();
            if let Some(e) = out[inner_start..].to_lowercase().find(close) {
                let inner = out[inner_start..inner_start + e].to_string();
                out = format!(
                    "{}{}{}{}{}",
                    &out[..s],
                    md,
                    inner,
                    md,
                    &out[inner_start + e + close.len()..]
                );
            } else {
                break;
            }
        }
    }
    // Links <a href="url">text</a>
    while let Some(s) = out.to_lowercase().find("<a ") {
        let tag_end = match out[s..].find('>') {
            Some(i) => s + i + 1,
            None => break,
        };
        let href = {
            let tag_str = &out[s..tag_end];
            if let Some(h) = tag_str.to_lowercase().find("href=\"") {
                let start = s + h + 6;
                let end_q = out[start..].find('"').map(|i| start + i).unwrap_or(start);
                out[start..end_q].to_string()
            } else {
                String::new()
            }
        };
        if let Some(e) = out[tag_end..].to_lowercase().find("</a>") {
            let text = out[tag_end..tag_end + e].to_string();
            out = format!(
                "{}[{}]({}){}",
                &out[..s],
                text,
                href,
                &out[tag_end + e + 4..]
            );
        } else {
            break;
        }
    }
    // Code
    while let Some(s) = out.to_lowercase().find("<code>") {
        if let Some(e) = out[s + 6..].to_lowercase().find("</code>") {
            let inner = out[s + 6..s + 6 + e].to_string();
            out = format!("{}`{}`{}", &out[..s], inner, &out[s + 6 + e + 7..]);
        } else {
            break;
        }
    }
    // Paragraphs
    out = out.replace("<p>", "").replace("</p>", "\n\n");
    out = out
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    // List items
    out = out.replace("<li>", "- ").replace("</li>", "\n");
    out = out.replace("<ul>", "").replace("</ul>", "\n");
    out = out.replace("<ol>", "").replace("</ol>", "\n");
    // Strip remaining tags
    html_to_text(&out)
}
