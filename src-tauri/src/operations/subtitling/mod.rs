//! Subtitling · Analyze tab backend.
//!
//!   · probe_subtitles — list subtitle streams in a media file via ffprobe
//!   · lint_subtitle   — parse SRT/VTT/ASS, return cue-level warnings
//!   · diff_subtitle   — line-oriented diff between two subtitle files

pub mod diff;
pub mod lint;
pub mod probe;

pub use diff::{diff_subtitle, SubDiffLine};
pub use lint::{lint_subtitle, LintIssue, LintThresholds};
pub use probe::{probe_subtitles, SubStream};

/// Upper bound on the size of a subtitle file accepted by `diff_subtitle` and
/// `lint_subtitle`. Real captions never come close; a 32 MiB plain-text file
/// is ~16h of dialogue. Larger inputs are almost certainly wrong media types
/// or hostile, and `read_to_string`'ing them eats RAM on the command thread.
pub(crate) const SUBTITLE_MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Read a subtitle file with a hard byte ceiling. Returns a typed-but-stringy
/// error message so callers can forward straight to the frontend.
///
/// The check is done via `File::metadata().len()` plus `take(SUBTITLE_MAX_BYTES)`
/// so a zero-byte file lying about its size (FIFO, /proc entry, etc.) can't
/// slip past the metadata gate.
pub(crate) fn read_subtitle_capped(path: &str) -> Result<String, String> {
    use std::io::Read;

    let file = std::fs::File::open(path).map_err(|e| format!("read subtitle: {e}"))?;
    if let Ok(meta) = file.metadata() {
        if meta.len() > SUBTITLE_MAX_BYTES {
            return Err(format!(
                "subtitle file too large: {} bytes (max {})",
                meta.len(),
                SUBTITLE_MAX_BYTES
            ));
        }
    }
    let mut body = String::new();
    file.take(SUBTITLE_MAX_BYTES + 1)
        .read_to_string(&mut body)
        .map_err(|e| format!("read subtitle: {e}"))?;
    if body.len() as u64 > SUBTITLE_MAX_BYTES {
        return Err(format!(
            "subtitle file too large: >{} bytes",
            SUBTITLE_MAX_BYTES
        ));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "fade-sub-test-{}-{}-{}",
            tag,
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn read_subtitle_capped_accepts_small_file() {
        let dir = unique_tmp("sub-ok");
        let f = dir.join("a.srt");
        fs::write(&f, b"1\n00:00:00,000 --> 00:00:01,000\nhi\n").unwrap();
        let body = read_subtitle_capped(f.to_str().unwrap()).unwrap();
        assert!(body.contains("hi"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_subtitle_capped_rejects_oversized_file() {
        let dir = unique_tmp("sub-big");
        let f = dir.join("huge.srt");
        // Write one byte past the cap — cheap and deterministic.
        let big = vec![b'x'; (SUBTITLE_MAX_BYTES as usize) + 1];
        fs::write(&f, &big).unwrap();
        let err = read_subtitle_capped(f.to_str().unwrap()).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_subtitle_capped_errors_on_missing_file() {
        let missing = std::env::temp_dir().join(format!(
            "fade-sub-nope-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let err = read_subtitle_capped(missing.to_str().unwrap()).unwrap_err();
        assert!(err.contains("read subtitle"), "got: {err}");
    }
}
