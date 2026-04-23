# TASK-2: Remove Err("CANCELLED") sentinels from convert/ modules

## Goal
The `from_result` bridge in `convert_file`'s thread is removed and all 14 `Err("CANCELLED")` sentinels in convert/ modules are replaced with typed cancellation returns via a new `ConvertResult` enum, making the cancellation invariant compiler-enforced on the `convert_file` path.

## Context
B16 phase 1 (`830d105`) added `op_result` as the typed conversion point for the `run_operation` path. The `convert_file` path still uses `from_result` (lib.rs:901) because all convert/ runner functions return `Result<(), String>` and string-return `Err("CANCELLED")` on cancellation.

The `from_result` bridge (`lib.rs:101`) string-matches `"CANCELLED"` to map to `JobOutcome::Cancelled`. This is a leaky boundary. The fix is to introduce `ConvertResult` and have all convert/ runner functions return it instead.

There are 15 `Err("CANCELLED")` sites total:
- 14 in `src-tauri/src/convert/` (the ones this task fixes)
- 1 in `src-tauri/src/operations/mod.rs:267` inside `run_ffmpeg` — **do NOT touch this one**. It feeds the `run_operation` path via `op_result`, which already handles it correctly. Changing `run_ffmpeg`'s return type cascades to 18+ out-of-scope operation files.

The `convert_file` thread shape (lib.rs ~900):
```rust
let mut outcome = JobOutcome::from_result(result.map(|()| Some(output_path.clone())));
```
After the change, match directly on `ConvertResult`:
```rust
let mut outcome = match result {
    ConvertResult::Done => JobOutcome::Done { output_path: output_path.clone() },
    ConvertResult::Cancelled => JobOutcome::Cancelled { remove_path: None },
    ConvertResult::Error(msg) => JobOutcome::Error { message: msg },
};
```
Then set `remove_path` for the Cancelled arm as the existing code does.

Important: some convert/ files may call `operations::run_ffmpeg` (from `src-tauri/src/operations/mod.rs`) which returns `Result<(), String>`. Since we are NOT changing `run_ffmpeg`, those callers receive `Result<(), String>` — handle the `"CANCELLED"` case by mapping it to `ConvertResult::Cancelled` at the call site within the convert/ file.

Existing `from_result` unit tests are in lib.rs around lines 3244–3295. Delete them. The `op_result` tests around line 3310 must be preserved.

## In scope
- `src-tauri/src/lib.rs` — add `ConvertResult`, remove `from_result`, update `convert_file` thread, delete `from_result` unit tests
- All 14 convert/ sentinel files:
  - `src-tauri/src/convert/font.rs`
  - `src-tauri/src/convert/timeline.rs`
  - `src-tauri/src/convert/ebook.rs`
  - `src-tauri/src/convert/model_blender.rs`
  - `src-tauri/src/convert/image.rs`
  - `src-tauri/src/convert/video.rs`
  - `src-tauri/src/convert/audio.rs`
  - `src-tauri/src/convert/notebook.rs`
  - `src-tauri/src/convert/tracker.rs`
  - `src-tauri/src/convert/archive.rs` (5 sentinel sites: lines ~206, 276, 382, 453, 527)
  - `src-tauri/src/convert/model.rs`

## Out of scope
- `src-tauri/src/operations/mod.rs` — do NOT touch; `run_ffmpeg`'s `Err("CANCELLED")` feeds `op_result` on the `run_operation` path; changing it cascades to 18+ other files
- `op_result` function and its tests in lib.rs — do not touch
- Any frontend files
- Any analysis/probe/preview commands (those are TASK-3/4/5)
- Any operations/ submodule files

## Steps
1. Read `src-tauri/src/lib.rs` lines 77–135 (JobOutcome + from_result), lines 895–915 (convert_file thread), lines 3244–3310 (from_result tests + op_result tests boundary).
2. Read all 14 convert/ files listed in-scope in full.
3. In `lib.rs`, add `pub(crate) enum ConvertResult { Done, Cancelled, Error(String) }` near the `JobOutcome` definition.
4. For each convert/ file: change the runner function's return type from `Result<(), String>` to `ConvertResult`. Replace every `return Err("CANCELLED".to_string())` with `return ConvertResult::Cancelled`. Replace every `return Err(msg)` or `return Err(format!(...))` with `return ConvertResult::Error(msg)`. Replace `Ok(())` success returns with `ConvertResult::Done`. If the file calls `operations::run_ffmpeg` and propagates its `Result<(), String>`, add a mapping layer at that callsite.
5. Update the `convert_file` thread in lib.rs to match on `ConvertResult` directly (see Context). Preserve the `remove_path` assignment for `Cancelled`.
6. Delete `from_result` and its unit tests from lib.rs (lines ~3244–3295). Keep `op_result` and its tests untouched.
7. Run `cargo test --manifest-path src-tauri/Cargo.toml`.
8. Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
9. Run `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.

## Success signal
`cargo test` passes. `grep -r "from_result" src-tauri/src/` returns no hits. `grep -rn '"CANCELLED"' src-tauri/src/convert/` returns no hits. One `"CANCELLED"` in `operations/mod.rs:267` remains and is expected. `cargo clippy -D warnings` clean. `cargo fmt --check` clean.

## Notes
Verify the runner function signature for each convert/ file — some take `window: &Window` and `job_id: &str` params in addition to input/output paths. The return type change must match the full signature.

After step 4, `cargo build` will fail until step 5 is complete. That's fine — complete all edits before running build checks.
