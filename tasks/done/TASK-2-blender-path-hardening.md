# TASK-2: Harden blender_convert.py path resolution for packaged app

## Goal
`locate_script()` in `src-tauri/src/convert/model_blender.rs` checks all deployment-relevant locations for `blender_convert.py` and includes a diagnostic error message that lists which paths were tried. Conversion no longer fails silently with "not found" when the script exists in the app bundle at a path the current code doesn't check.

## Context
The `locate_script()` function currently checks two paths:
1. `scripts/blender_convert.py` relative to CWD (works in `tauri dev`)
2. `<exe_parent>/scripts/blender_convert.py` (works if the binary is next to the scripts dir)

The fragility: in a packaged macOS app bundle, the binary lives at `Fade.app/Contents/MacOS/Fade`, so `exe.parent()` returns `Contents/MacOS/`. The scripts directory in a Tauri 2 bundle is placed in `Contents/Resources/` by default — one level up from the binary, in a sibling directory. The current code never checks `exe.parent().parent().join("Resources/scripts/...")`.

Similarly, on Linux, packaged apps may have resources at `../share/<appname>/` or `../lib/` relative to the binary. On Windows, resources are typically alongside the executable, which the current code does handle.

The fix adds the macOS bundle path (`../Resources/scripts/...`) as a third candidate and improves the error message to enumerate all attempted paths, so diagnostic output makes the failure actionable.

Relevant files:
- `src-tauri/src/convert/model_blender.rs` — `locate_script()` function (lines ~13–30)
- No other files need changing

The function currently:
```rust
fn locate_script() -> Result<PathBuf, String> {
    let rel = PathBuf::from("scripts/blender_convert.py");
    if rel.exists() { return Ok(rel); }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("scripts/blender_convert.py");
            if candidate.exists() { return Ok(candidate); }
        }
    }

    Err("blender_convert.py not found — ...")
}
```

## In scope
- `src-tauri/src/convert/model_blender.rs` — extend `locate_script()` with additional search paths and an improved error message

## Out of scope
- Any change to `build_blender_args`, `find_blender`, or `convert()`
- Any change to `src-tauri/tauri.conf.json` or the build configuration
- Any change to where `blender_convert.py` is stored in the source tree
- Any change to other modules

## Steps
1. Read `src-tauri/src/convert/model_blender.rs` lines 13–30 (the `locate_script` function) and lines 40–70 (how it's called from `convert()`).
2. Extend `locate_script()` to build a list of candidate paths and try each in order. The candidates, in priority order:
   a. `scripts/blender_convert.py` (CWD-relative — works in dev)
   b. `<exe_parent>/scripts/blender_convert.py` (sibling of binary — works in some layouts)
   c. `<exe_parent>/../Resources/scripts/blender_convert.py` (macOS app bundle — `exe.parent().parent().join("Resources/scripts/...")`)
   d. `<exe_parent>/../share/fade/scripts/blender_convert.py` (Linux FHS layout)
3. For each candidate that exists, return `Ok(candidate)`.
4. If none exist, build an error message that lists all tried absolute paths:
   ```
   "blender_convert.py not found. Tried:\n  <path1>\n  <path2>\n..."
   ```
   Use `canonicalize` on the CWD-relative path to show the absolute form. For paths built from `current_exe`, show the resolved absolute form.
5. `cargo build --manifest-path src-tauri/Cargo.toml` must succeed.
6. `cargo test --manifest-path src-tauri/Cargo.toml --lib` must exit 0.
7. `cargo fmt --manifest-path src-tauri/Cargo.toml` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `grep -n "Resources\|share/fade" src-tauri/src/convert/model_blender.rs` returns matches showing the new search paths.
- `grep "Tried:" src-tauri/src/convert/model_blender.rs` returns a match (improved error message).
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- `PathBuf::from("scripts/blender_convert.py").canonicalize()` may fail if the path doesn't exist — use `std::fs::canonicalize()` and fall back to the raw path in the error message if it fails.
- The function is `fn locate_script() -> Result<PathBuf, String>` (no arguments) — keep that signature; don't add parameters to thread AppHandle through.
- This fix does NOT require a Tauri resource API — it uses standard `std::env::current_exe()` + path manipulation only.
- The new paths don't need to handle all possible layouts — just add the macOS bundle and Linux FHS candidates. Windows layouts already work with candidate (b).
