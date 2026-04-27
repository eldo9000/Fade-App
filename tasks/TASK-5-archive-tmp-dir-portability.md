# TASK-5: Replace hardcoded `/tmp` in archive with `std::env::temp_dir()`

## Goal
`convert/archive.rs` uses the platform-correct temp directory (`std::env::temp_dir()`) for both the extraction staging dir and the tar staging file. The hardcoded `/tmp/fade_archive_*` and `/tmp/fade_tar_stage_*.tar` literals are gone. Behaviour matches every other module that already uses `std::env::temp_dir()` (subtitle, tracker, preview, vmaf).

## Context
Both passes (concern-based C-3 + static M-1) flagged the same issue. Two sites:

- `convert/archive.rs:120` — `let tmp_dir = format!("/tmp/fade_archive_{}", job_id);`
- `convert/archive.rs:447` (`tar_stage`) — same `/tmp/...` pattern

Problems:
- **macOS:** `$TMPDIR` is per-user under `/var/folders/...`. Writing to `/tmp` instead drops files into a globally readable directory shared by every user on the machine. Violates principle of least exposure.
- **Windows future-proofing:** any future Windows port breaks immediately — Windows has no `/tmp`.
- **Inconsistency:** every other module that needs a temp file uses `std::env::temp_dir()`. Examples — `convert/tracker.rs:214`, `convert/subtitle.rs:202,216`, `operations/analysis/vmaf.rs:73`, `preview/image_quality.rs:37`. Archive is the outlier.

The fix is mechanical: swap the literal for `std::env::temp_dir().join(...)`.

Relevant files:
- `src-tauri/src/convert/archive.rs:120` (extract staging dir)
- `src-tauri/src/convert/archive.rs:447` (tar stage file — verify line number against current file)
- `src-tauri/src/convert/tracker.rs:214` (reference pattern)

## In scope
- Replace the two `/tmp/...` literals with `std::env::temp_dir().join(...)` constructions.
- Keep the `format!("fade_archive_{}", job_id)` and `format!("fade_tar_stage_{}.tar", job_id)` shape (only the leading directory changes).
- Add a unit test in `convert/archive.rs` that asserts `tmp_dir.starts_with(std::env::temp_dir())` for both sites — but note the existing helpers may be private; if exposing them just for the test isn't worth it, skip the test and rely on integration-level coverage.

## Out of scope
- The Zip Slip containment fix (TASK-1).
- Renaming the prefix from `fade_archive_*` to anything else.
- Touching other modules that already use `std::env::temp_dir()` correctly.
- Adding `tempfile` crate hardening — that's TASK-10 (covers symlink-attack window across all temp-dir sites).

## Steps
1. Read `convert/archive.rs:115-150` for full context of the extraction-staging path.
2. Replace line 120 with:
   ```rust
   let tmp_dir = std::env::temp_dir()
       .join(format!("fade_archive_{}", job_id))
       .to_string_lossy()
       .into_owned();
   ```
   The `to_string_lossy().into_owned()` is needed because `tmp_dir` is consumed downstream as `&str` (passed to `extract_archive(..., &tmp_dir, ...)`).
3. Read around line 447 for the tar staging site. Apply the same pattern:
   ```rust
   let tar_stage = std::env::temp_dir()
       .join(format!("fade_tar_stage_{}.tar", job_id))
       .to_string_lossy()
       .into_owned();
   ```
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::archive`
6. Manual smoke (optional but recommended on macOS): `tauri dev`, convert a `.zip` → `.tar.gz`, confirm the staging dir appears under `/var/folders/.../T/` (macOS TMPDIR) rather than `/tmp/`.

## Success signal
- `grep '"/tmp/fade' src-tauri/src/convert/archive.rs` returns no matches.
- `grep "std::env::temp_dir" src-tauri/src/convert/archive.rs` returns 2+ matches.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::archive` exits 0.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- This is a one-line change at each site. Low risk, clear win, both audit passes flagged it.
- Don't try to consolidate with TASK-10 (tempfile hardening). They're related but independent: this task fixes *where* temp files live; TASK-10 fixes *how* they're created (O_EXCL for symlink-attack resistance).
- The `into_owned()` boilerplate is unfortunate. If the downstream API can be changed to accept `&Path` instead of `&str`, that's a cleaner refactor — but out of scope here.
