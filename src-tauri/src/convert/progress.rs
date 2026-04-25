//! Progress emission for conversion functions.
//!
//! Pure conversion functions (the future `convert(...)` split out of each
//! `run(...)`) accept a [`ProgressFn`] callback rather than a Tauri `&Window`.
//! The Tauri wrapper builds a closure that translates these events into
//! `job-progress` emits on the active window (and friends). Tests use
//! [`noop_progress`] when they don't care about progress.
//!
//! This module is intentionally minimal — variants can grow as new emit
//! shapes appear. Today, every conversion module emits `job-progress`
//! payloads consisting of a percent and a human-readable message, plus the
//! `job-done` terminal event in `archive`. Those map cleanly onto the
//! variants below.

/// A single progress signal emitted by a pure conversion function.
///
/// `Percent` is normalized to `0.0..=1.0` (not `0..=100`); the Tauri wrapper
/// is responsible for scaling it to whatever the frontend expects.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// The conversion has begun. Wrappers typically translate this into a
    /// `percent: 0.0` `job-progress` emit.
    Started,
    /// A human-readable phase label, e.g. "Extracting…", "Repacking…",
    /// "Converting data…". Carries the same semantics as the `message`
    /// field in today's `JobProgress` payload.
    Phase(String),
    /// Fractional progress in `0.0..=1.0`.
    Percent(f32),
    /// The conversion has finished successfully. Wrappers typically
    /// translate this into a `percent: 1.0` `job-progress` emit (and may
    /// also emit `job-done` separately, which is not part of this contract).
    Done,
}

/// The callback type that pure conversion functions accept.
///
/// A mutable trait object so callers can hold state (e.g. a job id, a
/// `Window`, or a `Vec<ProgressEvent>` for tests) without leaking those
/// concerns into the conversion code.
pub type ProgressFn<'a> = &'a mut dyn FnMut(ProgressEvent);

/// A progress callback that discards every event. Useful in tests that
/// exercise a conversion's correctness but don't care about progress.
pub fn noop_progress() -> impl FnMut(ProgressEvent) {
    |_event: ProgressEvent| {}
}
