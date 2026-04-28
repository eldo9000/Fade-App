# TASK-17: mkdtemp sandbox for medium-priority temp-file sites

## Goal
Four medium-priority production temp-file sites in `convert/` and `operations/` use `tempfile::TempDir` sandboxes (mode 0700), matching the pattern established in TASK-14 for renderer-facing sites. An attacker on the same machine cannot pre-place a symlink at the predicted output path inside these sandboxes.

## Context
TASK-14 applied per-job mkdtemp sandboxes to the three renderer-facing sites (`chroma_key.rs`, `image_quality.rs`, `video_diff.rs`). Four medium-priority production sites remain using flat `std::env::temp_dir().join(format!("..."))` paths:

1. **`convert/subtitle.rs`** — two sites (~202, ~216):
   - `std::env::temp_dir().join(format!("fade-{job_id}.srt"))` — temp SRT written then passed to ffmpeg as input, or written by ffmpeg then read back. Consumed entirely within the function. Use `TempDir` with RAII cleanup (no `keep()`).

2. **`convert/tracker.rs`** — one site (~214):
   - `std::env::temp_dir().join(format!("fade-tracker-{}.wav", job_id))` — temp WAV written by ffmpeg (render target), then re-used as ffmpeg input for transcoding. Consumed within the function. Use `TempDir` with RAII cleanup.

3. **`operations/analysis/vmaf.rs`** — one site (~73):
   - `std::env::temp_dir().join(format!("fade_vmaf_{}.json", uuid::Uuid::new_v4()))` — JSON log written by libvmaf (passed via ffmpeg `-lavfi libvmaf=log_path=...`), then read back. Consumed within the function. Use `TempDir` with RAII cleanup.

4. **`operations/mod.rs`** — one site in `write_temp_concat_list` (~283):
   - `std::env::temp_dir().join(format!("fade_concat_{}.txt", uuid::Uuid::new_v4()))` — concat list written by Rust (`std::fs::write`), path returned to caller. Because the path is returned (caller holds it while ffmpeg runs), use `keep()` to suppress auto-cleanup, same as TASK-14's renderer-facing pattern.

The `tempfile` crate is already a runtime dependency (added in TASK-14, `src-tauri/Cargo.toml`).

**Important exclusion:** `operations/subtitling/mod.rs` temp paths at the originally-cited lines are inside `#[cfg(test)]` — test-only code. Do NOT touch them.

The pattern for within-function consumption (sites 1–3):
```rust
let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
    .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
let tmp_path = sandbox.path().join("output.srt");  // or .wav, .json
// ... use tmp_path with ffmpeg ...
// sandbox drops here → auto-cleanup
```

The pattern for returned paths (site 4):
```rust
let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
    .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
let path = sandbox.path().join("concat.txt");
// ... write file, then ...
let kept = sandbox.keep().map_err(|e| format!("failed to keep sandbox: {e}"))?;
// return path string; kept.path is the sandbox dir (leaked until OS sweep)
return Ok(path.to_string_lossy().to_string());
```

## In scope
- `src-tauri/src/convert/subtitle.rs` — two temp-path sites
- `src-tauri/src/convert/tracker.rs` — one temp-path site
- `src-tauri/src/operations/analysis/vmaf.rs` — one temp-path site
- `src-tauri/src/operations/mod.rs` — one temp-path site (`write_temp_concat_list`)

## Out of scope
- `operations/subtitling/mod.rs` — cited temp paths are test-only; do NOT touch
- Renderer-facing sites (`chroma_key.rs`, `image_quality.rs`, `video_diff.rs`) — already done in TASK-14
- Adding new tests beyond a mode-check smoke (the pattern is already tested in TASK-14's `sandbox_directory_is_mode_0700`)
- Changing any function signatures beyond what's required for the sandbox pattern
- Any file not listed under In scope

## Steps
1. Read each of the four in-scope files end-to-end. Confirm the exact temp path pattern and whether the path is consumed within the function or returned to the caller.
2. For each within-function site (subtitle.rs ×2, tracker.rs, vmaf.rs): replace the flat `temp_dir().join(...)` construction with a `TempDir::new_in(std::env::temp_dir())` sandbox. Let the TempDir drop naturally at function end for RAII cleanup. The inner filename should be simple and descriptive (`temp.srt`, `render.wav`, `vmaf.json` etc.).
3. For `write_temp_concat_list` in `operations/mod.rs`: replace the flat construction with a `TempDir::new_in` sandbox, write the file inside it, then call `keep()` and return the path as before. The function signature (`-> Result<String, String>`) does not change.
4. For the vmaf.rs site: the log path is interpolated into an ffmpeg filter graph string. After replacing the construction with a TempDir-based path, verify the path is still correctly escaped for the libvmaf filter (the existing `.replace(':', "\\:")` call must still apply to the new path string).
5. `cargo fmt --manifest-path src-tauri/Cargo.toml`
6. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::subtitle convert::tracker operations::analysis operations`
7. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep -r "tempfile::TempDir" src-tauri/src/convert/subtitle.rs src-tauri/src/convert/tracker.rs src-tauri/src/operations/analysis/vmaf.rs src-tauri/src/operations/mod.rs` returns matches in all four files.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All tests pass.

## Notes
- `tempfile::TempDir::keep()` replaced the deprecated `into_path()` (confirmed in TASK-14). Use `keep()` for the returned-path case.
- The vmaf filter graph string embeds the log path. After sandbox migration, the path will contain the TempDir's random suffix (e.g. `/tmp/.tmpXXXXXX/vmaf.json`). The existing colon-escape must still apply to this path — verify it does.
- `convert/tracker.rs`: if `out_ext == "wav"`, the output goes directly to the final output path and the temp WAV is never used. The TempDir should still be created (it drops immediately with no file in it, which is fine — no overhead).
