# TASK-7: Refactor archive conversion module

## Goal
`convert::archive` exposes a new `pub fn convert(...)` callable without `&Window`. `run()` becomes a thin wrapper. All four internal repack paths (generic 7z repack, ISO repack, DMG repack, tar variants) are reachable through the new pure function. A new test file `src-tauri/tests/refactored_archive_sweep.rs` calls `convert()` with at least one zip↔tar.gz round-trip case, skipping if `7z` isn't in PATH. Existing behavior is unchanged from a Tauri caller's perspective.

## Context
Seventh task in the `&Window` decoupling arc. TASK-1 established `convert::progress`. TASKs 2–6 refactored 14 modules. This task handles the most complex one alone: `convert::archive`.

`archive.rs` is ~600 lines and contains four conversion paths:
- Generic repack via 7z (most archives)
- ISO repack (special handling for ISO output)
- DMG repack (macOS-only, hdiutil)
- Tar variants (tar, tar.gz, tar.xz)

Each path has its own `window.emit` calls — typically: extracting %, repacking %, finalize. After refactor, `convert()` keeps the dispatcher and all four paths; only the emit sites change to `progress(ProgressEvent::...)`.

Same wrapper pattern as previous tasks:

```rust
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> { /* ... */ }

pub fn run(...) -> ConvertResult {
    let mut emit = /* closure with original payload shape */;
    convert(..., &mut emit, processes, &cancelled)
}
```

The inner helpers (`repack_archive`, `repack_with_7z`, `repack_iso`, `repack_dmg`, `extract_archive`) currently take `&Window`. They should be updated to take `progress: ProgressFn` instead. The dispatcher passes the progress callback down to each helper.

Progress payload shape MUST match what `archive.rs` currently emits — there's a 7z percent parser (`parse_7z_percent`) feeding the emits, and the frontend depends on the exact shape.

Relevant files:
- `src-tauri/src/convert/archive.rs` — the whole module
- `src-tauri/src/convert/progress.rs` — from TASK-1

## In scope
- `src-tauri/src/convert/archive.rs` — extract `convert()`, reduce `run()`, update all internal `repack_*` and `extract_archive` helpers to take `ProgressFn` instead of `&Window`
- `src-tauri/tests/refactored_archive_sweep.rs` (new file) — at least one zip↔tar.gz case, plus a tar.xz case if feasible, with `7z`-availability skipping

## Out of scope
- All other `convert/*.rs` modules — refactored in earlier tasks
- `src-tauri/src/lib.rs`
- Any progress payload shape change visible to the frontend
- Any change to `parse_7z_percent` or the helper utility functions

## Steps
1. Read `src-tauri/src/convert/progress.rs` and one previously-refactored module (TASK-3 ebook is a good reference — same shell-out shape, simpler).
2. Read `src-tauri/src/convert/archive.rs` end to end. List every function that takes `&Window`: there are at least 6 (`run`, `extract_archive`, `repack_archive`, `repack_with_7z`, `repack_iso`, `repack_dmg`). For each, note every `window.emit` call.
3. Refactor the leaf helpers first (no dependency on other helpers). Order: `repack_with_7z` → `repack_iso` → `repack_dmg` → `extract_archive` → `repack_archive` → `run`.
4. For each leaf helper: change `window: &Window` to `progress: ProgressFn` in the signature, replace each `window.emit("job-progress", payload)` with `progress(ProgressEvent::...)`. Build after each helper to catch errors early.
5. Once all helpers take `ProgressFn`, extract `convert()` from `run()`. The dispatcher logic (which path is chosen based on input/output extension) moves into `convert()`. `convert()` calls the helpers passing `progress` through.
6. Reduce `run()` to a wrapper that builds the emit closure and calls `convert()`. Build: `cargo build --manifest-path src-tauri/Cargo.toml`.
7. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. All must pass.
8. Create `src-tauri/tests/refactored_archive_sweep.rs`:
   - Skip-if-tool-missing helper:
     ```rust
     fn has_7z() -> bool {
         std::process::Command::new("7z").arg("--help").output()
             .map(|o| !o.stdout.is_empty()).unwrap_or(false)
     }
     ```
   - Synthesize a small zip fixture: create a temp dir, drop a couple of small files, zip it.
   - Case 1: zip → tar.gz. Call `archive::convert()` with `noop_progress()` and an empty `processes` map. Assert output exists.
   - Case 2: tar.gz → 7z. Round-trip sanity.
   - Case 3 (optional): zip → tar.xz.
   - All cases skip if `has_7z()` returns false.
9. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_archive_sweep --no-run`.
10. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_archive_sweep -- --nocapture`.

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_archive_sweep -- --nocapture` exits 0 with at least 2 PASS rows (or all SKIP if 7z missing).
- `archive.rs::run()` is ≤ 25 lines after refactor.
- `grep "&Window" src-tauri/src/convert/archive.rs` returns one line (the `run()` wrapper signature) — every other helper now takes `ProgressFn`.

## Notes
- Archive's complexity is in the dispatcher — the "is this an iso, a dmg, a tar variant, or a 7z target" decision tree. Don't try to simplify or restructure it during the refactor; lift it as-is.
- DMG repack only runs on macOS. The `cfg(target_os = "macos")` guards stay in place.
- The existing `parse_7z_percent` function is pure-Rust and already works — no changes needed.
- The 7z child process is registered in `processes` for cancellation. Preserve that registration logic exactly.
- Don't write a Cartesian sweep here. Archive's "matrix" of input format × output format × compression level is huge but not particularly informative; one zip↔tar.gz round-trip proves the wrapper plumbing works. Bigger sweeps can come in a later iteration.
