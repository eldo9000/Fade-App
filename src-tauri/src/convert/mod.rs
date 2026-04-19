//! Per-media-type conversion pipelines.
//!
//! Each submodule owns one `run(...)` function that takes raw paths + options
//! and drives the conversion to completion, emitting `job-progress` events.
//! Process-spawning variants also take a shared `processes` map + cancellation
//! flag so `cancel_job` can kill them mid-run.

pub mod archive;
pub mod audio;
pub mod data;
pub mod document;
pub mod image;
pub mod model;
pub mod video;

pub use archive::run as run_archive_convert;
pub use audio::run as run_audio_convert;
pub use data::run as run_data_convert;
pub use document::run as run_document_convert;
pub use image::run as run_image_convert;
pub use model::run as run_model_convert;
pub use video::run as run_video_convert;
