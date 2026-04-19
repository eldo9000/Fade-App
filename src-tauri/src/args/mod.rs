//! Pure arg builders for external tools (ffmpeg, ImageMagick).
//!
//! Every function here is pure: same inputs produce the same `Vec<String>`,
//! no I/O, no spawning, no state. That makes them trivially unit-testable
//! and safe to call anywhere — conversion pipeline, preview generation, tests.

pub mod audio;
pub mod image;
pub mod model;
pub mod video;

pub use audio::build_ffmpeg_audio_args;
pub use image::build_image_magick_args;
pub use model::{assimp_format_id, build_assimp_args};
pub use video::{build_ffmpeg_video_args, ffmpeg_video_codec_args, resolution_to_scale};
