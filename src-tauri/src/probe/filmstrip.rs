use crate::validate_input_path;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{command, Emitter, State, Window};

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

/// Per-filmstrip cancel flags, keyed by the caller's `id`.
///
/// Before this registry existed, `get_filmstrip` spawned a detached thread
/// that looped through N ffmpeg frame extractions with no cancel hook — if the
/// user removed the item or swapped to a different one mid-load, the old
/// thread kept spawning ffmpegs and emitting `filmstrip-frame` events for a
/// stale id. The frontend filters them out, but the CPU is still spent.
///
/// On re-entry for the same id (rapid item switch), the old flag is flipped
/// to `true` so the previous thread exits on its next iteration; the new
/// thread owns the slot. `cancel_filmstrip(id)` flips the flag from the
/// frontend on item removal.
#[derive(Default)]
pub struct FilmstripCancels(pub Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);

/// Install `cancelled` as the cancel flag for `id`, flipping any predecessor
/// flag so a lingering thread exits on its next iteration. Exposed for
/// testing; the production path goes through `register_cancel` inside
/// `get_filmstrip`.
fn register_cancel(
    map: &Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    id: &str,
    cancelled: &Arc<AtomicBool>,
) {
    let mut guard = map.lock();
    if let Some(old) = guard.insert(id.to_string(), Arc::clone(cancelled)) {
        old.store(true, Ordering::SeqCst);
    }
}

/// Remove the cancel flag for `id`, but only if it's still the one we own
/// (`Arc::ptr_eq`). If a newer thread has already replaced it, leave theirs
/// in place — our cleanup must not clear a successor's flag.
fn clear_cancel_if_owned(
    map: &Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    id: &str,
    owned: &Arc<AtomicBool>,
) {
    let mut guard = map.lock();
    if let Some(current) = guard.get(id) {
        if Arc::ptr_eq(current, owned) {
            guard.remove(id);
        }
    }
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
/// - `false` (standard): `scale=173` — good thumbnail fidelity.
/// - `true`  (heavy file / draft): `scale=86` — half of standard, still readable.
///
/// Frame count is caller-driven. Current policy keeps 20 frames in both modes;
/// the scale reduction alone delivers the heavy-mode speed-up.
#[command]
pub fn get_filmstrip(
    window: Window,
    state: State<'_, FilmstripCancels>,
    path: String,
    id: String,
    count: usize,
    duration: f64,
    draft: bool,
) -> Result<(), String> {
    validate_input_path(&path)?;
    if count == 0 || duration <= 0.0 {
        return Ok(());
    }
    let count = clamp_count(count);

    let scale_filter = if draft {
        "scale=86:-2:flags=fast_bilinear"
    } else {
        "scale=173:-2:flags=fast_bilinear"
    }
    .to_string();

    let cancelled = Arc::new(AtomicBool::new(false));
    register_cancel(&state.0, &id, &cancelled);
    let cancels = Arc::clone(&state.0);

    std::thread::spawn(move || {
        use base64::Engine as _;

        for i in 0..count {
            if cancelled.load(Ordering::SeqCst) {
                break;
            }
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

            // Re-check after the blocking spawn returns: the user may have
            // cancelled while this frame was decoding. Drop the frame rather
            // than emitting a stale event for an already-removed item.
            if cancelled.load(Ordering::SeqCst) {
                break;
            }

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

        clear_cancel_if_owned(&cancels, &id, &cancelled);
    });

    Ok(())
}

/// Signal any in-flight `get_filmstrip` for `id` to stop at the next
/// iteration boundary. No-op if no filmstrip is registered for this id.
/// Called from the frontend when a queue item is removed or the user
/// navigates away so stale ffmpeg invocations don't pile up.
#[command]
pub fn cancel_filmstrip(state: State<'_, FilmstripCancels>, id: String) -> Result<(), String> {
    let guard = state.0.lock();
    if let Some(flag) = guard.get(&id) {
        flag.store(true, Ordering::SeqCst);
    }
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

    #[test]
    fn register_cancel_inserts_flag_for_new_id() {
        let map: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let flag = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &flag);
        assert!(Arc::ptr_eq(map.lock().get("item-a").unwrap(), &flag));
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn register_cancel_flips_predecessor_flag_on_reentry() {
        // Rapid item switch: a second filmstrip for the same id should
        // cause the previous thread to exit on its next iteration.
        let map: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let old = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &old);

        let new = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &new);

        assert!(
            old.load(Ordering::SeqCst),
            "predecessor flag must be flipped so the old thread exits"
        );
        assert!(
            !new.load(Ordering::SeqCst),
            "the successor flag must stay clean"
        );
        assert!(Arc::ptr_eq(map.lock().get("item-a").unwrap(), &new));
    }

    #[test]
    fn clear_cancel_removes_only_owned_flag() {
        let map: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Register a flag, then replace it, then have the original thread
        // try to clean up. The successor's entry must survive.
        let first = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &first);
        let second = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &second);

        clear_cancel_if_owned(&map, "item-a", &first);

        let guard = map.lock();
        assert!(
            guard.contains_key("item-a"),
            "successor's flag must not be cleared by the predecessor's exit"
        );
        assert!(Arc::ptr_eq(guard.get("item-a").unwrap(), &second));
    }

    #[test]
    fn clear_cancel_removes_own_flag_on_normal_exit() {
        let map: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let flag = Arc::new(AtomicBool::new(false));
        register_cancel(&map, "item-a", &flag);
        clear_cancel_if_owned(&map, "item-a", &flag);
        assert!(map.lock().get("item-a").is_none());
    }
}
