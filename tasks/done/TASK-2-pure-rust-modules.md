# TASK-2: Refactor the pure-Rust conversion modules (email, subtitle, document)

## Goal
`convert::email`, `convert::subtitle`, and `convert::document` each expose a new `pub fn convert(...)` that does the actual conversion without taking a Tauri `&Window`. The existing `pub fn run(...)` is reduced to a thin wrapper that builds a progress closure and delegates to `convert()`. A new test file `src-tauri/tests/refactored_pure_sweep.rs` calls each `convert()` directly and verifies it produces output. Existing behavior is unchanged from a Tauri caller's perspective.

## Context
This is the second task in the `&Window` decoupling arc. The progress contract is already in place from TASK-1: `convert::progress::{ProgressEvent, ProgressFn, noop_progress}`. This task is the first to actually use it and establishes the refactor pattern that every later task copies.

These three modules are picked first because they are pure-Rust (or mostly pure-Rust) — no external CLI shell-out — so the refactor is mechanical. Later tasks tackle modules that spawn external processes (calibre, pandoc, assimp, ffmpeg, 7z), which will reuse this same pattern but layer in process management.

The pattern, applied to each module:

```rust
// New: pure conversion. Tests and any future non-Tauri caller use this.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    progress(ProgressEvent::Started);
    // ... existing conversion logic, with every `window.emit(...)` replaced
    //     by `progress(ProgressEvent::...)` ...
    progress(ProgressEvent::Done);
    Ok(())
}

// Reduced: thin wrapper. Existing dispatcher in lib.rs keeps calling this.
pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let _ = processes; // unused for pure-Rust modules; keep arg for signature stability
    let job_id = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = serde_json::json!({
            "jobId": job_id,
            // map ev fields into the existing payload shape this module already used
        });
        let _ = win.emit("job-progress", payload);
    };
    convert(input, output, opts, &mut emit, &cancelled)
}
```

The exact JSON payload shape **must match what the module currently emits** so the frontend's progress UI keeps working. Don't invent new payload fields; replicate the existing ones.

Relevant files:
- `src-tauri/src/convert/email.rs` — eml ↔ mbox; pure Rust; helpers `eml_to_mbox` and `mbox_to_eml` are already `pub`.
- `src-tauri/src/convert/subtitle.rs` — srt ↔ sbv pure Rust + ffmpeg path for vtt/ass/ssa; helpers `srt_to_sbv` and `sbv_to_srt` are already `pub`. The ffmpeg path stays in `convert()` and shells out exactly as it does today.
- `src-tauri/src/convert/document.rs` — md/html/txt pure Rust + pandoc path for docx/pdf; helpers `strip_md`, `html_to_text`, `html_to_md` are already `pub`. Pandoc path stays in `convert()`.
- `src-tauri/src/convert/progress.rs` — defined in TASK-1; import from here.
- `src-tauri/tests/extra_sweep.rs` — already uses the public helper fns. Do NOT delete or move it; this task adds a separate file.

## In scope
- `src-tauri/src/convert/email.rs` — extract `convert()`, reduce `run()` to wrapper
- `src-tauri/src/convert/subtitle.rs` — same
- `src-tauri/src/convert/document.rs` — same
- `src-tauri/tests/refactored_pure_sweep.rs` (new file) — a sweep test calling each module's new `convert()` with `noop_progress()`, with at least 2 cases per module

## Out of scope
- Any other convert/*.rs module
- `src-tauri/src/lib.rs` — the dispatcher still calls `run()`; don't change it
- The existing `extra_sweep.rs` file — leave it alone
- Any change to `ConvertResult` type or the `run()` signature
- Any progress payload shape change visible to the frontend

## Steps
1. Open `src-tauri/src/convert/email.rs`. Identify the body of `run()`. Find every `window.emit("job-progress", payload)` call and note the payload shape. Extract everything except those emits into a new `pub fn convert(input, output, opts, progress, cancelled) -> Result<(), String>`. Replace each emit with `progress(ProgressEvent::...)` using the variant that matches what was being emitted.
2. Reduce `email::run()` to a wrapper: build a `move |ev|` closure that re-emits `window.emit("job-progress", ...)` with the same JSON shape that was there before, then call `convert()`. The closure must produce the exact same payload the module emitted before — frontend depends on it.
3. Build: `cargo build --manifest-path src-tauri/Cargo.toml`. Must succeed.
4. Repeat steps 1–3 for `subtitle.rs`. Note: subtitle has both pure-Rust paths (srt↔sbv) and an ffmpeg shell-out path. Keep both inside `convert()`. The ffmpeg path uses `processes` for cancellation — pass `cancelled` through; for `processes`, decide: either thread it through `convert()` as an extra arg, or keep process registration inside `run()` and have `convert()` take a `&dyn ProcessRegistrar` trait. Pick the simpler one; if unsure, thread `processes` through `convert()` directly.
5. Repeat steps 1–3 for `document.rs`. Same notes as subtitle re: pandoc shell-out.
6. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. All must pass.
7. Create `src-tauri/tests/refactored_pure_sweep.rs`. Pattern: import `fade_lib::convert::{email, subtitle, document, noop_progress}`. For each module, call `convert()` with synthesized inputs (use the same `SAMPLE_EML`, `SAMPLE_SRT`, `SAMPLE_MD` constants from `extra_sweep.rs` — copy them, do NOT cross-import test files). Assert output file exists and is non-empty. Use `noop_progress()` for the progress callback. At minimum: 2 cases per module (one happy path, one different format).
8. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_pure_sweep --no-run`. Must succeed.
9. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_pure_sweep -- --nocapture`. All cases must pass.

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0 with all existing tests passing.
- `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_pure_sweep -- --nocapture` exits 0 with at least 6 passing cases (2 per module × 3 modules).
- `grep -c "window.emit" src-tauri/src/convert/email.rs src-tauri/src/convert/subtitle.rs src-tauri/src/convert/document.rs` shows each file has at most one `window.emit` site (inside the `run()` wrapper closure), down from however many it had before.

## Notes
- The hardest call is how to handle `processes: Arc<Mutex<HashMap<String, Child>>>` in modules that spawn child processes. The simplest path is to add it as an arg to `convert()` alongside `cancelled`. Yes, it's a Tauri-adjacent type, but it's a `std::sync` + `std::process` type, not a `tauri::*` type — pure Rust can construct one in a test (`Arc::new(Mutex::new(HashMap::new()))`). Document this choice in a comment in `progress.rs` so later tasks follow the same convention.
- Keep `run()` ≤ 20 lines after the refactor. If it's longer, you've left logic in the wrapper that should be in `convert()`.
- DO NOT change the progress payload shape. Frontend code reads specific JSON keys; changing them breaks the UI silently.
- If a module currently has no `window.emit` calls at all (some pure-Rust paths might not emit progress), `run()` becomes effectively `convert(...)` with the unused window arg discarded. That's fine.
