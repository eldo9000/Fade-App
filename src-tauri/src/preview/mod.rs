//! Diff preview commands — re-encode a snippet and return a visual diff
//! against the original. Used by the UI to show compression artifacts before
//! committing to a full conversion.

pub mod image_quality;
pub mod video_diff;

pub use image_quality::preview_image_quality;
pub use video_diff::preview_diff;
