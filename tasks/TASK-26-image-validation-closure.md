# TASK-26: Close image validation pass — document live set

## Goal
`SESSION-STATUS.md` reflects 0.6.5 image validation closure analogous to v0.6.4 audio. The live image output set is documented (jpeg, png, webp, tiff, bmp, avif) along with any per-format caveats surfaced by TASK-24/25. The 19 todo-flagged formats remain todo (no work done in this sprint to enable them — that's a separate decision).

## Context
0.6.4 closed audio validation: confirmed live formats listed, pruned formats noted, $effect clamps in `AudioOptions.svelte` for stale values. The image equivalent should follow the same shape.

The image picker (`src/App.svelte:1573`) already separates live (`{ id: 'jpeg' }`) from todo (`{ id: 'gif', todo: true }`). Live formats: jpeg, png, webp, tiff, bmp, avif. Todo formats: gif, svg, ico, jpegxl, heic, heif, psd, exr, hdr, dds, xcf, raw, cr2, cr3, nef, arw, dng, orf, rw2.

`ImageOptions.svelte` already conditionally renders option panels per format. Whether it needs `$effect` clamps depends on what TASK-24/25 surfaced — analogous clamps to AudioOptions only apply if a format-specific option (e.g. tiff bit-depth) only works in a subset of contexts.

## In scope
- `SESSION-STATUS.md` — add a "0.6.5 image validation pass complete" block under `## Next action`, mirroring the 0.6.4 audio entry
- `INVESTIGATION-LOG.md` — add a single dated CONFIRMED entry summarizing the validation closure (cross-reference TASK-24/25 entries already there)
- `src/lib/ImageOptions.svelte` — only if TASK-25 surfaced a format-specific stale-value bug analogous to the AudioOptions clamps. If not, do nothing here.

## Out of scope
- The picker (no live/todo flag changes in this task)
- Enabling any todo-flagged format (separate sprint)
- Adding new format-specific options
- Any test file
- Any Rust file

## Steps
1. Read the audio validation memory pattern (see `project_060_audio_validation.md`) and the 0.6.4 SESSION-STATUS / INVESTIGATION-LOG entries that closed it. Mirror the shape.
2. Re-read TASK-24 and TASK-25 outcomes from `INVESTIGATION-LOG.md`.
3. Append to `SESSION-STATUS.md` under `## Next action`:
   ```
   v0.6.5: image validation pass complete YYYY-MM-DD. Confirmed live image output formats: jpeg (q/chroma/progressive), png (compression/color-mode/interlaced), webp (lossless/method/quality), tiff (compression/bit-depth/color-mode — caveats: <if any>), bmp (bit-depth — caveats: <if any>), avif (speed/chroma/quality — speed clamped to ≤9 per BC-005 #2). Todo formats remain unimplemented (gif, svg, ico, jpegxl, heic, heif, psd, exr, hdr, dds, xcf, 8 raw camera formats).
   ```
4. Append to `INVESTIGATION-LOG.md`:
   ```
   YYYY-MM-DD | CONFIRMED | 0.6.5 image validation closure — 6 live formats verified end-to-end via image_full sweep (143 cases). Caveats documented in SESSION-STATUS. Todo formats out of scope.
   ```
5. If TASK-25 surfaced a format-specific stale-value bug, add the matching `$effect` clamp in `ImageOptions.svelte`.
6. Commit.

## Success signal
- `SESSION-STATUS.md` has a 0.6.5 closure block.
- A future agent reading `SESSION-STATUS.md` can answer "which image formats does Fade ship?" without reading any other file.

## Notes
This is a documentation task, not a fix task. If TASK-25 didn't run (TASK-24 was clean), there are no caveats to document — keep the closure block minimal. If TASK-25 produced fixes, every BC-005 instance number added to `KNOWN-BUG-CLASSES.md` should be referenced in the SESSION-STATUS block.

Save a memory entry analogous to `project_060_audio_validation.md` for `project_065_image_validation.md` so the closure persists across conversations.

Commit message: `docs(status): record 0.6.5 image validation closure — 6 live formats verified`
