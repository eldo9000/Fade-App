# TASK-8: Replace sleep-based concat-list cleanup with deterministic guard

## Goal
The merge operation's concat-list temp file is deleted after `run_ffmpeg` returns, not after a 2-second sleep. The race window — where ffmpeg cold-starts slowly and the file disappears before it's opened — is closed. The cleanup happens in both fast-path and slow-path branches.

## Context
Current code at `src-tauri/src/operations/merge.rs:125-130`:

```rust
// Schedule cleanup; errors are non-fatal
std::thread::spawn(move || {
    // Wait a moment for FFmpeg to open the file before deleting
    std::thread::sleep(std::time::Duration::from_secs(2));
    let _ = std::fs::remove_file(&list_path);
});
```

Problems:
- **Linux/macOS:** holds inode after open, so a 2s race is "usually safe" — but if ffmpeg cold-starts (cold-disk seek, large input metadata probe, system under load), the file may be deleted before ffmpeg reads it. ffmpeg fails with confusing "file not found" error mid-merge.
- **Windows:** filesystems don't allow deletion of an open file. Either the delete fails (file leak) or the open fails (merge fails). Either way, broken.
- **Slow path** (line 132+): no cleanup scheduled at all. Relies on the OS temp cleaner, which on macOS only sweeps /tmp every few days.

The deterministic fix is to defer cleanup until after `run_ffmpeg` returns. Either:
- Wrap the call site in a guard pattern (`Drop` impl on a `TempFile` newtype).
- Use `tempfile::NamedTempFile` which auto-cleans on drop.
- Manually delete after the ffmpeg invocation completes.

The simplest is the manual-delete approach since the call structure already has a clear "after ffmpeg returns" point.

Relevant files:
- `src-tauri/src/operations/merge.rs:90-160` — concat list creation, fast/slow path branches
- `src-tauri/src/operations/merge.rs:126-130` — the sleep+spawn cleanup

## In scope
- Remove the `std::thread::spawn(...)` cleanup at lines 126-130.
- Add cleanup at the call site that invokes ffmpeg with these args. The call site is wherever the `args` vec from this function is consumed.
- Verify the slow-path branch (line 132+) doesn't need similar cleanup — if it creates any temp files, those need cleanup too.
- Consider migrating to `tempfile::NamedTempFile` for both paths — it auto-cleans on drop and avoids the manual cleanup boilerplate.
- Test (existing or new) that confirms `list_path` is removed after the merge call returns successfully.

## Out of scope
- Refactoring the entire merge operation to a single code path.
- Switching to ffmpeg's `concat protocol` (vs `concat demuxer`) to avoid the list file entirely. Larger change.
- Touching other operations.

## Steps
1. Read `operations/merge.rs:80-200` for full context. Identify the call site that consumes the `args` vec returned by this function. Trace upward through the call graph.
2. Decide on cleanup strategy:
   - **Option A (manual):** Return both `args` and `list_path` from this function. Caller deletes `list_path` after `run_ffmpeg` returns (success or failure).
   - **Option B (NamedTempFile):** Use `tempfile::NamedTempFile` for the concat list. The struct's `Drop` impl handles cleanup. Need to check if `tempfile` is already a dependency.
   - **Option C (RAII guard):** Create a local `TempFileGuard` newtype with a `Drop` impl. Construct it in the caller; pass `.path()` to ffmpeg.
3. Recommended: **Option A**. Smallest diff, no new dependency, matches the existing manual-cleanup style elsewhere in the codebase.
4. Refactor the function signature to return `(Vec<String>, Option<PathBuf>)` where the second element is the temp file to clean up after ffmpeg.
5. At the call site (likely `operations/merge.rs::run` or similar), add cleanup after `run_ffmpeg`:
   ```rust
   let result = run_ffmpeg(&args, ...);
   if let Some(p) = list_path {
       let _ = std::fs::remove_file(&p);
   }
   result?
   ```
6. Verify the slow-path branch (line 132+) for any analogous temp files. If found, apply the same pattern.
7. `cargo fmt --manifest-path src-tauri/Cargo.toml`
8. `cargo test --manifest-path src-tauri/Cargo.toml --lib operations::merge`
9. Manual smoke: `tauri dev`, run a merge of two short videos, confirm:
   - Merge completes successfully
   - The concat list file no longer exists in the temp dir afterwards
   - No error log about "file not found" mid-merge

## Success signal
- `grep "thread::sleep.*remove_file\|sleep.*list_path" src-tauri/src/operations/merge.rs` returns no matches.
- `grep "remove_file" src-tauri/src/operations/merge.rs` returns 1+ match at the new cleanup site.
- All merge tests pass.
- Manual smoke confirms cleanup works in both success and failure paths.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- The 2-second sleep was a workaround pattern that "usually works" because Linux holds the inode. It was never correct on Windows and was always racy on slow-cold-start cases. Deterministic cleanup is the right fix.
- If choosing Option B (NamedTempFile), check `Cargo.toml` first — `tempfile` may not be a current dependency. Adding it is fine for this purpose but requires bumping the lockfile.
- The slow-path concat-filter branch (line 132+) doesn't use a list file — it inlines `-i input1 -i input2 ...` for each input. So it has nothing to clean up. Verify by reading the slow-path code; the task scope is the fast-path list file only.
- Cleanup should happen on **failure too**, not just success — otherwise a mid-merge crash leaks files. The pattern in step 5 (cleanup *before* the `?` propagation) handles this.
