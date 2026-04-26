# TASK-3: Fix tar.gz and tar.xz repack via two-step 7zz approach

## Goal
Converting an archive to `.tar.gz` or `.tar.xz` output succeeds instead of returning a "7z repack failed" error. The fix uses a two-step approach: first create a plain `.tar` from the extracted files, then compress it to the target format, both via `7zz`.

## Context
Fade's `full_sweep.rs` diagnostic (run 2026-04-25) found that all archive-to-tar.gz and archive-to-tar.xz conversions fail. Root cause: modern 7zz rejects single-step creation of `.tar.gz` and `.tar.xz` with `E_INVALIDARG` — it cannot atomically create a compressed tar. The current `repack_with_7z` in `src-tauri/src/convert/archive.rs` builds a single `7zz a out.tar.gz src/*` command, which always fails for these formats.

The fix is a two-step procedure:
1. `7zz a /tmp/<job_id>/stage.tar /src_dir/*` — create an uncompressed tar
2. `7zz a output.tar.gz /tmp/<job_id>/stage.tar` — wrap the tar in gzip compression (or xz for `.tar.xz`)

Both steps go through the existing process-tracking and cancellation machinery. The intermediate `.tar` lives in a per-job temp path and is deleted after step 2.

The routing to `repack_with_7z` happens in `repack_archive` via the `_ =>` catch-all. `ext_of(output_path)` returns `"gz"` for `out.tar.gz` (last segment only). To distinguish `tar.gz` from plain `.gz`, check `output_path.ends_with(".tar.gz")` and `output_path.ends_with(".tar.xz")` before the `ext_of` dispatch — or add the check inside `repack_with_7z` itself.

Relevant files:
- `src-tauri/src/convert/archive.rs` — `repack_with_7z` function (lines ~413–488) and `repack_archive` dispatch (lines ~388–410). This is the only file to change.
- The existing `seven_zip_bin()` helper, `parse_7z_percent()`, and process-tracking pattern are all reusable from within the same file.

## In scope
- `src-tauri/src/convert/archive.rs` — add a two-step repack path for `.tar.gz` and `.tar.xz`. No other files.

## Out of scope
- Any change to `src-tauri/src/lib.rs` or `ConvertOptions`
- Any change to the UI
- Any change to other archive formats (zip, 7z, tar, rar, iso, dmg all work)
- Adding new archive format support beyond fixing the existing broken formats
- Any change to test files

## Steps
1. Read `src-tauri/src/convert/archive.rs` in full — especially `repack_archive`, `repack_with_7z`, `seven_zip_bin()`, `parse_7z_percent()`, and the process-tracking pattern (how processes are registered, how stdout/stderr are consumed, how the child exit status is checked).
2. Add a new private function `repack_tar_compressed` with the same signature as `repack_with_7z` (copy it). It takes `out_ext: &str` in addition (or reads the extension from `output_path`). The function:
   a. Builds a temp path for the intermediate tar: `/tmp/fade_tar_stage_<job_id>.tar`
   b. Step 1: run `7zz a <tmp_tar> <src_dir>/*`. Consume stdout/stderr with the existing pattern. Emit progress 0–50%. On failure, delete the temp tar and return `ConvertResult::Error(...)`. On cancel, delete temp tar and return `ConvertResult::Cancelled`.
   c. Step 2: run `7zz a <output_path> <tmp_tar>`. Consume stdout/stderr. Emit progress 50–100%. Always delete the temp tar after step 2 completes, regardless of success or failure.
   d. Return `ConvertResult::Done` on step-2 success, `ConvertResult::Error(...)` on step-2 failure.
3. In `repack_archive`, before the `_ => repack_with_7z(...)` arm, add detection for tar-compressed formats:
   - If `output_path.ends_with(".tar.gz")` or `output_path.ends_with(".tar.xz")`, call `repack_tar_compressed(...)` instead.
   - Keep the `_ => repack_with_7z(...)` fallback for all other formats.
4. Note: `archive_compression` level (`opts.archive_compression`) applies only to step 2 (the compression step), not step 1 (the tar creation step). Apply the level to step 2 only.
5. `cargo build --manifest-path src-tauri/Cargo.toml` must succeed.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `grep "repack_tar_compressed\|tar\.gz\|tar\.xz" src-tauri/src/convert/archive.rs` returns matches for both the function definition and the dispatch guard.
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.
- No existing tests broken: `cargo test --manifest-path src-tauri/Cargo.toml --lib` exits 0.

## Notes
- Do not attempt to run a live archive conversion test — the sweep fixture infrastructure requires the `#[ignore]` manual suite. Verification via build + clippy + lib tests is sufficient.
- The temp tar path must be unique per job to avoid collisions: `/tmp/fade_tar_stage_<job_id>.tar` where `<job_id>` is the existing `job_id: &str` parameter.
- `7zz a archive.tar.gz archive.tar` works because 7zz detects the `.gz` extension on the outer path and applies gzip compression to whatever is inside — it does not attempt to re-interpret the inner tar's content.
- The cancellation check (`cancelled.load(Ordering::SeqCst)`) should happen between steps 1 and 2, and inside the stdout-reading loop of each step, following the same pattern as `repack_with_7z`.
- `parse_7z_percent()` is already defined in this file — reuse it for both steps.
