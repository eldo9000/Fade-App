use crate::convert::progress::{ProgressEvent, ProgressFn};
use crate::{ConvertOptions, JobProgress};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, Window};

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

    let raw = std::fs::read_to_string(input_path).map_err(|e| e.to_string())?;
    let in_ext = Path::new(input_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let out_fmt = opts.output_format.to_lowercase();

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
    let job_id_owned = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = match ev {
            ProgressEvent::Started => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: "Converting document…".to_string(),
            },
            ProgressEvent::Phase(msg) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: msg,
            },
            ProgressEvent::Percent(p) => JobProgress {
                job_id: job_id_owned.clone(),
                percent: (p * 100.0).clamp(0.0, 100.0),
                message: String::new(),
            },
            ProgressEvent::Done => JobProgress {
                job_id: job_id_owned.clone(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        };
        let _ = win.emit("job-progress", payload);
    };
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
