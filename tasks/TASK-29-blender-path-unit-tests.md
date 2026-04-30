# TASK-29: Unit tests for locate_script() and find_blender()

## Goal
`src-tauri/src/convert/model_blender.rs` and `src-tauri/src/args/model_blender.rs` each have at least one unit test covering the path-resolution logic. `cargo check` passes. The tests exercise the candidate-list construction in `locate_script()` and the PATH-lookup + hardcoded-path fallback in `find_blender()` — not whether Blender is actually installed.

## Context
`locate_script()` (`convert/model_blender.rs:13–58`) builds a priority-ordered candidate list: CWD-relative, exe-adjacent, macOS bundle, Linux FHS. It returns the first `path.exists()` hit, or a diagnostic error listing all tried paths. No unit test exists for this function.

`find_blender()` (`args/model_blender.rs:4–67`) does a 5-stage PATH + hardcoded-path search. It returns `Option<PathBuf>`. No unit test exists.

The goal is to validate the structure of the candidate list and the error message format — not to mock the filesystem or actually install Blender. Tests that call `locate_script()` in an environment where the script doesn't exist should verify the error message lists the expected number of paths and contains the filename "blender_convert.py". Tests for `find_blender()` are necessarily environment-sensitive; they should at minimum confirm the function returns without panicking and that the PATH lookup runs first.

## In scope
- `src-tauri/src/convert/model_blender.rs` — add `#[cfg(test)]` module with tests for `locate_script()` error path (script not found → error lists candidates containing "blender_convert.py")
- `src-tauri/src/args/model_blender.rs` — add `#[cfg(test)]` module with a smoke test for `find_blender()` (call it, assert it either returns `Some(path)` with a non-empty path, or `None` — no panic)

## Out of scope
- Adding Blender version check logic (deferred)
- Mocking the filesystem
- Touching `convert/model_blender.rs` logic (test module only)
- Touching `args/model_blender.rs` logic (test module only)
- Any other file

## Steps
1. Read `src-tauri/src/convert/model_blender.rs` lines 1–60 — understand `locate_script()` fully.
2. Read `src-tauri/src/args/model_blender.rs` lines 1–75 — understand `find_blender()` fully.
3. In `convert/model_blender.rs`, add:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn locate_script_error_lists_candidates() {
           // In a test environment the script won't exist at any candidate path.
           // Verify the error message is diagnostic (lists "blender_convert.py").
           // NOTE: this test may pass unexpectedly if run from a dir that has the script.
           // That's acceptable — it just means the function works correctly.
           match locate_script() {
               Ok(_) => { /* script found — acceptable in dev */ }
               Err(e) => {
                   assert!(
                       e.contains("blender_convert.py"),
                       "Error should mention script filename, got: {e}"
                   );
                   assert!(
                       e.contains("Tried:"),
                       "Error should list tried paths, got: {e}"
                   );
               }
           }
       }
   }
   ```
4. In `args/model_blender.rs`, add:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn find_blender_does_not_panic() {
           // Result is environment-dependent; just confirm no panic and
           // that any returned path is non-empty.
           let result = find_blender();
           if let Some(path) = result {
               assert!(!path.as_os_str().is_empty(), "Blender path should be non-empty");
           }
           // None is valid — Blender may not be installed in CI
       }
   }
   ```
5. Run `cargo check` from `src-tauri/` — must pass with no errors or warnings from the new code.
6. Do NOT commit (dispatcher commits).

## Success signal
- `cargo check` exits 0.
- `grep -n "locate_script_error_lists_candidates\|find_blender_does_not_panic" src-tauri/src/convert/model_blender.rs src-tauri/src/args/model_blender.rs` shows both test functions.

## Notes
These are smoke/contract tests, not integration tests. They validate the structure of the code, not runtime behavior. They must pass in CI (where Blender is not installed) — both are written to accept `None`/`Ok` as valid outcomes.
