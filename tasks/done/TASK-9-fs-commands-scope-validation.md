# TASK-9: Apply path-traversal + allowed-roots scoping to `fs_commands`

## Goal
`file_exists` and `scan_dir` (in `src-tauri/src/fs_commands.rs`) reject paths that contain `..` traversal or fall outside the allowed-roots set (HOME, TMPDIR, /Volumes, /media, /mnt). The renderer can no longer probe arbitrary filesystem paths via these IPC commands.

## Context
Both commands today accept any path with no scoping. From `fs_commands.rs:17-19`:

```rust
#[command]
pub fn file_exists(path: String) -> Result<bool, String> {
    Ok(std::path::Path::new(&path).exists())
}
```

And `scan_dir` at `:29` similarly accepts any path. Today the impact is bounded — Tauri's content security policy and asset scope keep the renderer from being attacker-controlled. But:

- This is a textbook reconnaissance primitive: enumerate `/Users/`, probe `/etc/`, etc.
- Tauri's `fs:*` plugin is intentionally restrictive precisely because these primitives are unsafe to expose unscoped.
- Any future XSS, template-injection, or compromised dependency in the renderer chain becomes an immediate disk-reconnaissance vector.
- The fix is two existing validators (`validate_no_traversal`, `validate_output_dir`) and ~6 lines of code.

The `validate_output_dir` validator (`lib.rs:499-523`) already encodes the right allowed-roots set. It's `pub(crate)` so directly callable from `fs_commands.rs` after a small re-export.

Relevant files:
- `src-tauri/src/fs_commands.rs:17-64` — both commands
- `src-tauri/src/lib.rs:499-523` — `validate_output_dir`
- `src-tauri/src/lib.rs:527-535` — `validate_no_traversal`
- `src-tauri/src/fs_commands.rs:67-211` — existing test patterns to extend

## In scope
- Apply `validate_no_traversal(&path)?` at the top of both `file_exists` and `scan_dir`.
- Apply an allowed-roots check (matching `validate_output_dir`'s logic) to both. May need to expose `validate_output_dir` as `pub(crate)` if it isn't already, or factor the allowed-roots check into a separate `validate_allowed_root` helper that both commands can call.
- For `file_exists`: if the path is a file, check the parent directory against allowed roots.
- For `scan_dir`: check the path itself against allowed roots.
- Tests:
  1. `file_exists_rejects_traversal_path` (path: `../etc/passwd`)
  2. `file_exists_rejects_path_outside_allowed_roots` (path: `/etc/passwd`)
  3. `file_exists_accepts_path_under_home` (regression — temp dir)
  4. `scan_dir_rejects_traversal` (path: `../`)
  5. `scan_dir_rejects_root_outside_allowed_roots` (path: `/etc`)
  6. `scan_dir_accepts_temp_dir` (regression)

## Out of scope
- Auditing other Tauri commands beyond these two (do that as a separate sweep).
- Changing the depth/entry caps in `scan_dir` (already correct).
- Touching `validate_output_dir`'s logic.
- Tauri scope plugin configuration changes.

## Steps
1. Read `fs_commands.rs` end-to-end.
2. Read `lib.rs:494-535` to confirm validator signatures and visibility.
3. Make `validate_output_dir` and `validate_no_traversal` accessible from `fs_commands.rs` — they're already `pub(crate)` per the source. If `fs_commands` doesn't see them, add `use crate::{validate_no_traversal, validate_output_dir};` at the top.
4. Modify `file_exists`:
   ```rust
   #[command]
   pub fn file_exists(path: String) -> Result<bool, String> {
       validate_no_traversal(&path)?;
       // file_exists may be called with a file path; check the parent dir
       let p = std::path::Path::new(&path);
       let dir_for_check = if p.is_dir() {
           path.clone()
       } else {
           p.parent()
               .and_then(|d| d.to_str())
               .unwrap_or(&path)
               .to_string()
       };
       validate_output_dir(&dir_for_check)?;
       Ok(p.exists())
   }
   ```
5. Modify `scan_dir`:
   ```rust
   #[command]
   pub fn scan_dir(path: String, recursive: Option<bool>) -> Result<Vec<String>, String> {
       validate_no_traversal(&path)?;
       validate_output_dir(&path)?;
       // ... existing body unchanged
   }
   ```
6. Update existing tests in `fs_commands.rs` if they pass paths outside allowed roots — they should still work because `unique_tmp` uses `std::env::temp_dir()` which is in the allowed-roots set.
7. Add the six new tests.
8. `cargo fmt --manifest-path src-tauri/Cargo.toml`
9. `cargo test --manifest-path src-tauri/Cargo.toml --lib fs_commands`
10. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "validate_no_traversal\|validate_output_dir" src-tauri/src/fs_commands.rs` returns 4+ matches.
- The six new tests pass.
- All existing `fs_commands` tests pass.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- Calling `validate_output_dir` on a *file path* (not a directory) over-rejects, which is why step 4's `file_exists` check uses the parent dir. Alternative: introduce a `validate_path_in_allowed_roots(path)` that doesn't require `path` to be a directory — would be cleaner but adds API surface.
- This task closes a defence-in-depth gap, not an actively-exploited bug. The threat model assumes a future renderer compromise (XSS, dependency hijack); today the renderer is local app code.
- The Tauri asset scope and CSP already provide a strong outer perimeter. This task adds the inner perimeter for these two commands so they match the rest of the codebase's discipline.
