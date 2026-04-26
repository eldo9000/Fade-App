# TASK-2: Guard DNxHR conversions against unsupported minimum resolution

## Goal
Attempting to encode with DNxHR when the source (or scaled) resolution is below the minimum supported by the dnxhd encoder returns a clear `ConvertResult::Error` before ffmpeg is spawned, rather than a generic "Conversion failed!" after ffmpeg exits non-zero.

## Context
Fade's `full_sweep.rs` diagnostic (run 2026-04-25) found that all DNxHR conversions on the 64×64 test fixture fail across all five profiles (LB, SQ, HQ, HQX, 444). Root cause: the `dnxhd` encoder (which handles both DNxHD and DNxHR) requires a minimum resolution — the lowest DNxHR profile (LB) supports 1280×720 at minimum; the higher profiles require 1920×1080+. Fade currently passes whatever dimensions the source has to ffmpeg without validation. The encoder rejects it silently from Fade's perspective.

The fix is an early-exit guard in the arg builder: when `opts.codec` is `"dnxhr"`, check whether the effective output resolution is below 1280×720. If it is, return an error before building the arg list. The guard lives in the Rust arg builder (`src-tauri/src/args/video.rs`), not in the UI — this is a runtime safeguard, not a UI restriction.

"Effective output resolution" means: if `opts.resolution` is set, parse its dimensions; otherwise the guard cannot know the source resolution at arg-build time, so the guard should only fire when `opts.resolution` is explicitly set to a sub-minimum value. When no resolution is specified, the guard passes (ffmpeg will use source dimensions; the user will see an ffmpeg error if source is too small — that's acceptable until probe metadata is wired).

Relevant files:
- `src-tauri/src/args/video.rs` — the DNxHR branch at lines 377–380. This is where the guard goes. The `resolution_to_scale` helper at line 586+ shows how resolution strings are parsed; reuse that pattern to extract width/height.
- `src-tauri/src/lib.rs` — `ConvertOptions::dnxhr_profile` and `ConvertOptions::resolution` fields. Doc comments may need a note.
- `src-tauri/src/args/video.rs` existing test module — add unit tests for the guard.

## In scope
- `src-tauri/src/args/video.rs` — add the resolution guard in the DNxHR branch. Add unit tests for it.
- `src-tauri/src/lib.rs` — add a doc note to `dnxhr_profile`: `// Requires output resolution ≥ 1280×720; returns error if opts.resolution is set below this.`

## Out of scope
- Any change to `VideoOptions.svelte` — no UI change for this task
- Any change to the DNxHD codec branch (only DNxHR is affected)
- Probe/metadata-based source-resolution checking (that's a future feature)
- Any change to `full_sweep.rs` or `matrix.rs`
- Any change to non-DNxHR codecs

## Steps
1. Read `src-tauri/src/args/video.rs` lines 377–380 (the DNxHR branch) and lines 586–600 (the `resolution_to_scale` helper that parses `"WxH"` strings). Understand the `opts.resolution` format (`"1920x1080"`, `"1280x720"`, etc.).
2. In the DNxHR branch (before the `-profile:v` arg is pushed), add a guard:
   - If `opts.resolution` is `Some(res)`, parse it as `"WxH"` to extract width and height integers.
   - If width < 1280 or height < 720, return `Err("DNxHR requires a minimum output resolution of 1280×720. Set a higher resolution or leave unscaled.".to_string())`.
   - The branch's return type needs to accommodate this — check what `build_ffmpeg_video_args` returns (likely `Result<Vec<String>, String>` or `Vec<String>`). If it's infallible (`Vec<String>`), the guard must use a different mechanism. Read the function signature first and adapt accordingly: if infallible, the guard should be placed in `video::convert()` before the arg-build call instead.
3. Add unit tests to `args/video.rs`'s `#[cfg(test)] mod tests`:
   - `dnxhr_resolution_guard_rejects_small_resolution`: set `codec = "dnxhr"`, `resolution = Some("640x360".into())`. Assert the result is an error containing "1280".
   - `dnxhr_resolution_guard_allows_hd_resolution`: set `codec = "dnxhr"`, `resolution = Some("1920x1080".into())`. Assert no error (args are returned).
   - `dnxhr_resolution_guard_allows_unset_resolution`: set `codec = "dnxhr"`, `resolution = None`. Assert no error (guard does not fire when resolution is unspecified).
4. Update the doc comment on `ConvertOptions::dnxhr_profile` in `src-tauri/src/lib.rs`.
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video`. All tests pass.
6. `cargo fmt --manifest-path src-tauri/Cargo.toml` + `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. Both clean.

## Success signal
- `cargo test --manifest-path src-tauri/Cargo.toml --lib args::video` exits 0 with the 3 new DNxHR guard tests passing.
- `grep "1280" src-tauri/src/args/video.rs` returns a match in the guard logic.
- `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` exits 0.

## Notes
- The resolution string format in `ConvertOptions` is `"WxH"` (e.g., `"1920x1080"`). Split on `'x'` and parse each part as `u32`. If parsing fails (malformed string), skip the guard — don't fail on malformed input at the guard level; let the main encoder error naturally.
- If `build_ffmpeg_video_args` is infallible (returns `Vec<String>` not `Result<...>`), read `src-tauri/src/convert/video.rs` to find where `build_ffmpeg_video_args` is called and place the guard check there before the call, returning `ConvertResult::Error(...)` directly.
- Do not hard-code per-profile minimums (LB=1280×720, SQ/HQ=1920×1080, etc.) — a single 1280×720 floor covers all profiles safely. The user wanting a lower resolution should use a different codec.
