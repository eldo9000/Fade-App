# TASK-19: H.264 lossless (crf=0) profile + pix_fmt guard

## Goal
When H.264 lossless encoding is requested (crf=0), `args/video.rs` automatically promotes the pixel format to `yuv444p` and the profile to "high444". The 120 `video_full` h264 lossless sweep failures are resolved. A user requesting crf=0 without specifying pix_fmt gets a working encode instead of an ffmpeg error.

## Context
`full_sweep.rs` `video_full` surfaces 120 H.264 failures for lossless cases (crf=0). The error is "baseline/main/high profile doesn't support lossless". Root cause: H.264 lossless encoding (`-crf 0`) requires yuv444p pixel format AND the "high444" profile. The existing `h264_effective_profile()` in `args/video.rs` already auto-promotes to "high444" when `pix_fmt = yuv444p`, but it does not force yuv444p when crf=0.

The fix: in the h264 branch of `build_ffmpeg_video_args()` (or wherever pix_fmt is emitted), detect `crf == Some(0)` and:
1. Override the effective pix_fmt to "yuv444p" (regardless of what was requested).
2. `h264_effective_profile()` will then return "high444" automatically (its existing logic).

This is purely additive — it adds an override when crf=0, touching nothing else.

Relevant files:
- `src-tauri/src/args/video.rs` — the h264 branch where pix_fmt and profile are emitted (~line 297–338)

## In scope
- `src-tauri/src/args/video.rs` — add lossless pix_fmt promotion in the h264 branch

## Out of scope
- H.265 lossless (separate constraint, separate profile space — not in scope here)
- `convert/video.rs` — do not touch
- Any UI changes
- Adding tests beyond the existing lib tests

## Steps
1. Read `src-tauri/src/args/video.rs` lines 297–340 to see the exact h264 branch structure and where pix_fmt is set.
2. Before the pix_fmt is emitted in the h264 arm, add:
   ```rust
   let effective_pix_fmt = if opts.crf == Some(0) {
       Some("yuv444p")
   } else {
       opts.pix_fmt.as_deref()
   };
   ```
   Use `effective_pix_fmt` wherever the h264 arm currently uses `opts.pix_fmt.as_deref()` for both the profile call and the `-pix_fmt` arg emission.
3. `cargo fmt --manifest-path src-tauri/Cargo.toml`
4. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`
5. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "crf.*Some(0)\|Some(0).*crf" src-tauri/src/args/video.rs` returns a match (the lossless guard).
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All lib tests pass.

## Notes
- `opts.crf` field type: check the actual field name in `ConvertOptions` — it may be `crf: Option<u32>` or `crf: Option<f32>`. The guard should match accordingly (`Some(0)` for u32, `Some(0.0)` for f32).
- H.264 lossless with yuv444p does produce visually perfect output but roughly doubles the bitrate of a CRF-0 encode vs yuv420p. That's expected and correct behaviour.
- This fix only applies to H.264. H.265 lossless is a different constraint with different profile requirements — do not extend this guard to h265.
