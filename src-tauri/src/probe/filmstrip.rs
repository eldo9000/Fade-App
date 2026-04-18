use serde::Serialize;
use std::process::Command;
use tauri::{command, Emitter, Window};

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

    let scale_filter = if draft {
        "scale=80:-2:flags=fast_bilinear"
    } else {
        "scale=160:-2:flags=fast_bilinear"
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
                    "-n", "19",
                    "ffmpeg",
                    "-ss", &ts,
                    "-i", &path,
                    "-frames:v", "1",
                    "-vf", &scale_filter,
                    "-threads", "1",
                    "-f", "image2pipe",
                    "-vcodec", "mjpeg",
                    "-q:v", "7",
                    "-",
                ])
                .output();

            let data = match output {
                Ok(o) if !o.stdout.is_empty() => {
                    base64::engine::general_purpose::STANDARD.encode(&o.stdout)
                },
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
