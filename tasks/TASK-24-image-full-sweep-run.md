# TASK-24: Run image_full sweep and report results

## Goal
`cargo test --test full_sweep image_full -- --ignored --nocapture` runs to completion. Every failure (if any) is documented in `INVESTIGATION-LOG.md` as a dated OPEN entry with the case name, the underlying error message, and a one-line classification (code-shape vs env-shape). If zero failures, log a single CONFIRMED entry stating the live image set passed clean.

## Context
v0.6.4 closed audio output validation by running the audio sweep, classifying every failure, and pruning unsupported formats from the picker. The image equivalent has not been run as a gating pass.

`src-tauri/tests/full_sweep.rs` already contains `image_full()` (test fn at line 351, `#[ignore]`). It generates a PNG fixture and exercises six format families in one run:

- `jpeg_cases()` — 3 quality × 3 chroma × 2 progressive = 18 cases
- `png_cases()` — 4 compression × 5 color modes × 2 interlaced = 40 cases
- `webp_cases()` — 2 lossless × 3 method × 3 quality = 18 cases
- `tiff_cases()` — 4 compression × 3 bit-depth × 3 color modes = 36 cases
- `bmp_cases()` — 4 bit-depths = 4 cases
- `avif_cases()` — 3 speed × 3 chroma × 3 quality = 27 cases

Total: 143 cases. Each calls `build_image_magick_args()` and runs `magick`. Failures are surfaced via `report()` (writes per-case files under `test-results/image/`).

The picker exposes exactly these 6 live image output formats (`src/App.svelte:1574`); 19 additional formats are flagged `todo: true`. The live picker set is what this sprint validates.

## In scope
- Running the test
- Appending per-failure OPEN entries (or one CONFIRMED entry on clean) to `INVESTIGATION-LOG.md`
- No source edits

## Out of scope
- Fixing any surfaced failures (deferred to TASK-25)
- Touching `args/image.rs`, `convert/image.rs`, the picker, or the UI options panel
- Re-running other sweeps (audio/video/extra)

## Steps
1. From `src-tauri/`: `cargo test --test full_sweep image_full -- --ignored --nocapture` (this can take several minutes — ImageMagick spawned per case).
2. Capture stderr + stdout. Identify any line containing "FAIL" or non-zero `magick` exit.
3. For every failed case: append to `INVESTIGATION-LOG.md` as `YYYY-MM-DD | OPEN | image_full <case-name> — <one-line error> — <code-shape | env-shape>`.
4. If zero failures: append one line `YYYY-MM-DD | CONFIRMED | image_full sweep clean (143 cases, 6 live formats: jpeg, png, webp, tiff, bmp, avif) — gating run for 0.6.5 image validation`.
5. Commit.

## Success signal
- The test process exits.
- `INVESTIGATION-LOG.md` reflects the run outcome.

## Notes
- Do not touch the test code itself. If the test panics due to a missing `magick` binary, stop and report — that's an env failure, not a test failure.
- The fixture is generated per-run via `make_png()`, so no checked-in fixture is required.
- Output files end up under `target/test-results/image/` (per the `output_root` helper) — leave them in place.

Commit message: `test(sweep): run image_full — <N> cases, <M> failures, log updated`
