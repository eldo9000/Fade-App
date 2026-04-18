//! Read-only probes of media files — metadata, waveform, spectrogram, filmstrip.
//!
//! None of these commands produce output files; they return data (or emit events)
//! used by the UI for preview and timeline rendering.

pub mod file_info;
pub mod filmstrip;
pub mod spectrogram;
pub mod waveform;

pub use file_info::get_file_info;
pub use filmstrip::get_filmstrip;
pub use spectrogram::get_spectrogram;
pub use waveform::get_waveform;
