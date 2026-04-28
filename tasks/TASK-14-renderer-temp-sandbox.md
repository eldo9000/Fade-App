# TASK-14: Per-job mkdtemp sandbox for renderer-facing temp files

## Goal
The three renderer-facing temp-file sites (`operations/chroma_key.rs`, `preview/image_quality.rs`, `preview/video_diff.rs`) write into per-call sandbox directories created with mode 0700. An attacker on the same machine cannot pre-place a symlink at the predicted output path because the sandbox directory is created atomically before the path is computed, and only the running user can write into it. The originally-attempted `OpenOptions::create_new(true)` mitigation (TASK-10) doesn't apply here because the file is opened by the spawned `ffmpeg`/`magick` subprocess, not by Rust — but a directory whose permissions defeat pre-placement does work, regardless of who opens the file inside it.

## Context
TASK-10 failed because its specified mitigation (`create_new(true)`) only protects opens performed by Rust. The renderer-facing sites pass paths to ffmpeg/magick subprocesses, which open the file themselves with `O_TRUNC` (ffmpeg `-y`) — and `create_new(true)` cannot be applied to a path that we don't open.

The actual exposure (from static analysis):
1. Attacker predicts the next UUID-suffixed temp path (via `inotify`/`FSEvents` or by enumerating).
2. Attacker pre-places a symlink at that path pointing at `~/.ssh/known_hosts` or similar.
3. Fade spawns ffmpeg/magick with the predicted path as the output target.
4. ffmpeg `-y` truncates the symlink target → arbitrary file overwrite as the running user.

The robust mitigation is to write the file inside a directory the attacker cannot write to. `mkdtemp`-style atomic directory creation with mode 0700 achieves this:
- The directory is created in one syscall with mode 0700 (only the owner can write/list).
- The directory name is unguessable until after creation (random suffix).
- ffmpeg/magick still open the file with `O_TRUNC`, but the attacker cannot place a symlink inside the dir because they don't have write permission to it.

The cleanest Rust API is the `tempfile` crate (`TempDir::new` / `TempDir::new_in`). For renderer-facing sites where the output path must outlive the function (returned to the JS frontend for display), use `TempDir::into_path()` to suppress auto-cleanup; the cleanup happens later (next preview, app shutdown, OS sweep).

Affected sites (renderer-facing only — same set as TASK-10):
- `operations/chroma_key.rs:~301` — chroma key preview output (returned to renderer)
- `preview/image_quality.rs:~48, ~65` — compressed and diff outputs (returned to renderer)
- `preview/video_diff.rs:~97` — video diff output (returned to renderer)

Note: TASK-10 worker confirmed there are no Rust-side `File::create` or `fs::write` calls at these sites. The path is built in Rust, then passed to a spawned subprocess. This task addresses the directory the path lives in, not the open syscall.

Relevant files:
- `src-tauri/src/operations/chroma_key.rs` — chroma key preview generator
- `src-tauri/src/preview/image_quality.rs` — image quality preview pipeline
- `src-tauri/src/preview/video_diff.rs` — video diff preview pipeline
- `src-tauri/Cargo.toml` — add `tempfile = "3"` if not already present

## In scope
- Add the `tempfile` crate as a dependency in `src-tauri/Cargo.toml` (latest 3.x).
- For each of the three high-priority sites:
  1. Replace the `std::env::temp_dir().join(format!("fade-...-{uuid}.{ext}"))` pattern with a `tempfile::TempDir::new_in(std::env::temp_dir())` followed by `.path().join("output.{ext}")` (or similar fixed inner filename).
  2. For paths returned to the renderer: call `.into_path()` on the `TempDir` to disable auto-cleanup; the path now points inside a 0700 directory.
  3. For paths fully consumed within the function: keep the `TempDir` handle alive until cleanup is wanted (RAII cleanup on Drop).
- Tests:
  1. Sandbox directory exists and is mode 0700 after creation (Unix).
  2. A pre-placed symlink at the predicted file path inside the sandbox cannot be placed by another user (skip — would require multi-user test infra; document the threat model in the test instead).
  3. Smoke: each migrated site still produces a valid output file in the new location.

## Out of scope
- Migrating the medium-priority sites (subtitle.rs, tracker.rs, vmaf.rs, mod.rs, subtitling/mod.rs). Defer to follow-up sweep — they are not renderer-facing.
- Cleanup hygiene for `into_path()` paths (sites that return to renderer leak directories until OS sweep). A separate task could add a job-scoped cleanup map.
- Migrating to `O_TMPFILE` + `linkat` (Linux-only, more complex; tempfile crate is portable).
- Refactoring the prefix conventions or moving sites between modules.
- Touching the actual ffmpeg/magick invocation arguments.

## Steps
1. Read `Cargo.toml` to check whether `tempfile` is already a dev-dep or runtime dep. If only dev-dep, promote to runtime; if absent, add `tempfile = "3"` under `[dependencies]`.
2. Read each of the three high-priority sites end-to-end. For each, identify:
   - Where the output path is constructed.
   - Whether it's consumed within the function or returned to the renderer.
3. Replace the path construction at each site with the tempfile pattern. For renderer-returned paths:
   ```rust
   let sandbox = tempfile::TempDir::new_in(std::env::temp_dir())
       .map_err(|e| format!("failed to create temp sandbox: {e}"))?;
   let output_path = sandbox.path().join("output.png");
   // ... pass &output_path to ffmpeg/magick ...
   let kept_path = sandbox.into_path().join("output.png");
   // return kept_path or its string form to caller
   ```
   Adjust the inner filename per site (`output.png`, `diff.png`, `compressed.jpg`, etc.).
4. For each site, verify that the path returned to the renderer is the post-`into_path` version, not the original `sandbox.path()` (which becomes invalid after the function returns).
5. Add a Unix-gated test in one of the three modules:
   ```rust
   #[test]
   #[cfg(unix)]
   fn sandbox_directory_is_mode_0700() {
       use std::os::unix::fs::PermissionsExt;
       let dir = tempfile::TempDir::new_in(std::env::temp_dir()).unwrap();
       let perms = std::fs::metadata(dir.path()).unwrap().permissions();
       // mkdtemp on Unix sets 0700; tempfile crate honours this.
       assert_eq!(perms.mode() & 0o777, 0o700, "sandbox dir mode is not 0700");
   }
   ```
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib operations::chroma_key preview::image_quality preview::video_diff`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep -r "tempfile::TempDir" src-tauri/src/operations/chroma_key.rs src-tauri/src/preview/image_quality.rs src-tauri/src/preview/video_diff.rs` returns matches at all three sites.
- `grep "tempfile" src-tauri/Cargo.toml` shows it under `[dependencies]` (not just `[dev-dependencies]`).
- The new mode-check test passes.
- All existing tests pass.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- This task replaces TASK-10, which was rejected because its specified mitigation (`OpenOptions::create_new(true)`) doesn't apply to subprocess opens. See `tasks/done/TASK-10-tempfile-symlink-hardening.md` for the rejection note.
- `tempfile` is well-maintained, zero-cost, single-purpose. The original TASK-10 forbade it to keep diff small; that constraint was wrong given the actual code shape.
- `into_path()` defeats the auto-cleanup-on-Drop benefit. Renderer-facing sites accept this trade-off — the path needs to outlive the IPC handler that returned it. A follow-up could track these paths in app state for explicit cleanup, but that's a separate concern.
- macOS `$TMPDIR` is already per-user under `/var/folders/.../T/`, which is mode 0700. The mkdtemp inside it is belt-and-braces. On Linux with `/tmp` mounted shared (default), the mkdtemp is the primary defence.
- Threat model: the attacker needs local code execution as the same user (macOS) or a different user on the same machine (Linux multi-user). Realistic on shared installs; negligible on single-user laptops.
- Medium-priority sites (`convert/subtitle.rs`, `convert/tracker.rs`, `operations/analysis/vmaf.rs`, `operations/mod.rs`, `operations/subtitling/mod.rs`) should get the same treatment in a follow-up sweep, but they're not renderer-facing so the exposure is lower.
