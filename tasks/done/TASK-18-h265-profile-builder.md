# TASK-18: H.265 codec-aware profile builder

## Goal
`args/video.rs` has a dedicated `h265_effective_profile()` function that maps pixel formats and profile hints to valid libx265 profile names. The `"h264" | "h265" =>` branch is split so H.265 uses its own profile logic instead of calling `h264_effective_profile()` (which returns libx264 names that libx265 rejects). All 27 `video_full` h265 sweep cases pass.

## Context
`full_sweep.rs` `video_full` surfaces 27 H.265 failures with the error "unknown profile <high>". Root cause: `src-tauri/src/args/video.rs` around line 297 handles both h264 and h265 in one branch (`"h264" | "h265" =>`), and both codecs call `h264_effective_profile()`. That function returns libx264 profile names ("baseline", "main", "high", "high422", "high444") which are invalid for libx265. libx265 uses a completely different profile namespace: "main", "main10", "main422-10", "main444-8", "main444-10".

The fix is to split the shared branch and introduce `h265_effective_profile()`. Profile mapping for libx265:
- `pix_fmt = yuv444p` → "main444-8"
- `pix_fmt = yuv422p` → "main422-10"
- `pix_fmt = yuv420p10le` or profile hint = "main10" → "main10"
- everything else → "main"

Both codecs share most args (preset, tune, pix_fmt). Only the profile name generation differs.

Relevant files:
- `src-tauri/src/args/video.rs` — `h264_effective_profile()` at ~line 280, shared `"h264" | "h265" =>` branch at ~line 297

## In scope
- `src-tauri/src/args/video.rs` — add `h265_effective_profile()`, split the shared branch

## Out of scope
- `src-tauri/src/convert/video.rs` — do not touch
- H.264 logic — do not change `h264_effective_profile()` or the h264 branch
- Any UI changes
- Adding tests (the full_sweep run in TASK-15 is the regression test)

## Steps
1. Read `src-tauri/src/args/video.rs` end-to-end to understand the current branch shape and where `h264_effective_profile()` is called.
2. Add `h265_effective_profile()` adjacent to `h264_effective_profile()`. Signature mirrors `h264_effective_profile` — takes `profile: Option<&str>` and `pix_fmt: Option<&str>`, returns `&str`. Implement the mapping:
   - `pix_fmt = "yuv444p"` → "main444-8"
   - `pix_fmt = "yuv422p"` → "main422-10"
   - `profile = "main10"` or `pix_fmt = "yuv420p10le"` → "main10"
   - else → "main"
3. Split `"h264" | "h265" =>` into two separate arms (`"h264" =>` and `"h265" =>`). The h265 arm is identical to the h264 arm except the profile call becomes `h265_effective_profile(profile_opt, pix_fmt_opt)`.
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "h265_effective_profile" src-tauri/src/args/video.rs` returns 2+ matches (definition + call site).
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All lib tests pass.

## Notes
- libx265 profile names differ entirely from libx264: "main", "main10", "main422-10", "main444-8", "main444-10" — not "high"/"high422"/"high444".
- The two arms will be nearly identical except for the profile function call. That duplication is intentional — sharing a branch to avoid duplication was the bug.
- After this fix, running `cargo test --test full_sweep -- --include-ignored video_full` should show the 27 h265 cases green (assuming ffmpeg libx265 is present). Do not run the full sweep as part of this task — it's too slow for a CI build check.
