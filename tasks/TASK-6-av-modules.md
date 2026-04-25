# TASK-6: Refactor image, audio, video conversion modules

## Goal
`convert::image`, `convert::audio`, and `convert::video` each expose a new `pub fn convert(...)` callable without `&Window`. `run()` becomes a thin wrapper. Existing `conversions.rs`, `matrix.rs`, and `full_sweep.rs` test files keep passing — they exercise the `build_*_args` helpers directly and that path must not break. A new test file `src-tauri/tests/refactored_av_sweep.rs` calls each `convert()` directly with a small set of cases. Existing behavior is unchanged from a Tauri caller's perspective.

## Context
Sixth task in the `&Window` decoupling arc. TASK-1 established `convert::progress`. TASKs 2–5 refactored 11 modules. This task handles the three biggest media converters. They're already partially testable (`build_image_magick_args`, `build_ffmpeg_audio_args`, `build_ffmpeg_video_args` are `pub`), but `run()` itself still needs `&Window`.

These modules are different from the others in two ways:
1. They have heavier progress emission — ffmpeg progress parsing, percent updates, multi-stage pipelines (e.g. video with audio extraction).
2. Their tests already exist and are extensive (`matrix.rs` has 33 cases, `full_sweep.rs` has ~700+). Those tests exercise the arg builders, NOT `run()`. They must keep passing untouched.

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

The progress emitter for AV modules is the most complex of any in the codebase. Image probably emits a few phases. Audio emits ffmpeg-percent-parsed updates. Video emits the same plus possibly multi-pass updates. Preserve every existing emit. Do not change frequency, do not change payload shape — frontend's progress bar parses these.

Relevant files:
- `src-tauri/src/convert/image.rs` — ImageMagick shell-out
- `src-tauri/src/convert/audio.rs` — ffmpeg shell-out, parses progress from stderr
- `src-tauri/src/convert/video.rs` — ffmpeg shell-out, possibly multi-pass, parses progress from stderr
- `src-tauri/src/args/{image,audio,video}.rs` — already public, do NOT touch
- `src-tauri/src/convert/progress.rs` — from TASK-1
- `src-tauri/tests/conversions.rs`, `matrix.rs`, `full_sweep.rs` — must keep passing

## In scope
- `src-tauri/src/convert/image.rs` — extract `convert()`, reduce `run()`
- `src-tauri/src/convert/audio.rs` — same
- `src-tauri/src/convert/video.rs` — same
- `src-tauri/tests/refactored_av_sweep.rs` (new file) — at least 1 case per module via `convert()` directly

## Out of scope
- `src-tauri/src/args/*.rs` — already public, do not touch
- `convert/archive.rs` — TASK-7
- The existing arg-builder tests — leave them alone, they test a different layer
- `src-tauri/src/lib.rs`
- Any progress payload shape change

## Steps
1. Read `src-tauri/src/convert/progress.rs` and one previously-refactored module to refresh the wrapper pattern.
2. Read `src-tauri/src/convert/image.rs` end to end. Note every `window.emit` call and the payload shape. Image is the simplest of the three — a single ImageMagick invocation with stdout/stderr capture.
3. Refactor `image.rs`:
   - Extract `pub fn convert()` from `run()`. Replace each emit with `progress(ProgressEvent::...)`.
   - Reduce `run()` to a wrapper. Build: `cargo build --manifest-path src-tauri/Cargo.toml`.
4. Read `src-tauri/src/convert/audio.rs` end to end. Note: how is ffmpeg progress parsed? Does it spawn ffmpeg with `-progress pipe:1` and parse line-by-line? Each emit during that loop must be preserved.
5. Refactor `audio.rs` using the same pattern. The progress parsing loop moves into `convert()`, but its emit calls become `progress(ProgressEvent::Percent(p))` etc.
6. Read `src-tauri/src/convert/video.rs` end to end. Note multi-pass / preview / different progress phases. Refactor with the same pattern. This is the biggest module of the three; allow extra time.
7. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. The old `conversions.rs`, `matrix.rs`, and (if you run with `--include-ignored`) `full_sweep.rs` must all pass — they call arg builders directly and must be unaffected by the refactor.
8. Create `src-tauri/tests/refactored_av_sweep.rs`:
   - For each module, synthesize a fixture using `magick`/`ffmpeg` (same patterns as `matrix.rs`).
   - Call `image::convert()` for one image conversion (PNG → WebP), `audio::convert()` for one audio conversion (WAV → MP3), `video::convert()` for one video conversion (MP4 → WebM, mark `#[ignore]` because of encode time).
   - Use `noop_progress()` and an empty `processes` map for all calls.
   - Assert output file exists and is non-empty.
9. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_av_sweep --no-run`.
10. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_av_sweep -- --include-ignored --nocapture`.

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0; all of `conversions.rs`, `matrix.rs`, the un-ignored parts of `full_sweep.rs`, and `extra_sweep.rs` still pass.
- `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_av_sweep -- --include-ignored --nocapture` exits 0 with ≥ 3 PASS rows.
- Each refactored module has at most one `window.emit` site (inside the `run()` wrapper closure).
- `wc -l` on `run()` in each refactored module shows ≤ 30 lines (slightly looser than other tasks because AV modules have more wiring).

## Notes
- ffmpeg progress parsing typically reads stderr line-by-line. The reading loop stays in `convert()`. Each line that previously triggered `window.emit` now triggers `progress(ProgressEvent::Percent(p))`. The parsing logic itself is unchanged.
- DO NOT refactor the arg builders. They're already in the right shape and are heavily depended upon by existing tests.
- If `audio.rs` or `video.rs` has a "two-pass encode" or "audio extraction then re-encode" pipeline, both passes' emits must be preserved.
- Cancellation: these modules check `cancelled` periodically during the ffmpeg loop. That logic stays in `convert()` — `cancelled` is already plumbed through.
