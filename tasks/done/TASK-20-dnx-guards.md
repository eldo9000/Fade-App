# TASK-20: DNxHD resolution guard + DNxHR guard relocation

## Goal
`convert/video.rs` gains a DNxHD minimum-resolution guard (returns a clear error for sub-1280×720 output) mirroring the existing DNxHR guard. The DNxHR guard is also annotated with a doc comment making the "guard lives in convert(), not in args builder" contract explicit. The 7 DNxHD sweep failures on the 64×64 test fixture produce a clear error message instead of a cryptic ffmpeg rejection.

## Context
`full_sweep.rs` `video_full` surfaces:
- 7 DNxHD failures: all on the 64×64 test fixture. The `dnxhd` encoder validates (bitrate, resolution, fps, pix_fmt) tuples; sub-minimum-resolution inputs are rejected by ffmpeg with a cryptic error. No pre-flight check in `convert/video.rs` or `args/video.rs`.
- 5 DNxHR failures: also on the 64×64 fixture. `convert/video.rs` already has a 1280×720 minimum guard for DNxHR, but it only fires when `opts.resolution` is explicitly set. The sweep runs without an explicit resolution override, so the guard is bypassed — the 64×64 fixture passes through unchanged.

For DNxHD: add a resolution guard in `convert/video.rs` matching the DNxHR guard pattern. DNxHD also requires at least 1280×720.

For DNxHR bypass: the existing guard checks `opts.resolution`. When no resolution is explicitly set, the output resolution matches the input. Add a fallback: if `opts.resolution` is `None`, also reject if the input file dimensions are below 1280×720. However, determining input dimensions requires reading the file at the Rust layer, which is complex. A simpler and honest fix: document the contract gap with a comment and ensure the error message is clear. The "guard bypass" finding is a documentation/architecture issue, not a silent wrong-output issue — ffmpeg still rejects the encode, just with a less helpful error.

Pragmatic scope: add DNxHD guard matching the DNxHR pattern (same location, same message shape), and add a doc comment on the DNxHR guard explaining that it only covers explicitly-set resolutions.

Relevant files:
- `src-tauri/src/convert/video.rs` — DNxHR guard at lines 27–45; add DNxHD guard adjacent to it

## In scope
- `src-tauri/src/convert/video.rs` — add DNxHD guard, annotate DNxHR guard

## Out of scope
- `src-tauri/src/args/video.rs` — do not touch (guards intentionally live in convert layer)
- Input-dimension detection for the implicit-resolution case — too complex for this task
- Any UI changes
- Any other codecs

## Steps
1. Read `src-tauri/src/convert/video.rs` lines 1–60 to see the exact DNxHR guard and surrounding structure.
2. Immediately after the DNxHR block (lines 27–45), add an analogous DNxHD guard:
   ```rust
   // DNxHD minimum-resolution guard — same constraint as DNxHR.
   if opts.codec.as_deref() == Some("dnxhd") {
       if let Some(res) = &opts.resolution {
           if let Some((w_str, h_str)) = res.split_once('x') {
               if let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) {
                   if w < 1280 || h < 720 {
                       return ConvertResult::Error(
                           "DNxHD requires a minimum output resolution of 1280×720. \
                            Set a higher resolution or leave unscaled."
                               .to_string(),
                       );
                   }
               }
           }
       }
   }
   ```
3. Add a doc comment above the DNxHR guard block explaining the contract:
   ```rust
   // Guard fires only when opts.resolution is explicitly set. If the caller
   // passes no resolution, the input dimensions pass through unchanged —
   // ffmpeg will still reject sub-minimum inputs, but the error will be
   // less descriptive. Full pre-flight dimension detection is deferred.
   ```
4. `cargo fmt --manifest-path src-tauri/Cargo.toml`
5. `cargo test --manifest-path src-tauri/Cargo.toml --lib convert::video`
6. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

## Success signal
- `grep "dnxhd" src-tauri/src/convert/video.rs` returns a match inside a guard block.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- All lib tests pass.

## Notes
- The DNxHD guard intentionally mirrors the DNxHR guard exactly (same structure, same minimum, same message shape). This is the right call — both use the dnxhd ffmpeg encoder under the hood.
- The 7 DNxHD sweep failures are on the 64×64 test fixture which has no explicit resolution set, so this guard will NOT fix those failures (since `opts.resolution` will be None). The guard prevents user-visible bad UX when a resolution IS explicitly set. The sweep failures are a test-infrastructure issue (the fixture is too small for DNxHD) — they should be documented in INVESTIGATION-LOG.md after this task if not already. Do not add an INVESTIGATION-LOG entry in this task — the sweep finding already has one.
