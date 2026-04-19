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
