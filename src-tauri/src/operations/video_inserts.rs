//! Video insert operation — splice a segment from `insert_video` into
//! `base_video` at the specified time range, producing a seamless output.
//!
//! FFmpeg complex-filter approach:
//!   pre   = base[0:start]
//!   ins   = insert[0:dur]   (dur = insert_end - insert_start)
//!   post  = base[end:]
//!   video output: [pre][ins][post]concat=n=3:v=1:a=0[vout]
//!   audio output: [apre][apost]concat=n=2:v=0:a=1[aout]
//!     or, when keep_insert_audio=true, also splice insert audio.

use crate::operations::run_ffmpeg;
use crate::probe_duration;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Window;

// ── Filter-string builder (exposed for unit tests) ─────────────────────────

/// Build the FFmpeg `-filter_complex` string for a video insert.
///
/// `start` / `end` are the cut-points in `base_video` (seconds).
/// `dur` = `insert_end - insert_start` (seconds of insert to take).
/// `keep_insert_audio`: when `true`, the insert segment's audio is included
/// in the output instead of being silently cut.
pub(crate) fn build_insert_filter(
    start: f64,
    end: f64,
    dur: f64,
    keep_insert_audio: bool,
) -> String {
    if keep_insert_audio {
        // Video: pre + insert + post (concat 3 segments).
        // Audio: pre-audio + insert-audio + post-audio (concat 3 segments).
        format!(
            "[0:v]trim=0:{start:.6},setpts=PTS-STARTPTS[pre];\
             [1:v]trim=0:{dur:.6},setpts=PTS-STARTPTS[ins];\
             [0:v]trim={end:.6},setpts=PTS-STARTPTS[post];\
             [pre][ins][post]concat=n=3:v=1:a=0[vout];\
             [0:a]atrim=0:{start:.6},asetpts=PTS-STARTPTS[apre];\
             [1:a]atrim=0:{dur:.6},asetpts=PTS-STARTPTS[ains];\
             [0:a]atrim={end:.6},asetpts=PTS-STARTPTS[apost];\
             [apre][ains][apost]concat=n=3:v=0:a=1[aout]",
            start = start,
            dur = dur,
            end = end,
        )
    } else {
        // Video: pre + insert + post (concat 3 segments).
        // Audio: pre-audio + post-audio only (concat 2 segments, insert silent).
        format!(
            "[0:v]trim=0:{start:.6},setpts=PTS-STARTPTS[pre];\
             [1:v]trim=0:{dur:.6},setpts=PTS-STARTPTS[ins];\
             [0:v]trim={end:.6},setpts=PTS-STARTPTS[post];\
             [pre][ins][post]concat=n=3:v=1:a=0[vout];\
             [0:a]atrim=0:{start:.6},asetpts=PTS-STARTPTS[apre];\
             [0:a]atrim={end:.6},asetpts=PTS-STARTPTS[apost];\
             [apre][apost]concat=n=2:v=0:a=1[aout]",
            start = start,
            end = end,
            dur = dur,
        )
    }
}

// ── Main operation ─────────────────────────────────────────────────────────

/// Splice `insert_video` into `base_video` at the specified time window
/// `[insert_start, insert_end]` (seconds), writing the result to `output`.
///
/// Audio from the base video around the insert is preserved.
/// `keep_insert_audio`: when `true`, the insert segment's audio track is
/// used for that span instead of silence.
///
/// Emits: job-progress, job-done, job-error, job-cancelled.
#[allow(clippy::too_many_arguments)]
pub fn run_video_insert(
    window: &Window,
    job_id: &str,
    base_video: &str,
    insert_video: &str,
    output: &str,
    insert_start: f64,
    insert_end: f64,
    keep_insert_audio: bool,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    if insert_end <= insert_start {
        return Err(format!(
            "insert_end ({insert_end}) must be greater than insert_start ({insert_start})"
        ));
    }
    let dur = insert_end - insert_start;

    // Probe base video duration for progress reporting (best-effort).
    let duration = probe_duration(base_video);

    let filter = build_insert_filter(insert_start, insert_end, dur, keep_insert_audio);

    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        base_video.to_string(),
        "-i".into(),
        insert_video.to_string(),
        "-filter_complex".into(),
        filter,
        "-map".into(),
        "[vout]".into(),
        "-map".into(),
        "[aout]".into(),
        "-progress".into(),
        "pipe:1".into(),
        output.to_string(),
    ];

    run_ffmpeg(window, job_id, &args, duration, processes, cancelled)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_insert_filter_no_insert_audio_contains_required_segments() {
        let f = build_insert_filter(10.0, 20.0, 10.0, false);
        // Must trim base video into three segments.
        assert!(f.contains("[pre]"), "missing [pre]");
        assert!(f.contains("[ins]"), "missing [ins]");
        assert!(f.contains("[post]"), "missing [post]");
        // Concat 3 video segments.
        assert!(
            f.contains("concat=n=3:v=1:a=0[vout]"),
            "missing vout concat"
        );
        // Audio: only 2 segments (no insert audio).
        assert!(
            f.contains("concat=n=2:v=0:a=1[aout]"),
            "missing aout concat 2"
        );
        assert!(
            !f.contains("[ains]"),
            "must not contain [ains] when keep=false"
        );
    }

    #[test]
    fn build_insert_filter_with_insert_audio_contains_three_audio_segments() {
        let f = build_insert_filter(5.0, 15.0, 10.0, true);
        // Audio: 3 segments including insert audio.
        assert!(f.contains("[ains]"), "missing [ains]");
        assert!(
            f.contains("concat=n=3:v=0:a=1[aout]"),
            "missing aout concat 3"
        );
    }

    #[test]
    fn build_insert_filter_start_and_end_times_embedded() {
        let f = build_insert_filter(3.5, 7.25, 3.75, false);
        // trim=0:3.5 for pre
        assert!(f.contains("trim=0:3.500000"), "start not embedded");
        // atrim=7.25 for post
        assert!(f.contains("atrim=7.250000"), "end not embedded");
    }

    #[test]
    fn build_insert_filter_dur_embedded() {
        let f = build_insert_filter(0.0, 5.0, 5.0, false);
        // trim=0:5.0 for insert video
        assert!(f.contains("trim=0:5.000000"), "dur not embedded");
    }
}
