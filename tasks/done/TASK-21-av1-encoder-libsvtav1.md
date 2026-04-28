# TASK-21: Switch AV1 default encoder from libaom-av1 to libsvtav1

## Goal
`args/video.rs` uses `libsvtav1` as the default AV1 encoder instead of `libaom-av1`. AV1 encoding works out-of-the-box on Homebrew FFmpeg 8.1 (which ships libsvtav1 but not libaom-av1). The 9 `av1_*` sweep failures resolve.

## Context
`full_sweep.rs` `video_full` surfaces 9 AV1 failures with "Encoder not found". Root cause: `args/video.rs` line 248 hardcodes `libaom-av1` as the AV1 encoder. Homebrew FFmpeg 8.1 ships `libsvtav1` only — `libaom-av1` is not in the Homebrew build.

The fix is a one-line change: `"libaom-av1"` → `"libsvtav1"`.

Relevant context on encoder differences:
- `libaom-av1`: Google's reference AV1 encoder — very slow, highest quality
- `libsvtav1`: Intel's Scalable Video Technology AV1 — much faster, good quality; ships in Homebrew FFmpeg 8.1
- Both produce standards-compliant AV1. For Fade's use case (user-facing conversion), libsvtav1 is preferable — it's what ships and it's faster.

Note: `libsvtav1` does not support all the same `-tune` and `-preset` values as `libaom-av1`. Check whether the existing h264/h265-style `-preset` or `-tune` args are emitted for AV1 — if they are, they may need to be suppressed or mapped.

Relevant files:
- `src-tauri/src/args/video.rs` — `ffmpeg_video_codec_args()` at ~line 248

## In scope
- `src-tauri/src/args/video.rs` — change the AV1 encoder string; check for any preset/tune args that need adjustment for libsvtav1

## Out of scope
- Runtime encoder detection (probing ffmpeg at startup) — deferred; one-line fix is sufficient
- libtheora absent — separate codec, separate task if desired
- HAP encoder absent — environment issue, already documented in INVESTIGATION-LOG
- Any UI changes

## Steps
1. Read `src-tauri/src/args/video.rs` — find the AV1 codec arm in `ffmpeg_video_codec_args()` and any AV1-specific arg blocks in `build_ffmpeg_video_args()`.
2. Change `"libaom-av1"` to `"libsvtav1"` in the codec arm.
3. Check if any `-preset`, `-tune`, or other encoder-specific args are emitted for AV1. `libsvtav1` supports `-preset` (0–13, lower = slower/better quality) but does NOT support `-tune`. If tune args are emitted for AV1, suppress them for libsvtav1.
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "libsvtav1" src-tauri/src/args/video.rs` returns a match.
- `grep "libaom-av1" src-tauri/src/args/video.rs` returns no matches.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All lib tests pass.

## Notes
- `libsvtav1` preset range is 0–13 (not the same as libx264/libx265 named presets like "slow"/"medium"/"fast"). If the existing arg builder passes named presets for AV1, they need to be mapped or suppressed. If it passes numeric presets or no presets, the change is purely the encoder name.
- This change only affects macOS dev installs with Homebrew FFmpeg. If a Linux user has `libaom-av1` available, they lose it with this change. This is an acceptable trade-off given the project is primarily macOS-dev — a runtime detection mechanism can be added later if needed.
- After this change, update INVESTIGATION-LOG.md: change the `libaom-av1 absent` OPEN entry to CONFIRMED (resolved by this commit).
