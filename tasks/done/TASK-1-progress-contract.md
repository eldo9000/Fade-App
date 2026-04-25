# TASK-1: Define the progress + cancellation contract for pure conversions

## Goal
A new module `src-tauri/src/convert/progress.rs` exists, exposes `ProgressEvent`, `ProgressFn`, and `noop_progress()`, and is re-exported from `convert::mod`. Compiles cleanly. No conversion modules use it yet — that's later tasks.

## Context
Fade's conversion pipeline currently couples conversion logic to Tauri's `&Window` for progress emission. Every `convert/<category>::run()` takes `&Window` and calls `window.emit("job-progress", ...)` interleaved with the actual conversion code. This makes the conversion logic unreachable from tests, CLIs, and anything that isn't a Tauri window.

The plan: split each `run()` into a pure `convert()` function plus a thin `run()` wrapper. The pure function takes a callback for progress instead of a Tauri window. This task defines the shared progress contract that every later refactor will use. It must land first because every subsequent task imports from it.

Relevant files:
- `src-tauri/src/convert/mod.rs` — currently lists every submodule and re-exports `pub use <module>::run as run_<category>_convert`. The new `progress` module is added here.
- `src-tauri/src/lib.rs` — does NOT need changes in this task. The new types are not yet used by callers.

The `&Window` is used only for `window.emit("job-progress", payload)` in every module; it has no other use. So a `dyn FnMut(ProgressEvent)` callback can fully replace it.

## In scope
- `src-tauri/src/convert/progress.rs` (new file)
- `src-tauri/src/convert/mod.rs` (one new `pub mod progress;` line + one `pub use progress::*;` line)

## Out of scope
- Any change to `convert/<category>/*.rs` — DO NOT touch any conversion module's `run()` yet
- Any change to `lib.rs`
- Any caller migration
- Writing tests against any conversion module

## Steps
1. Read `src-tauri/src/convert/mod.rs` to see the current module-list pattern.
2. Read 2–3 of the existing modules (`convert/email.rs`, `convert/archive.rs`, `convert/data.rs`) and grep for every `window.emit("job-progress", ...)` call. Note what payload shapes they emit (typically a JSON object with `jobId`, `phase`, `percent`, sometimes more). The `ProgressEvent` enum must be expressive enough to cover what every existing emit call sends.
3. Create `src-tauri/src/convert/progress.rs` containing:
   - `pub enum ProgressEvent` with at least `Started`, `Phase(String)`, `Percent(f32)`, `Done`. Add variants if step 2 turned up emit shapes not covered by these four.
   - `pub type ProgressFn<'a> = &'a mut dyn FnMut(ProgressEvent);` — the type tests and wrappers will pass around.
   - `pub fn noop_progress() -> impl FnMut(ProgressEvent)` — a no-op closure for tests that don't care about progress.
   - Brief module-level doc comment explaining the contract: "Progress emission for conversion functions. The Tauri wrapper builds a closure that translates these events into `window.emit(\"job-progress\", ...)`. Tests use `noop_progress()`."
4. Update `src-tauri/src/convert/mod.rs`:
   - Add `pub mod progress;` alongside the other submodule declarations.
   - Add `pub use progress::{noop_progress, ProgressEvent, ProgressFn};` near the existing `pub use` re-exports.
5. Build: `cargo build --manifest-path src-tauri/Cargo.toml`. Must succeed.
6. Sanity test: `cargo test --manifest-path src-tauri/Cargo.toml --lib`. All existing tests must still pass — this task adds new types without changing any existing behavior.

## Success signal
- `src-tauri/src/convert/progress.rs` exists.
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` exits 0 with the same test count as before this task.
- `grep -c "window.emit" src-tauri/src/convert/*.rs` returns the same total as before this task — no conversion code was touched.

## Notes
- The `ProgressEvent` variants can grow later; pick the smallest set that covers existing emit calls. Better to start small than to over-design.
- `Percent(f32)` should be in `0.0..=1.0` not `0..=100`. State this in a doc comment.
- If you find an emit call with a payload that doesn't fit any of the listed variants, add one variant (e.g. `Custom(serde_json::Value)`) rather than skipping it. Document the variant in the enum doc.
