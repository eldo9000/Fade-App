# TASK-3: Make fast probe commands non-blocking via spawn_blocking

## Goal
Six fast probe IPC commands are converted to `async fn` using `tokio::task::spawn_blocking`, so they no longer block Tauri's IPC thread while executing. Frontend callers are unchanged.

## Context
Tauri 2 routes `#[tauri::command]` functions through its tokio executor. Synchronous commands that call `std::process::Command::output()` block the executor thread for their full duration, preventing other IPC calls from being serviced concurrently.

Six commands are fast (< 500ms, typically < 100ms) — they run a single ffprobe or pure text parsing operation and return immediately. These do not benefit from full job-lifecycle treatment (no cancellation value, no progress). The correct fix is `pub async fn` + `tokio::task::spawn_blocking`, which offloads the blocking work to tokio's blocking thread pool without changing the frontend call contract.

**The 6 commands and their locations:**

| Command | File | Frontend caller |
|---|---|---|
| `get_file_info` | `src-tauri/src/probe/file_info.rs` | `QueueManager.svelte:110, 227` |
| `get_streams` | `src-tauri/src/operations/extract.rs` (note: this is a `#[tauri::command]` inside a non-command file — grep confirms it) | `OperationsPanel.svelte:291` |
| `probe_subtitles` | `src-tauri/src/operations/subtitling/probe.rs` | `OperationsPanel.svelte:652` |
| `diff_subtitle` | `src-tauri/src/operations/subtitling/diff.rs` | `OperationsPanel.svelte:638` |
| `lint_subtitle` | `src-tauri/src/operations/subtitling/lint.rs` | `OperationsPanel.svelte:618` |
| `preview_image_quality` | `src-tauri/src/preview/image_quality.rs` | `App.svelte:389` |

`preview_diff` (`src-tauri/src/preview/video_diff.rs`) is NOT in scope here — it runs a longer FFmpeg comparison and is handled in TASK-5 with the full job-based treatment.

**The migration pattern for each function:**

Before:
```rust
#[tauri::command]
pub fn get_file_info(path: String) -> Result<FileInfo, String> {
    // blocking work
}
```

After:
```rust
#[tauri::command]
pub async fn get_file_info(path: String) -> Result<FileInfo, String> {
    tokio::task::spawn_blocking(move || {
        // same blocking work, verbatim
    })
    .await
    .map_err(|e| e.to_string())?
}
```

All captured variables must be owned (not borrowed) because `spawn_blocking` requires `'static`. Any `&str` params become `String` params (they already are in most cases). Verify each function's param types before writing.

`tokio` is available via Tauri 2's runtime — no Cargo.toml change needed.

## In scope
- `src-tauri/src/probe/file_info.rs`
- `src-tauri/src/operations/extract.rs` (only the `get_streams` function)
- `src-tauri/src/operations/subtitling/probe.rs`
- `src-tauri/src/operations/subtitling/diff.rs`
- `src-tauri/src/operations/subtitling/lint.rs`
- `src-tauri/src/preview/image_quality.rs`

## Out of scope
- Frontend files — callers do not change
- `preview_diff` (`src-tauri/src/preview/video_diff.rs`) — TASK-5
- Any analysis commands (`analyze_*`, `get_waveform`, `get_spectrogram`) — TASK-4 and TASK-5
- `lib.rs` — no changes needed; Tauri's command registry handles async functions transparently

## Steps
1. Read each of the 6 files listed in-scope. For each function, note its parameter types.
2. Add `async` to each function signature.
3. Wrap the existing function body in `tokio::task::spawn_blocking(move || { ... }).await.map_err(|e| e.to_string())?`.
4. Ensure all params captured by the closure are owned types (clone any `&str` params if needed).
5. Run `cargo build --manifest-path src-tauri/Cargo.toml` to confirm compilation.
6. Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` to confirm no new warnings.
7. Run `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
8. Run `cargo test --manifest-path src-tauri/Cargo.toml` to confirm existing tests pass.

## Success signal
`cargo build --release` succeeds. `cargo clippy -D warnings` clean. `cargo test` passes. Running `grep -n "^pub fn get_file_info\|^pub fn get_streams\|^pub fn probe_subtitles\|^pub fn diff_subtitle\|^pub fn lint_subtitle\|^pub fn preview_image_quality" src-tauri/src/**/*.rs` returns no hits (all should now be `pub async fn`).

## Notes
If `spawn_blocking` causes a Tauri command routing issue (rare but possible with certain generic type bounds), the alternative is to mark the function `async` and use `tokio::task::block_in_place` instead. Prefer `spawn_blocking` — it moves work off the current thread entirely.

For `lint_subtitle`, the function body may be pure Rust with no external process (subtitle parsing). If so, `spawn_blocking` is still correct (CPU-bound work belongs in the blocking pool) but the closure won't have FFmpeg concerns.
