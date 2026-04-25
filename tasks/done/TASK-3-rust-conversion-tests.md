# TASK-3: Rust integration tests for conversion smoke (one per media category)

## Goal
`cargo test --test conversions` passes with one successful file conversion per media category (image, video, audio, data), each test calling the Rust conversion logic directly, generating a small fixture, converting it, and asserting the output file exists.

## Context
Fade's conversion pipeline lives in `src-tauri/src/`. The Tauri IPC layer (`src-tauri/src/lib.rs`) dispatches to per-format converter modules. For integration testing without a running Tauri app, we call the conversion modules directly from Rust integration tests in `src-tauri/tests/`.

**Rust integration tests** live in `src-tauri/tests/` (a sibling of `src/`). Files there are compiled as separate crates that have access to the library's public API (`use fade_lib::...`). The library crate is named `fade_lib` (from `src-tauri/Cargo.toml`: `[lib] name = "fade_lib"`).

**Fixture generation inside tests:** Tests generate their own small fixture files using `std::process::Command` to invoke `ffmpeg` and `imagemagick`. This avoids committed binary blobs. Each test writes its fixture to a temp directory (`tempfile::tempdir()` or `std::env::temp_dir()`), runs the conversion, asserts the output exists, then the temp dir is cleaned up automatically.

**How to invoke conversions:** Read `src-tauri/src/lib.rs` and the relevant per-format module files to understand the call chain. The `#[tauri::command] convert_file` function in `lib.rs` takes a `window`, `state`, `job_id`, `input_path`, and `ConvertOptions`. For direct testing, bypass the command and call the underlying conversion function directly. Read the converter modules under `src-tauri/src/operations/` or `src-tauri/src/converters/` to find the function to call.

**Required tools:** `ffmpeg` and `imagemagick` must be in PATH. On the macOS-14 CI runner they are NOT installed by default — add `brew install ffmpeg imagemagick` to `.github/workflows/ci.yml` before the Rust integration test step.

**What to test:**
1. **Image:** Generate a 10x10 PNG with imagemagick (`convert -size 10x10 xc:blue test.png`), convert to WebP, assert `test.webp` exists and is non-empty.
2. **Video:** Generate a 1-second H.264 MP4 with ffmpeg (`ffmpeg -f lavfi -i color=c=blue:size=64x64:rate=30 -t 1 -c:v libx264 -pix_fmt yuv420p test.mp4`), convert to WebM, assert `test.webm` exists.
3. **Audio:** Generate a 1-second WAV with ffmpeg (`ffmpeg -f lavfi -i sine=frequency=440:duration=1 test.wav`), convert to MP3, assert `test.mp3` exists.
4. **Data:** Write a small CSV string to a temp file, convert to JSON, assert `test.json` exists and contains valid JSON.

## In scope
- `src-tauri/tests/conversions.rs` — new integration test file
- `.github/workflows/ci.yml` — add `brew install ffmpeg imagemagick` step if not already present; add `cargo test --test conversions` step
- `src-tauri/Cargo.toml` — add `tempfile` dev-dependency if needed for temp dir creation

## Out of scope
- Any changes to `src/` Svelte source files
- Any changes to `src-tauri/src/` Rust source files (read-only)
- Any changes to existing `src-tauri/src/` tests
- Archive and model format conversions (out of scope)
- Any Playwright specs

## Steps

1. **Read `src-tauri/src/lib.rs`** to understand the `convert_file` command and how it dispatches to converters. Identify the underlying function or module that actually runs the conversion (likely a `run()` function in a per-format module).

2. **Read the relevant converter modules** — look in `src-tauri/src/operations/` or wherever image/video/audio/data converters live. Find the public function(s) to call from a test. If they are private (`pub(crate)` or no `pub`), note this — you may need to add `#[cfg(test)] pub` visibility, which is a minimal acceptable change to `src/`.

3. **Check `src-tauri/Cargo.toml`** for existing dev-dependencies. If `tempfile` is not present, add it under `[dev-dependencies]`:
   ```toml
   [dev-dependencies]
   tempfile = "3"
   ```

4. **Check `.github/workflows/ci.yml`** for an existing `brew install ffmpeg` step. If absent, add before the Rust integration test step:
   ```yaml
   - name: Install ffmpeg and imagemagick
     run: brew install ffmpeg imagemagick
   ```

5. **Create `src-tauri/tests/conversions.rs`.** Structure:
   - Helper function `run_cmd(program: &str, args: &[&str])` that calls `std::process::Command` and panics if the command fails.
   - Four `#[test]` functions, each:
     - Creates a temp directory via `tempfile::tempdir()` (or `std::env::temp_dir()` with a uuid-named subdir)
     - Generates the fixture file via `run_cmd`
     - Calls the converter function with the fixture path and expected output path
     - Asserts the output file exists: `assert!(output_path.exists(), "output not found: {:?}", output_path)`
     - Asserts the output is non-empty: `assert!(output_path.metadata().unwrap().len() > 0)`

6. **Run `cargo test --manifest-path src-tauri/Cargo.toml --test conversions` locally** to verify all four tests pass. Fix any visibility or call-site errors.

## Success signal
- `src-tauri/tests/conversions.rs` exists with four `#[test]` functions.
- `cargo test --manifest-path src-tauri/Cargo.toml --test conversions` exits 0 with all four tests passing.
- `.github/workflows/ci.yml` includes `brew install ffmpeg imagemagick` and `cargo test --test conversions` steps.
- No `src-tauri/src/` file was changed except for visibility annotations needed to expose functions to the integration test crate.

## Notes
- Integration tests in `src-tauri/tests/` use the crate as a black box — they can only call `pub` items. If the converter functions are not `pub`, check if there's a public wrapper to call. If there is no public path, add minimal `pub` visibility to the needed function (this is an acceptable src change with a comment explaining it's for integration testing).
- The `run_cmd` helper should capture stdout and stderr and include them in the panic message for easier debugging.
- If `tempfile` crate is not available and adding it is undesirable, use `std::env::temp_dir().join(format!("fade_test_{}", uuid::Uuid::new_v4()))` — `uuid` is already a dependency.
- Video conversion (MP4 → WebM) is the slowest test — may take 5–15 seconds on CI. If the default test timeout is too short, add `#[test] #[ignore]` and run with `--include-ignored` to exclude from the default `cargo test --lib` run while still runnable via `cargo test --test conversions`.
- The data conversion test (CSV → JSON) may be fast enough to not need ffmpeg — just write the CSV bytes directly with `std::fs::write`.
