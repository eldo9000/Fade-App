# TASK-1: Containment for archive extraction (Zip Slip / Tar Slip)

## Goal
`extract_archive` no longer permits archive entries to escape `dest_dir`. Both the `unar` and `7z` branches reject (or refuse to create) any entry whose canonical path does not lie under the canonical extraction root, including symlinks. A hostile archive can no longer write outside the extraction directory.

## Context
Static analysis surfaced a CWE-22 vulnerability in `convert/archive.rs`. Today both extractor branches honour archive-supplied paths verbatim:

- `convert/archive.rs:276-321` shells out to `unar -force-overwrite -o <dest_dir> <input>`. `-force-overwrite` exacerbates impact (no overwrite-prompt fallback).
- `convert/archive.rs:325-388` shells out to `7z x <input> -o<dest_dir> -y`. No `-snl-` (disallow symlinks), no per-entry path normalisation, no post-extraction containment walk.

A user who opens a downloaded `.zip` / `.cbr` / `.cbz` containing entries like `../../../Library/LaunchAgents/x.plist` or absolute paths gets arbitrary file write into HOME (or anywhere the user can write). For a media converter whose users routinely handle untrusted archives, this is a reachable, single-action exploit.

The cleanest defence is a Rust-side post-extraction walk that canonicalises every extracted file and rejects (deletes the extraction tree + errors out) if any path escapes `dest_dir`. As an additional layer, pass `-snl-` to 7z so it refuses to materialise symlinks at all.

Relevant files:
- `src-tauri/src/convert/archive.rs:258-389` (`extract_archive` function)
- `src-tauri/src/convert/archive.rs:120` (caller that creates `dest_dir`)
- `src-tauri/Cargo.toml` (no new dependency required — use `std::fs::canonicalize`)

## In scope
- Add `-snl-` to the 7z arg list at `archive.rs:326`.
- After successful extraction in **both** branches (before returning `ConvertResult::Done`), walk `dest_dir` recursively and verify every entry's canonicalised path starts with the canonicalised `dest_dir`. Any escape → delete `dest_dir`, return `ConvertResult::Error("Archive contains unsafe paths (entry escapes extraction root)")`.
- New helper `fn verify_extraction_contained(dest_dir: &str) -> Result<(), String>` in `convert/archive.rs`.
- Tests in `convert/archive.rs`'s `#[cfg(test)] mod tests`:
  1. Build a zip in-memory containing a benign file → `verify_extraction_contained` returns `Ok`.
  2. Build a zip containing `../escape.txt` (zip-slip entry); after `7z x`, verify the helper returns `Err`. Use the `zip` crate dev-dep if not already present, or hand-craft minimal test fixtures.
  3. Symlink case: create a symlink inside `dest_dir` pointing to `/etc/passwd`, helper returns `Err`.

## Out of scope
- Switching `extract_archive` to a pure-Rust extraction crate (`zip`, `tar`, `compress-tools`). Larger refactor; defer.
- The `repack_archive` direction (this task is read-only archives → disk).
- Touching `extract_audio` or any other unrelated extraction path.
- Any change to `convert_file` IPC entry validation (handled by TASK-2).

## Steps
1. Read `convert/archive.rs:258-389`. Confirm both branches return `ConvertResult::Done` on success without any path verification.
2. Add `-snl-` to the 7z arg list:
   ```rust
   .args(["x", input_path, &format!("-o{}", dest_dir), "-y", "-snl-"])
   ```
   This causes 7z to refuse extracting symlink entries entirely.
3. Implement `verify_extraction_contained`:
   ```rust
   fn verify_extraction_contained(dest_dir: &str) -> Result<(), String> {
       let root = std::fs::canonicalize(dest_dir)
           .map_err(|e| format!("cannot canonicalize extraction root: {e}"))?;
       fn walk(dir: &std::path::Path, root: &std::path::Path) -> Result<(), String> {
           for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
               let entry = entry.map_err(|e| e.to_string())?;
               let p = entry.path();
               // Reject symlinks outright (defence in depth alongside 7z -snl-)
               let meta = std::fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
               if meta.file_type().is_symlink() {
                   return Err(format!("symlink entry rejected: {}", p.display()));
               }
               let canon = std::fs::canonicalize(&p)
                   .map_err(|e| format!("cannot canonicalize {}: {e}", p.display()))?;
               if !canon.starts_with(root) {
                   return Err(format!("entry escapes extraction root: {}", p.display()));
               }
               if meta.is_dir() {
                   walk(&p, root)?;
               }
           }
           Ok(())
       }
       walk(&root, &root)
   }
   ```
4. Wire it in. In each branch, replace the trailing `ConvertResult::Done` with:
   ```rust
   if let Err(msg) = verify_extraction_contained(dest_dir) {
       let _ = std::fs::remove_dir_all(dest_dir);
       return ConvertResult::Error(msg);
   }
   ConvertResult::Done
   ```
5. Add the three tests listed under "In scope". Use `std::os::unix::fs::symlink` for the symlink test (gate with `#[cfg(unix)]`).
6. `cargo fmt --manifest-path src-tauri/Cargo.toml`
7. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::archive`
8. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "snl-" src-tauri/src/convert/archive.rs` returns a match.
- `grep "verify_extraction_contained" src-tauri/src/convert/archive.rs` returns 3+ matches (definition + 2 callers).
- `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::archive` exits 0 with the new tests passing.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- The canonicalize-walk approach catches both `../` traversal AND symlinks-to-outside, even ones the OS resolved during extraction. This is why the walk runs *after* extraction, not by parsing archive metadata.
- `-snl-` is 7z's "do not store/extract symlinks" flag. Combined with the post-walk it gives belt-and-braces: 7z refuses symlinks; the walk catches anything that slipped through (e.g. via unar's branch).
- On Windows the symlink test skips via `#[cfg(unix)]`; the canonicalize-walk still runs and still catches `..` escapes there.
- The fix for `dest_dir` itself living in `/tmp` (rather than `std::env::temp_dir()`) is TASK-5. Independent.
