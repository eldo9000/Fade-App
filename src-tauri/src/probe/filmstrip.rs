use serde::Serialize;
use std::process::Command;
use tauri::{command, Emitter, Window};

/// Hard ceiling on frames per `get_filmstrip` call. Each frame is a separate
/// ffmpeg seek+decode + a base64-JPEG IPC event; unbounded `count` from the
/// frontend can pin the spawn thread and flood the IPC channel. All in-app
/// callers request ≤ 20 today, so this is purely a safety valve.
const FILMSTRIP_MAX_COUNT: usize = 128;

/// Clamp a caller-supplied frame count to [`FILMSTRIP_MAX_COUNT`]. Extracted
/// so the cap stays unit-testable without spinning up a Tauri `Window`.
fn clamp_count(requested: usize) -> usize {
    requested.min(FILMSTRIP_MAX_COUNT)
}

#[derive(Serialize, Clone)]
struct FilmstripFrameEvent {
    id: String,
    index: usize,
    total: usize,
    data: String, // base64 JPEG
}

/// Extract `count` evenly-spaced thumbnail frames from a video.
/// Returns immediately — each frame is emitted as a "filmstrip-frame" event
/// as it finishes, so the UI fills in incrementally without blocking.
/// Each frame is a separate fast-seek ffmpeg call at nice -n 19 / 1 thread.
///
/// `draft` controls per-frame decode width:
/// - `false` (standard): `scale=60` — roughly half the data of a full decode.
/// - `true`  (heavy file / draft): `scale=30` — another 75% pixel reduction.
///
/// Frame count is caller-driven. Current policy keeps 20 frames in both modes;
/// the scale reduction alone delivers the heavy-mode speed-up.
#[command]
pub fn get_filmstrip(
    window: Window,
    path: String,
    id: String,
    count: usize,
    duration: f64,
    draft: bool,
) -> Result<(), String> {
    if count == 0 || duration <= 0.0 {
        return Ok(());
    }
    let count = clamp_count(count);

    let scale_filter = if draft {
        "scale=30:-2:flags=fast_bilinear"
    } else {
        "scale=60:-2:flags=fast_bilinear"
    }
    .to_string();

    std::thread::spawn(move || {
        use base64::Engine as _;

        for i in 0..count {
            // Centre each sample inside its slot
            let ts = format!("{:.3}", (i as f64 + 0.5) * duration / count as f64);

            // -ss before -i = fast keyframe seek; nice -n 19 + 1 thread = truly background
            let output = Command::new("nice")
                .args([
                    "-n",
                    "19",
                    "ffmpeg",
                    "-ss",
                    &ts,
                    "-i",
                    &path,
                    "-frames:v",
                    "1",
                    "-vf",
                    &scale_filter,
                    "-threads",
                    "1",
                    "-f",
                    "image2pipe",
                    "-vcodec",
                    "mjpeg",
                    "-q:v",
                    "7",
                    "-",
                ])
                .output();

            let data = match output {
                Ok(o) if !o.stdout.is_empty() => {
                    base64::engine::general_purpose::STANDARD.encode(&o.stdout)
                }
                _ => continue,
            };

            let _ = window.emit(
                "filmstrip-frame",
                FilmstripFrameEvent {
                    id: id.clone(),
                    index: i,
                    total: count,
                    data,
                },
            );
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_count_passes_through_below_ceiling() {
        assert_eq!(clamp_count(0), 0);
        assert_eq!(clamp_count(20), 20);
        assert_eq!(clamp_count(FILMSTRIP_MAX_COUNT), FILMSTRIP_MAX_COUNT);
    }

    #[test]
    fn clamp_count_bounds_oversized_requests() {
        assert_eq!(clamp_count(FILMSTRIP_MAX_COUNT + 1), FILMSTRIP_MAX_COUNT);
        assert_eq!(clamp_count(10_000), FILMSTRIP_MAX_COUNT);
        assert_eq!(clamp_count(usize::MAX), FILMSTRIP_MAX_COUNT);
    }
}
