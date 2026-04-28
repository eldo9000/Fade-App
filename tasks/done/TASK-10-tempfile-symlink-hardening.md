# TASK-10: Harden temp-file creation against symlink-pre-placement attacks

> **STATUS: REJECTED 2026-04-28 — false positive on mitigation.**
> The static-analysis report cited `File::create`/`std::fs::write` at six renderer-facing sites. The TASK-10 worker verified each line and found no Rust-side file creation at any of them — file creation is performed by spawned `ffmpeg`/`magick` subprocesses, not by Rust. `OpenOptions::create_new(true)` cannot mitigate a write performed by an external CLI tool that the kernel opens with its own flags (and `ffmpeg -y` truncates through symlinks regardless).
>
> The underlying exposure is real (attacker pre-places symlink at predicted UUID path, ffmpeg/magick truncates the symlink target), but the mitigation specified here doesn't fit the sink shape. Follow-up: **TASK-14** uses per-job mkdtemp sandboxes (mode 0700) instead, which protects opens regardless of who performs them.

## Goal
The renderer-facing temp-file creation sites use `OpenOptions::create_new(true)` (or `tempfile::NamedTempFile`), making symlink-pre-placement attacks impossible. The most exposed sites — `operations/chroma_key.rs` (preview path returned to renderer), `preview/image_quality.rs`, `preview/video_diff.rs` — are migrated first; lower-exposure sites in convert modules are migrated as a sweep.

## Context
Static analysis flagged 15+ sites across 11 files that create temp files via `std::env::temp_dir().join(...)` followed by `std::fs::File::create(...)` (or `std::fs::write`). `File::create` opens with `O_TRUNC` semantics, which **follows** an attacker-pre-placed symlink and overwrites the target. Combined with the predictable-ish prefixes (`fade-tracker-*`, `fade-image-*`), an attacker on the same machine could:

1. Predict the next temp path.
2. Pre-place a symlink at that path pointing at a sensitive file (e.g. `~/.ssh/known_hosts`).
3. Wait for Fade to write to its predicted temp file → overwrites the target.

UUIDs in the suffix mitigate predictability but not pre-placement (an attacker can race with `inotify`/`FSEvents`). The robust fix is `O_CREAT | O_EXCL`-equivalent: the `OpenOptions::create_new(true)` Rust API. If the path exists (file or symlink), the open fails. No symlink to follow.

The `tempfile` crate (`NamedTempFile::new_in`, `tempfile::tempdir`) wraps this pattern with auto-cleanup. Recommended for new code; for existing sites, the smaller-diff fix is to swap `File::create` → `OpenOptions::new().write(true).create_new(true).open(...)`.

Affected sites (from static analysis report):
- `lib.rs:511` (note: this is `validate_output_dir`'s reference to TMPDIR — not a sink, skip)
- `lib.rs:3195, 3302` (test-only sinks; medium priority)
- `fs_commands.rs:72, 100, 142` (test-only; low priority)
- `convert/subtitle.rs:202, 216` (sink; medium)
- `convert/tracker.rs:214` (sink; medium)
- `operations/analysis/vmaf.rs:73` (sink; medium)
- **`operations/chroma_key.rs:299`** (renderer-facing preview path; HIGH priority)
- `operations/mod.rs:283` (sink; medium)
- `operations/subtitling/mod.rs:59, 93` (sink; medium)
- **`preview/image_quality.rs:37, 107, 125, 171`** (renderer-facing previews; HIGH priority)
- **`preview/video_diff.rs:69`** (renderer-facing preview; HIGH priority)

Renderer-facing sites take priority because their output paths are returned to the frontend and sometimes displayed, making them the most likely race target.

## In scope
- Migrate the three high-priority renderer-facing sites first (`operations/chroma_key.rs`, `preview/image_quality.rs`, `preview/video_diff.rs`) to use `OpenOptions::new().write(true).create_new(true).open(path)` instead of `std::fs::File::create(path)`.
- Where the file is written via `std::fs::write(path, data)` (no explicit File handle), use:
  ```rust
  std::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&path)?
      .write_all(&data)?;
  ```
- Tests that confirm the new behaviour:
  1. Pre-place a symlink at the predicted path → file creation fails with the expected error.
  2. Normal case (no pre-existing path) → file creation succeeds.

## Out of scope
- Migrating the medium- and low-priority sites in this task. Track them as a follow-up sweep.
- Adding the `tempfile` crate as a dependency. Use the std-only `OpenOptions` approach.
- Refactoring the prefix conventions or moving sites between modules.
- Touching test-only sinks (`fs_commands.rs:72,100,142`).

## Steps
1. Read each high-priority site to confirm the current write pattern (`File::create` vs `std::fs::write`).
2. For each site, replace the create call:
   - `std::fs::File::create(&path)` → `std::fs::OpenOptions::new().write(true).create_new(true).open(&path)`
   - `std::fs::write(&path, &data)` → the OpenOptions+write_all pattern shown above.
3. The error mode changes: `create_new(true)` returns `ErrorKind::AlreadyExists` if the path is occupied. Add an explanatory error message:
   ```rust
   .map_err(|e| match e.kind() {
       std::io::ErrorKind::AlreadyExists => format!(
           "temp file already exists (possible symlink attack): {}",
           path.display()
       ),
       _ => format!("failed to create temp file: {e}"),
   })?
   ```
4. Add a test in each migrated module:
   ```rust
   #[test]
   #[cfg(unix)]
   fn temp_file_creation_rejects_pre_placed_symlink() {
       let dir = std::env::temp_dir();
       let path = dir.join(format!("fade-test-{}", uuid::Uuid::new_v4()));
       let target = dir.join(format!("fade-test-target-{}", uuid::Uuid::new_v4()));
       std::fs::write(&target, b"original").unwrap();
       std::os::unix::fs::symlink(&target, &path).unwrap();
       let result = std::fs::OpenOptions::new()
           .write(true).create_new(true).open(&path);
       assert!(result.is_err());
       assert_eq!(std::fs::read(&target).unwrap(), b"original");
       std::fs::remove_file(&path).ok();
       std::fs::remove_file(&target).ok();
   }
   ```
5. Verify no behaviour regression — the migrated sites previously worked; `create_new(true)` only adds a fail-mode for pre-existing paths. UUIDs in the prefix make collisions vanishingly unlikely.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib operations::chroma_key preview::image_quality preview::video_diff`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "create_new(true)" src-tauri/src/operations/chroma_key.rs src-tauri/src/preview/image_quality.rs src-tauri/src/preview/video_diff.rs` returns matches at the migrated sites.
- The three new symlink-rejection tests pass.
- All existing tests pass.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- This task only covers the renderer-facing top tier. After this lands, file a follow-up to sweep the medium-priority convert/operations sites the same way.
- `create_new(true)` is the std-library equivalent of `O_CREAT | O_EXCL`. It's atomic at the filesystem level — no race window.
- The `tempfile` crate would be a cleaner long-term solution (auto-cleanup via `Drop`), but adding a dependency for this single purpose is overkill. The std-only approach matches the codebase's existing temp-file style.
- Threat model: the attacker needs local code execution as a different user on the same machine, plus the ability to predict the UUID. Realistic on shared macOS/Linux installs (universities, dev shops). Not a concern on single-user laptops.
