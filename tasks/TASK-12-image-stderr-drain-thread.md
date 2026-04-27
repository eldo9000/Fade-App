# TASK-12: Drain ImageMagick stderr in a background thread

## Goal
`convert/image.rs` reads ImageMagick's stderr in a background thread (matching the pattern in `audio.rs`, `video.rs`, `model.rs`). The deadlock window — where ImageMagick fills its stderr pipe buffer and blocks before the parent reads — is closed. The pattern across shell-out modules is uniform.

## Goal — pattern note
This is pattern hygiene, not a fix for an active bug. The deadlock requires ImageMagick to emit > pipe-buffer-size (~64KB on Linux, ~16KB on macOS) of stderr before exiting, which is rare. But the pattern inconsistency invites future regressions.

## Context
Current code at `convert/image.rs:35-53`:

```rust
let stderr = child.stderr.take();

{
    let mut map = processes.lock();
    map.insert(job_id.to_string(), child);
}

// Blocks until process exits or pipe closes after kill
let stderr_content = {
    let mut lines = Vec::new();
    if let Some(s) = stderr {
        let reader = BufReader::new(s);
        for line in reader.lines().map_while(Result::ok) {
            lines.push(line);
        }
    }
    lines.join("\n")
};
```

The comment says "Blocks until process exits or pipe closes after kill" — true for the *parent thread*, which means the parent isn't doing anything else (e.g. reading stdout) while waiting. ImageMagick fills its stderr pipe and blocks on its write. Both processes are now waiting on each other → deadlock until the OS pipe buffer rotates (it doesn't).

In practice, ImageMagick's stderr is small (a few warnings at most). The deadlock is theoretical for typical usage but real for edge cases (deeply malformed input that triggers many warnings, ImageMagick's `-debug all` or `-verbose` modes if ever used, etc.).

The fix: spawn a thread to drain stderr in parallel with whatever the parent is doing. `audio.rs:48-57`, `video.rs:68-77`, `model.rs:81-89` already do this — copy the pattern.

Relevant files:
- `src-tauri/src/convert/image.rs:35-75` — current stderr handling
- `src-tauri/src/convert/video.rs:68-77` — reference pattern
- `src-tauri/src/convert/audio.rs:48-57` — reference pattern (similar shape)

## In scope
- Refactor `convert/image.rs:35-75` to drain stderr in a background thread.
- Match the pattern from `video.rs` exactly (down to variable names where reasonable) for cross-module consistency.
- Handle the join: `stderr_thread.join().unwrap_or_default()` returns the collected lines for error reporting.
- No new tests required — existing image conversion tests verify no regression.

## Out of scope
- Refactoring stdout handling (not a problem here — image module doesn't read stdout for progress, only awaits exit).
- Touching audio/video/model — they're already correct.
- Adding a shared helper for "spawn child + drain stderr" pattern. Could be a follow-up DRY refactor; not in scope here.

## Steps
1. Read `convert/image.rs:1-100` end-to-end.
2. Read `convert/video.rs:60-90` to see the reference drain pattern.
3. Refactor the stderr-handling block (currently lines 44-53) to:
   ```rust
   let stderr_thread = std::thread::spawn(move || {
       let mut lines = Vec::new();
       if let Some(s) = stderr {
           let reader = BufReader::new(s);
           for line in reader.lines().map_while(Result::ok) {
               lines.push(line);
           }
       }
       lines.join("\n")
   });
   ```
4. After the `processes.lock()` insert and before reading the exit status, join the stderr thread:
   ```rust
   let stderr_content = stderr_thread.join().unwrap_or_default();
   ```
5. Confirm the order matches video.rs: insert child into processes map → spawn stderr drain → wait on exit status → join drain → check success.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::image`
8. Manual smoke: `tauri dev`, convert any image (jpg → png), confirm normal completion.
9. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "stderr_thread" src-tauri/src/convert/image.rs` returns 2+ matches.
- Image conversion tests pass.
- Manual smoke confirms normal-case behavior unchanged.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- This is pattern hygiene. No active bug. The CI suite will not catch this issue because the test fixtures don't produce enough stderr to trigger the buffer-fill condition.
- If `convert/image.rs`'s stdout handling is also synchronous — verify it's not consumed (image conversion doesn't track progress via stdout in current architecture). If it *is* consumed, drain that too.
- The pattern across shell-out modules should converge. After this task, every shell-out module in `convert/` follows the same shape: spawn → take stdout/stderr → register in processes → spawn drain thread(s) → wait → join → check success. Worth a brief follow-up to factor a shared helper.
