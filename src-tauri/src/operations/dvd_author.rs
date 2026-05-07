//! DVD authoring: transcode input to MPEG-2, author a DVD filesystem,
//! and burn it to an ISO image.
//!
//! Tool chain required:
//!   - ffmpeg      — transcode to DVD-compliant MPEG-2
//!   - dvdauthor   — build the DVD-Video directory structure
//!   - mkisofs OR genisoimage — burn the directory into a .iso
//!
//! If any required tool is missing, the command returns a clear error
//! listing which tools are absent.

use crate::validate_output_name;
use std::io::Write;
use std::process::Command;

/// Check that all required DVD-authoring tools are present in PATH.
/// Returns Ok(mkisofs_name) on success (either "mkisofs" or "genisoimage"),
/// or Err with a message listing missing tools.
fn check_dvd_tools() -> Result<&'static str, String> {
    let ffmpeg = tool_in_path("ffmpeg");
    let dvdauthor = tool_in_path("dvdauthor");
    let mkisofs_name = if tool_in_path("mkisofs") {
        Some("mkisofs")
    } else if tool_in_path("genisoimage") {
        Some("genisoimage")
    } else {
        None
    };

    let mut missing = Vec::new();
    if !ffmpeg {
        missing.push("ffmpeg");
    }
    if !dvdauthor {
        missing.push("dvdauthor");
    }
    if mkisofs_name.is_none() {
        missing.push("mkisofs or genisoimage");
    }

    if !missing.is_empty() {
        return Err(format!(
            "DVD authoring requires: {}. Install the missing tools and try again.",
            missing.join(", ")
        ));
    }

    Ok(mkisofs_name.unwrap())
}

fn tool_in_path(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a subprocess, capturing stderr, and return Err if it fails.
fn run_cmd(program: &str, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to spawn `{program}`: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!(
            "`{program}` failed (exit {}): {}",
            out.status,
            stderr.lines().last().unwrap_or("(no output)")
        ))
    }
}

/// Author a DVD ISO from an arbitrary input video file.
///
/// Workflow:
/// 1. FFmpeg: transcode input to DVD-compliant MPEG-2 PS (.mpg)
/// 2. dvdauthor: build the VIDEO_TS directory from the MPEG-2 file
/// 3. mkisofs/genisoimage: burn the directory into a DVD-Video ISO
pub fn run(input: &str, output: &str) -> Result<(), String> {
    validate_output_name(output)?;

    let iso_bin = check_dvd_tools()?;

    // Derive temp paths from the output name to keep things co-located.
    let tmp_dir = std::env::temp_dir();
    let base = std::path::Path::new(output)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("dvd_author");

    let mpeg2_path = tmp_dir.join(format!("{base}_mpeg2.mpg"));
    let dvd_dir = tmp_dir.join(format!("{base}_dvd"));

    let mpeg2_str = mpeg2_path
        .to_str()
        .ok_or("temp path is not valid UTF-8")?
        .to_string();
    let dvd_dir_str = dvd_dir
        .to_str()
        .ok_or("temp DVD dir path is not valid UTF-8")?
        .to_string();

    // Step 1 — transcode to MPEG-2 DVD-video (NTSC: 720×480, AC3 audio).
    run_cmd(
        "ffmpeg",
        &[
            "-y",
            "-i",
            input,
            "-vcodec",
            "mpeg2video",
            "-b:v",
            "5000k",
            "-vf",
            "scale=720:480",
            "-acodec",
            "ac3",
            "-b:a",
            "192k",
            "-f",
            "dvd",
            &mpeg2_str,
        ],
    )
    .map_err(|e| format!("FFmpeg MPEG-2 transcode failed: {e}"))?;

    // Step 2 — generate dvdauthor XML and author the DVD directory.
    // Clean any existing partial directory.
    if dvd_dir.exists() {
        std::fs::remove_dir_all(&dvd_dir)
            .map_err(|e| format!("Cannot remove existing DVD dir: {e}"))?;
    }

    let xml_content = format!(
        r#"<dvdauthor dest="{dvd_dir_str}">
  <vmgm/>
  <titleset>
    <titles>
      <pgc>
        <vob file="{mpeg2_str}"/>
      </pgc>
    </titles>
  </titleset>
</dvdauthor>"#
    );

    // Write XML to a temp file.
    let xml_path = tmp_dir.join(format!("{base}_dvdauthor.xml"));
    let xml_str = xml_path
        .to_str()
        .ok_or("XML temp path is not valid UTF-8")?
        .to_string();
    {
        let mut f = std::fs::File::create(&xml_path)
            .map_err(|e| format!("Cannot create dvdauthor XML: {e}"))?;
        f.write_all(xml_content.as_bytes())
            .map_err(|e| format!("Cannot write dvdauthor XML: {e}"))?;
    }

    run_cmd("dvdauthor", &["-x", &xml_str]).map_err(|e| format!("dvdauthor failed: {e}"))?;

    // Step 3 — burn the directory to an ISO.
    run_cmd(iso_bin, &["-dvd-video", "-o", output, &dvd_dir_str])
        .map_err(|e| format!("ISO creation failed: {e}"))?;

    // Cleanup temporary files (best-effort; failures are non-fatal).
    let _ = std::fs::remove_file(&mpeg2_path);
    let _ = std::fs::remove_file(&xml_path);
    let _ = std::fs::remove_dir_all(&dvd_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_dvd_tools_returns_error_when_tools_missing() {
        // On machines without dvdauthor / mkisofs / genisoimage, this must
        // return an Err mentioning the missing tools.
        // On machines that have all tools, skip so we don't accidentally run
        // a real author.
        let has_all =
            tool_in_path("dvdauthor") && (tool_in_path("mkisofs") || tool_in_path("genisoimage"));
        if has_all && tool_in_path("ffmpeg") {
            eprintln!("skip check_dvd_tools: all tools present");
            return;
        }
        let result = check_dvd_tools();
        assert!(
            result.is_err(),
            "expected error when tools are missing, got Ok"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("requires"),
            "error should mention required tools, got: {msg}"
        );
    }
}
