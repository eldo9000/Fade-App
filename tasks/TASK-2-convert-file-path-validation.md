# TASK-2: Apply validate_* to `convert_file` IPC entry

## Goal
The `convert_file` IPC command rejects unsafe input paths, output directories, and assembled output paths the same way `run_operation` does. The CLAUDE.md rule "validate_output_name() before any CLI arg interpolation" is enforced for all 14 media-type pipelines that route through `convert_file`, not just the operations subset.

## Context
`convert_file` (`lib.rs:665-786`) is the busiest IPC entry — every conversion routes through it. Today it validates only the suffix (`validate_suffix`) at line 750 and the separator (`validate_separator`) at line 752. It does **not** validate:

- `input_path` (passed straight from the frontend; can contain `..`)
- `options.output_dir` (used to compose the output path; can be any directory)
- The assembled `output_path` returned by `build_output_path` (line 779-786)

Three validators already exist in `lib.rs` and are tested:
- `validate_no_traversal(path)` at `lib.rs:527-535` — rejects any `..` component
- `validate_output_dir(dir)` at `lib.rs:499-523` — rejects `..` and roots outside HOME / TMPDIR / /Volumes / /media / /mnt
- `validate_output_name(name)` at `lib.rs:~439` — rejects names that aren't ASCII alphanumeric + `-_.`

`OperationPayload::validate_outputs()` (`lib.rs:1271-1312`) calls these for the operations dispatch. `convert_file` is a separate code path that bypasses the umbrella entirely.

The static-analysis pass mistakenly verdict'd this as PASS because shell-args themselves use separate argv elements (no `format!` into a `Command::arg(...)`). That's true but narrow. The CLAUDE.md rule applies to any user-supplied data hitting the IO layer — `std::fs::write`, `std::fs::create_dir_all`, ffmpeg input args, etc.

Relevant files:
- `src-tauri/src/lib.rs:665-786` — `convert_file` body
- `src-tauri/src/lib.rs:439-535` — the three validators
- `src-tauri/src/lib.rs:1271-1312` — `OperationPayload::validate_outputs` (reference impl)
- `src-tauri/src/lib.rs:3261-3320` — existing validator tests (pattern to copy)

## In scope
- Add the following calls inside `convert_file`, in this order, before the cancellation-flag insert at line 789:
  1. `validate_no_traversal(&input_path)?` — immediately after the existence check at line 678.
  2. `validate_output_dir(dir)?` for `options.output_dir` — before `build_output_path` is called at line 779. Skip if `output_dir` is `None`.
  3. `validate_output_name(file_name_only)?` — on the file-name component of `output_path` (not the full path). After `output_path` is computed, extract the last path component and validate it.
- Tests in `lib.rs`'s `#[cfg(test)] mod tests` (or the existing IPC test module):
  1. `convert_file_rejects_traversal_in_input_path`
  2. `convert_file_rejects_output_dir_outside_allowed_roots`
  3. `convert_file_rejects_output_dir_with_parent_component`
  4. `convert_file_accepts_safe_paths` (regression)

## Out of scope
- Refactoring `convert_file` to deduplicate logic with `run_operation` (architecture finding A-1; defer).
- Changing the `OperationPayload::validate_outputs` umbrella (it's correct).
- The duckdb SQL hardening (TASK-7).
- The Zip Slip fix in `convert/archive.rs` (TASK-1).
- Splitting `lib.rs` into smaller files (M-2; defer).

## Steps
1. Read `lib.rs:665-786` end-to-end. Note the existing validation calls at lines 692 (ext check), 750 (suffix), 752 (separator).
2. Read `lib.rs:439-535` to confirm the validator signatures haven't drifted.
3. Insert `validate_no_traversal(&input_path)?;` immediately after the `if !p.exists()` block at line 679.
4. Insert validation for `options.output_dir` after the suffix/separator validation at line 752:
   ```rust
   if let Some(dir) = options.output_dir.as_deref() {
       validate_output_dir(dir)?;
   }
   ```
5. After `output_path` is computed (line 786), validate its file-name component:
   ```rust
   if let Some(name) = std::path::Path::new(&output_path)
       .file_name()
       .and_then(|n| n.to_str())
   {
       validate_output_name(name)?;
   }
   ```
   Do this for both branches of the `if let Some(real_ext) = ext.strip_prefix("seq_")` — the seq directory branch *also* needs validation (validate the directory's base name, not the full sequence path).
6. Add the four tests. Use the existing `validate_*` test patterns (`lib.rs:3261-3320`) as a reference for `Path` shapes.
7. `cargo fmt --manifest-path src-tauri/Cargo.toml`
8. `cargo test --manifest-path src-tauri/Cargo.toml --lib`
9. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "validate_no_traversal\|validate_output_dir\|validate_output_name" src-tauri/src/lib.rs` shows additional call sites inside `convert_file` (line range 665-790).
- The four new tests pass.
- All existing tests continue to pass — no regressions in the conversion suite.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- The validators return `Result<(), String>`. `convert_file` already returns `Result<(), String>` so `?` propagation works directly.
- `validate_output_name` accepts the *file name component* only (per its existing tests). Validating the full path would over-reject (paths contain `/` which is not in the allowed set).
- The seq-dir branch (line 754-777) calls `std::fs::create_dir_all(&seq_dir)` — that's an IO sink that needs validation upstream just like `build_output_path`'s output.
- Tests must use a temp dir under `std::env::temp_dir()` for the "accepts safe paths" case so they pass the `validate_output_dir` allowed-roots check.
- The static-analysis pass said this was PASS. It wasn't — the pass checked shell-arg-level safety, not the broader CLAUDE.md rule. This task closes the gap.
