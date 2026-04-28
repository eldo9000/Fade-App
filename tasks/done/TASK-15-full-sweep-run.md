# TASK-15: Run full_sweep.rs and document BC-005 findings

## Goal
Every `#[ignore]` test in `src-tauri/tests/full_sweep.rs` is executed manually. New encoder-constraint failures are appended to `INVESTIGATION-LOG.md` as OPEN entries. Known failures (HAP encoder absent) are skipped. The output is a documented findings report, not a green test run — failures are the expected product.

## Context
`full_sweep.rs` is a ~700-case Cartesian diagnostic covering image, audio, data, and video codecs. All cases are `#[ignore]` (manual-only; CI does not run them). The tests call the Rust `convert()` functions directly with `noop_progress()`, so each case shells out to ffmpeg or ImageMagick.

The test functions are:
- `image_full` — JPEG, PNG, WebP, TIFF, BMP, AVIF
- `audio_full` — MP3, WAV, FLAC, OGG, AAC, Opus, M4A
- `data_full` — CSV conversions (JSON, YAML, XML, TSV)
- `hap_full` — HAP sub-formats (already fully documented OPEN in INVESTIGATION-LOG.md; re-run but do not duplicate entries)
- `video_full` — H.264, H.265, AV1, VP9, ProRes, DNxHR, DNxHD, CineForm, FFV1, MJPEG, RawVideo, QTRLE, DV, MPEG1/2, Theora, Xvid, GIF

BC-005 (encoder-constraint class in `KNOWN-BUG-CLASSES.md`) documents confirmed instances: AVIF speed cap, DNxHR resolution minimum, H.264 profile/pix_fmt impossible combos. This sweep surfaces new instances.

INVESTIGATION-LOG.md already has two OPEN entries for HAP (2026-04-28) — do not duplicate them. Focus on new failure classes.

Run time estimate: 700 cases × ~0.5s each = ~6 minutes on a modern Mac with small fixtures.

## In scope
- `src-tauri/tests/full_sweep.rs` — the sweep file to run
- `INVESTIGATION-LOG.md` — append new OPEN entries for new findings
- `KNOWN-BUG-CLASSES.md` — read for context; do not modify (findings → INVESTIGATION-LOG only)

## Out of scope
- `src-tauri/tests/extra_sweep.rs` — separate task
- Fixing any failures found — document only
- Modifying test cases or the sweep structure
- Any source file under `src-tauri/src/`

## Steps
1. Read `INVESTIGATION-LOG.md` tail (last 20 lines) to see existing OPEN entries — avoid duplicating them.
2. Run image, audio, and data categories first (faster):
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep -- --include-ignored image_full audio_full data_full 2>&1 | tee /tmp/fade_sweep_img_aud_data.txt
   ```
3. Run the video category (longest):
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep -- --include-ignored video_full 2>&1 | tee /tmp/fade_sweep_video.txt
   ```
4. Run hap_full (already-known failures — run for completeness but only document if a NEW failure class appears):
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep -- --include-ignored hap_full 2>&1 | tee /tmp/fade_sweep_hap.txt
   ```
5. Parse each output file for `FAILED` lines. For each failed test:
   - Read the failure output to identify the error message (ffmpeg stderr excerpt).
   - Determine the constraint class (resolution, pixel format, bitrate, container, encoder absent, etc.).
   - Group failures by constraint class — do not write one entry per test case; write one per distinct constraint.
6. For each NEW constraint class not already in INVESTIGATION-LOG.md, append:
   ```
   2026-04-28 | OPEN | <constraint description> — full_sweep <test_function> cases
   ```
7. Count and report totals: how many cases passed, how many failed, how many distinct constraint classes found.

## Success signal
- All four `cargo test` commands complete without hanging (timeout > 15 min is a hang).
- INVESTIGATION-LOG.md has one OPEN entry per NEW constraint class found (zero new entries is also valid if no new constraints surface).
- The notes return includes the PASSED / FAILED counts and a list of new entries written (or "no new findings").

## Notes
- If `video_full` times out, re-run by codec family: use a test name substring filter like `-- --include-ignored h264` if the test names allow it. The test function is `video_full` so you can only filter at that level; if it's too slow, run it in two halves by temporarily editing the test to split the cases — but do NOT commit that change.
- Failures due to missing fixture files (not encoder constraints) should be noted separately — they're a different failure class.
- A test that panics (not just fails) is a distinct finding — note the panic location.
- AVIF, DNxHR, DNxHD, H.264 combos: these have known fixes already in the codebase. If they still fail, that means the fix is incomplete — flag it.
