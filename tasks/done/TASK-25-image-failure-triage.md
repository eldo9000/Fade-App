# TASK-25: Triage image_full failures (skip if TASK-24 ran clean)

## Goal
Every OPEN entry from TASK-24's image_full run is either CONFIRMED (fix landed) or reclassified as env-blocked / fixture-shape with a clear note. Code-shape failures land as targeted fixes in `args/image.rs` (or `convert/image.rs`) following the BC-005 pattern: codec-aware arg builders with capability checks before emission.

## Context
**Skip this task if TASK-24's run was clean.** It only fires if image_full surfaced failures.

If image_full failures exist, classify each against BC-005 (`KNOWN-BUG-CLASSES.md` lines 35–110): is the failure caused by an option combination the encoder rejects? If yes, fix the arg builder. If the failure is encoder-not-found (analogous to libtheora / HAP / libaom-av1 absent), document as env-blocked and stop — env-blocked is not a release blocker, the picker already gates it.

ImageMagick delegates: AVIF goes through libheif. WebP through libwebp. TIFF through libtiff. PNG/JPEG/BMP through ImageMagick's built-in coders. Homebrew imagemagick on macOS ships all of these by default.

Likely failure shapes (priors, may not surface):
- AVIF speed > 9 — already clamped (BC-005 instance #2). If this resurfaces, the clamp regressed.
- TIFF bit-depth 32 + color mode mismatch — the `quantum:format=floating-point` define only makes sense for grayscale or RGB; CMYK 32-bit may reject.
- PNG palette + RGBA — PNG color-type 3 is paletted RGB, no alpha; combined with RGBA input may produce a warning or downgrade.
- BMP bit-depth 32 — ImageMagick BMP coder may reject 32-bit; older versions only support 1/4/8/16/24.

## In scope
- `src-tauri/src/args/image.rs` — fix arg builder for any code-shape failure
- `src-tauri/src/convert/image.rs` — only if a pre-flight guard is required (analogous to DNxHR resolution guard)
- `INVESTIGATION-LOG.md` — close OPEN entries from TASK-24
- `KNOWN-BUG-CLASSES.md` — append BC-005 instances for any code-shape fix

## Out of scope
- The image_full test code (don't modify cases — they're the ground truth)
- The picker (no UI changes — TASK-26 handles documentation)
- Adding new image formats (jpeg-xl, jp2, etc. — those stay `todo`)
- Touching `args/video.rs` or `convert/video.rs`

## Steps
For each OPEN entry from TASK-24:
1. Reproduce: copy the case's options, build the magick arg list, run by hand. Confirm the failure repeats and capture the exact error.
2. Classify:
   - **encoder-not-found** → env-blocked. Mark CONFIRMED in log with `(env-blocked, picker already gates)`. No code change.
   - **option-combination rejection** → BC-005. Fix the arg builder (clamp, auto-promote, or omit the conflicting flag). Add a numbered instance to BC-005 in `KNOWN-BUG-CLASSES.md`. Mark CONFIRMED in log with the commit SHA.
   - **fixture-shape** (e.g., 1×1 PNG too small for some encoder) → mark CONFIRMED with `(fixture-shape, not a shipping bug)`. Optionally note in case the test should grow the fixture.
3. Re-run `image_full` after each batch of fixes. The number of failures must monotonically decrease.
4. After all OPEN entries are closed (CONFIRMED or env-blocked), commit.

## Success signal
- Zero remaining OPEN entries from TASK-24 in the log.
- `cargo test --test full_sweep image_full -- --ignored` shows no code-shape failures.

## Notes
Each fix is its own commit (`fix(image): <BC-005 instance summary>`) with the BC-005 instance description in the body. Don't bundle multiple BC-005 fixes into one commit — they may need to be reverted independently.

Stop and surface to user if a single failure requires more than 30 minutes of investigation — that's a sign the failure is bigger than this sprint.
