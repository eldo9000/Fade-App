# TASK-7: Add unit tests for VBR/CBR and image sequence FFmpeg arg builders

## Goal
`src-tauri/src/args/video.rs` has unit test coverage for the VBR/CBR codec paths and the image sequence arg builder, matching the coverage density of the existing WebM CBR test matrix already in that file.

## Context
Two backend feature areas shipped in commit `4bd1839` (VideoOptions overhaul) with no unit tests:

**1. VBR/CBR for h264/h265** — `codec_quality_args` in `args/video.rs` gained a `video_bitrate_mode` branch:
- `"vbr"` → emits `-b:v {br}k`
- `"cbr"` → emits `-b:v {br}k -minrate {br}k -maxrate {br}k -bufsize {br*2}k`
- `"crf"` (default) → existing behavior, already tested implicitly

The WebM CBR/VBR matrix (around line 547 of video.rs) is the established pattern. The h264/h265 paths need the same treatment.

**2. Image sequence arg builder** — `build_image_sequence_args` in `args/video.rs` is an entirely new function with no tests. It handles:
- Extension dispatch: `seq_png` → codec `png`, `seq_jpg` → codec `mjpeg` with `q:v` mapping, `seq_tiff` → codec `tiff`
- JPEG quality mapping: CRF 0 = q:v 2 (best), CRF 51 = q:v 31 (worst) — `2 + (crf * 29 / 51)`
- Trim: if `trim_start` is set, emits `-ss {trim_start}`; if `trim_end` is set, emits `-t {trim_end - trim_start}`
- Frame rate: if `framerate` is set, emits `-r {framerate}`
- Resolution: if `resolution` is set (e.g. `"1920x1080"`), emits `-vf scale=...`
- Always emits `-an` (no audio)
- Output pattern: the function receives `output_dir`; it appends `/frame_%04d.{ext}` as the last arg

The test infrastructure is already established in `video.rs`. The existing `find_pair(args, flag, value)` helper checks for adjacent flag+value pairs in the args Vec. There's also an established pattern of `ConvertOptions { ..Default::default() }` for constructing minimal test fixtures.

**Relevant file:** `src-tauri/src/args/video.rs` — read the test module (starting around line 529) and the new functions (search for `build_image_sequence_args` and `codec_quality_args`) before writing tests.

**How to call `build_image_sequence_args` in tests:** The function is private (`fn`, not `pub fn`). It's called via `build_ffmpeg_video_args` when `output_format.starts_with("seq_")`. Write tests that call `build_ffmpeg_video_args("in.mp4", "/tmp/frames", &opts)` with `output_format: "seq_png"` etc., and inspect the returned args.

**How to call VBR/CBR paths:** `build_ffmpeg_video_args("in.mp4", "out.mp4", &opts)` with `output_format: "mp4"`, `codec: Some("h264")`, `video_bitrate_mode: Some("vbr")`, `video_bitrate: Some(4000)`.

## In scope
- `src-tauri/src/args/video.rs` — add tests to the existing `#[cfg(test)]` module

## Out of scope
- Any source code changes — tests only, no production code changes
- `src-tauri/src/lib.rs` — do not modify
- Frontend files

## Steps
1. Read `src-tauri/src/args/video.rs` from line 529 to end (the test module).
2. Read `build_ffmpeg_video_args`, `codec_quality_args`, and `build_image_sequence_args` in full to understand the arg shapes.
3. Write VBR/CBR test cases for h264:
   - `h264_vbr_emits_bitrate`: `video_bitrate_mode: "vbr"`, `video_bitrate: 4000` → args contain `-b:v 4000k`; do NOT contain `-minrate` or `-maxrate`.
   - `h264_cbr_emits_full_cbr_flags`: `video_bitrate_mode: "cbr"`, `video_bitrate: 4000` → args contain all four CBR flags at correct values (`-b:v 4000k`, `-minrate 4000k`, `-maxrate 4000k`, `-bufsize 8000k`).
   - `h264_cbr_default_bitrate`: `video_bitrate_mode: "cbr"`, no `video_bitrate` → uses 4000k default.
   - `h264_crf_mode_unchanged`: `video_bitrate_mode: "crf"` or `None` → no `-b:v` flag; `-crf` flag present.
   - Repeat the VBR and CBR cases for `h265`.
4. Write image sequence test cases:
   - `seq_png_emits_png_codec`: `output_format: "seq_png"` → args contain `-vcodec png`; output arg ends with `frame_%04d.png`.
   - `seq_jpg_emits_mjpeg_and_quality`: `output_format: "seq_jpg"`, `crf: Some(0)` → `-vcodec mjpeg` and `-q:v 2`; `crf: Some(51)` → `-q:v 31`.
   - `seq_tiff_emits_tiff_codec`: `output_format: "seq_tiff"` → `-vcodec tiff`.
   - `seq_png_trim_start_emits_ss`: `trim_start: Some(10.0)` → `-ss 10` in args before `-i`.
   - `seq_png_trim_end_emits_t`: `trim_start: Some(5.0)`, `trim_end: Some(15.0)` → `-t 10` (duration = end - start).
   - `seq_png_framerate`: `framerate: Some(24)` → `-r 24` in args.
   - `seq_no_audio`: output args always contain `-an`.
5. Run `cargo test --manifest-path src-tauri/Cargo.toml` to confirm all tests pass.
6. Run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
7. Run `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.

## Success signal
`cargo test` passes with all new tests green. The new test count is ≥12 (8 VBR/CBR + at least 7 sequence). `cargo clippy -D warnings` clean. No production code was changed.

## Notes
The JPEG quality mapping formula: `q:v = 2 + (crf as u32 * 29 / 51)`. At CRF 0: `2 + 0 = 2`. At CRF 51: `2 + 29 = 31`. At CRF 23 (default): `2 + (23 * 29 / 51) ≈ 2 + 13 = 15`. Write at least one test at CRF 23 to verify the midpoint.

For the `-ss` trim-start flag: FFmpeg applies `-ss` before `-i` (input seek) when placed before the input. Verify the actual arg order in `build_image_sequence_args` before writing position-sensitive assertions.
