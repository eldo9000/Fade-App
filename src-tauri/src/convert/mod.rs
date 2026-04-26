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
pub mod ebook;
pub mod email;
pub mod font;
pub mod image;
pub mod model;
pub mod model_blender;
pub mod notebook;
pub mod progress;
pub mod subtitle;
pub mod timeline;
pub mod tracker;
pub mod video;

pub use progress::{noop_progress, ProgressEvent, ProgressFn};

pub use archive::run as run_archive_convert;
pub use audio::run as run_audio_convert;
pub use data::run as run_data_convert;
pub use document::run as run_document_convert;
pub use ebook::run as run_ebook_convert;
pub use email::run as run_email_convert;
pub use font::run as run_font_convert;
pub use image::run as run_image_convert;
pub use model::run as run_model_convert;
pub use notebook::run as run_notebook_convert;
pub use subtitle::run as run_subtitle_convert;
pub use timeline::run as run_timeline_convert;
pub use tracker::run as run_tracker_convert;
pub use video::run as run_video_convert;

/// Build a standard `job-progress` emitter closure for `run()` wrappers that
/// follow the canonical `ProgressEvent → JobProgress → window.emit` pattern.
///
/// The returned closure maps:
/// - `Started`     → `{percent: 0.0, message: starting_msg}`
/// - `Phase(msg)`  → `{percent: 0.0, message: msg}`
/// - `Percent(p)`  → `{percent: p*100 clamped 0–100, message: ""}`
/// - `Done`        → `{percent: 100.0, message: "Done"}`
///
/// Callers that batch Phase+Percent (e.g. `video::run`, `audio::run`) must
/// NOT use this helper — their closure logic cannot be expressed here.
pub fn window_progress_emitter(
    window: &tauri::Window,
    job_id: &str,
    starting_msg: &str,
) -> impl FnMut(ProgressEvent) {
    use tauri::Emitter as _;
    let win = window.clone();
    let job_id_owned = job_id.to_string();
    let starting_msg_owned = starting_msg.to_string();
    move |ev: ProgressEvent| {
        let payload = match ev {
            ProgressEvent::Started => crate::JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: starting_msg_owned.clone(),
            },
            ProgressEvent::Phase(msg) => crate::JobProgress {
                job_id: job_id_owned.clone(),
                percent: 0.0,
                message: msg,
            },
            ProgressEvent::Percent(p) => crate::JobProgress {
                job_id: job_id_owned.clone(),
                percent: (p * 100.0).clamp(0.0, 100.0),
                message: String::new(),
            },
            ProgressEvent::Done => crate::JobProgress {
                job_id: job_id_owned.clone(),
                percent: 100.0,
                message: "Done".to_string(),
            },
        };
        let _ = win.emit("job-progress", payload);
    }
}
