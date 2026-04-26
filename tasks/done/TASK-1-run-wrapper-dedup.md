# TASK-1: Extract shared window-emit helper to deduplicate run() wrappers

## Goal
A `pub fn window_progress_emitter(window: &Window, job_id: &str, starting_msg: &str) -> impl FnMut(ProgressEvent)` helper exists in `src-tauri/src/convert/mod.rs`, and all `run()` wrappers that follow the standard `ProgressEvent → JobProgress → window.emit("job-progress", ...)` pattern use it instead of inlining the closure.

## Context
The `&Window` decoupling refactor (2026-04-25) split all 15 conversion modules into `pub fn convert(...)` + `pub fn run(...)`. The `run()` wrappers are structurally near-identical: each one builds a `move |ev: ProgressEvent|` closure that maps `ProgressEvent` variants to `JobProgress` structs and emits them on the `Window`, then calls `convert(...)`. This boilerplate is duplicated across 15 files.

The standard pattern (followed by most modules):
```
pub fn run(window, job_id, ...) -> ConvertResult {
    let job_id_owned = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = match ev { Started => ..., Phase(msg) => ..., Percent(p) => ..., Done => ... };
        let _ = win.emit("job-progress", payload);
    };
    convert(input, output, opts, &mut emit, ...)
}
```

`video.rs::run()` uses a different batching pattern (Phase is held and merged with the next Percent event) — it must NOT be changed to use the standard helper.

Relevant files:
- `src-tauri/src/convert/mod.rs` — add the helper here, after the existing `pub use` re-exports
- All `src-tauri/src/convert/*.rs` files with `run()` wrappers — read each one and determine if it follows the standard pattern before editing
- `src-tauri/src/lib.rs` — may need `use crate::convert::window_progress_emitter` if the helper is accessed outside the module, but likely not

The 15 `run()` files:
`audio.rs`, `archive.rs`, `data.rs`, `email.rs`, `ebook.rs`, `font.rs`, `model.rs`, `document.rs`, `model_blender.rs`, `subtitle.rs`, `image.rs`, `video.rs`, `notebook.rs`, `tracker.rs`, `timeline.rs`

## In scope
- `src-tauri/src/convert/mod.rs` — add `window_progress_emitter` helper
- All `run()` wrappers that follow the standard pattern — replace the inline closure with a call to the helper

## Out of scope
- `src-tauri/src/convert/video.rs::run()` — its batching pattern is different; leave it as-is unless it genuinely matches the standard pattern after inspection
- Any change to the `convert()` function signatures
- Any change to `src-tauri/src/lib.rs` dispatch logic
- Any change to test files

## Steps
1. Read `src-tauri/src/convert/mod.rs` to understand the existing structure and imports.
2. Read `src-tauri/src/convert/image.rs::run()` (the cleanest example of the standard pattern) to understand the exact closure structure.
3. Read `src-tauri/src/convert/video.rs::run()` to confirm it is different — do not change it.
4. Add to `src-tauri/src/convert/mod.rs` a helper function:
   ```
   pub fn window_progress_emitter(window: &Window, job_id: &str, starting_msg: &str) -> impl FnMut(ProgressEvent)
   ```
   It captures `window.clone()` and `job_id.to_string()` and returns a closure that maps `ProgressEvent` variants to `JobProgress` and calls `window.emit("job-progress", ...)`. The `starting_msg` is used for the `ProgressEvent::Started` arm (some modules use a label like "Converting email…"). Use `starting_msg` for `Started`, pass-through `Phase` message for `Phase`, use `percent*100.0` for `Percent`, "Done" for `Done`.
   The helper needs `use tauri::{Emitter, Window}` and `use crate::{JobProgress};` plus the `ProgressEvent` import already in scope via `pub use progress::ProgressEvent`.
5. For each of the remaining 13 `run()` files (excluding `video.rs`): read the `run()` body. If it follows the standard pattern, replace the inline closure with `let mut emit = window_progress_emitter(window, job_id, "Converting X…");`. Keep the existing starting-message string from the `Started` arm. If a module's pattern is non-standard (e.g., has extra logic in the closure), leave it unchanged and note it in the return.
6. `cargo build --manifest-path src-tauri/Cargo.toml` must succeed.
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib` must exit 0.
8. `cargo fmt --manifest-path src-tauri/Cargo.toml` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `grep -rn "window_progress_emitter" src-tauri/src/convert/` returns the definition in `mod.rs` plus at least 10 call sites (one per standard wrapper).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` exits 0.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.
- `src-tauri/src/convert/video.rs::run()` is unchanged (the batching pattern stays).

## Notes
- The helper returns `impl FnMut(ProgressEvent)` — the caller stores it as `let mut emit = ...` and passes `&mut emit` to `convert(...)`, exactly as before.
- Some `run()` wrappers (e.g. `email.rs`) return `Result<(), String>` not `ConvertResult` — the helper still applies; the caller just wraps the call differently. Adapt accordingly.
- If a module's `Started` arm has no message (returns an empty `JobProgress`), pass `""` as `starting_msg`.
- `video.rs` batches Phase+Percent into a single emit — that logic cannot be expressed with the standard helper and should be left alone.
