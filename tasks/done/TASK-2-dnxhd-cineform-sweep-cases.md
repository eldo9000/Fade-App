# TASK-2: Add DNxHD and CineForm sweep cases to full_sweep.rs

## Goal
`src-tauri/tests/full_sweep.rs` has `dnxhd_cases()` and `cineform_cases()` functions, both integrated into `video_full()`, with `cargo check` passing clean.

## Context
BC-005 ("Encoder-constraint: UI presents invalid encoder-option combinations") was canonicalized in `KNOWN-BUG-CLASSES.md` at commit `5c1d58d`. The class describes a recurring pattern where Fade's UI exposes encoder-option combinations that an encoder silently rejects. The `full_sweep.rs` diagnostic test is the mechanism for surfacing these — it exhaustively tests format/option combinations against real FFmpeg.

`full_sweep.rs` currently covers: H.264, H.265, AV1, VP9, ProRes, DNxHR (5 profiles), HAP (4 sub-formats), and several others. Two codec paths are missing sweep coverage despite having option fields in `ConvertOptions`:

**DNxHD** — `ConvertOptions::dnxhd_bitrate: Option<u32>`. Supported bitrates (Mbps): 36, 115, 120, 145, 175, 185, 220. The `args/video.rs` handler (around line 372) maps `"dnxhd"` to `-vcodec dnxhd -b:v {br}M`. DNxHD is a fixed-bitrate codec that enforces exact bitrate×resolution×frame-rate combinations — specific bitrates are only valid at specific resolutions. This is the highest-risk BC-005 candidate because the arg builder passes any bitrate through without validation.

**CineForm** — `ConvertOptions::cineform_quality` (or similar). The `args/video.rs` handler (around line 381) maps `"cineform"` to `-vcodec cfhd -q:v 3`. The quality scale is 0 (lossless) to 12 (worst). The arg builder currently hardcodes `3` — the sweep should test the quality range to confirm the codec works and surface any bc-005-class issues.

Read `src-tauri/src/lib.rs` around line 253 and `src-tauri/src/args/video.rs` around lines 370–395 to confirm field names and valid option ranges before writing the test cases.

Read `src-tauri/tests/full_sweep.rs` around the `dnxhr_cases()` and `other_video_cases()` functions (lines 717–840) as style templates — new functions must follow the same pattern (`Vec<Case>`, `name`, `input`, `output`, `opts`).

All new test cases must be exercised inside `video_full()`. They must NOT carry `#[ignore]` individually — the `#[ignore]` attribute is on the `video_full()` test function itself, which is already set.

## In scope
- `src-tauri/tests/full_sweep.rs` — add `dnxhd_cases()` and `cineform_cases()` functions; extend `video_full()` to call them

## Out of scope
- `src-tauri/src/args/video.rs` — read only, no edits
- `src-tauri/src/lib.rs` — read only, no edits
- `src-tauri/src/convert/video.rs` — read only, no edits
- Any other sweep test file
- Any production source file

## Steps
1. Read `src-tauri/src/lib.rs` around line 250 — confirm `dnxhd_bitrate` field type and range comment.
2. Read `src-tauri/src/args/video.rs` around lines 370–395 — confirm how `"dnxhd"` and `"cineform"` are handled and what options they consume.
3. Read `src-tauri/tests/full_sweep.rs` lines 717–790 — use `dnxhr_cases()` as the style template.
4. Add `fn dnxhd_cases() -> Vec<Case>` covering the documented bitrate values (36, 115, 145, 185, 220 Mbps minimum; include 120 and 175 if the field comment lists them).
5. Add `fn cineform_cases() -> Vec<Case>` covering quality values 0, 3, 6, 12.
6. Read `fn video_full()` — locate where `dnxhr_cases()` is extended — add `cases.extend(dnxhd_cases());` and `cases.extend(cineform_cases());` in the same block.
7. Run `cargo check` from `src-tauri/` — must pass with no errors or warnings introduced by the new code.
8. Commit the change.

## Success signal
`cargo check` exits 0. `grep -n "dnxhd_cases\|cineform_cases" src-tauri/tests/full_sweep.rs` shows both the function definitions and the `cases.extend(...)` calls inside `video_full()`.

## Notes
DNxHD bitrate validation: the codec will fail at runtime if the bitrate doesn't match the resolution and frame rate of the fixture. The sweep is designed to surface exactly this kind of failure — don't add guards, let it run and report. The goal of this task is sweep coverage, not fixing any found failures. Found failures would be a separate BC-005 remediation arc.

CineForm: if `ConvertOptions` has no `cineform_quality` field (the arg builder hardcodes `-q:v 3`), add a single case with no extra options just to confirm the codec path works end-to-end.

Commit message: `test(sweep): add DNxHD bitrate and CineForm quality cases to full_sweep.rs`
