# Sprint A — Image Format Expansion

**Goal:** Unlock the 19 image output formats currently marked `todo` in the picker. ImageMagick is already wired; most of these are classifier + encoder path verification.

**Entry condition:** `image_full` sweep passes clean on all 6 live formats. `convert/image.rs` `convert()` contract stable.

---

## TASK-A1: GIF output (static)

**Scope:** Enable GIF as a live output format in `convert/image.rs`.

**What to do:**
- Set `gif` to `live: true` in the image format picker
- Verify ImageMagick converts all 6 live input formats → GIF without errors
- Add GIF output cases to `image_full` sweep
- Confirm palette quantization produces acceptable output (not banded)

**Done when:** `image_full` sweep includes GIF output cases and all pass. CI green.

---

## TASK-A2: ICO output

**Scope:** Enable ICO as a live output format.

**What to do:**
- Set `ico` to `live: true` in picker
- ImageMagick supports ICO; verify multi-size ICO (16, 32, 48, 256) works via `-define icon:auto-resize`
- Add ICO output cases to sweep

**Done when:** Sweep passes with ICO output. CI green.

---

## TASK-A3: SVG input → raster output

**Scope:** Accept SVG as input format (rasterize to JPEG/PNG/WebP/etc.).

**What to do:**
- ImageMagick calls Inkscape/rsvg-convert internally for SVG; verify dependency is present or add fallback error message
- Set SVG `live: true` as input-only
- Add SVG input cases to sweep (PNG and JPEG output targets)
- Decide: output SVG? (SVG → SVG is a no-op; SVG → SVGZ is compression only — defer)

**Done when:** SVG input converts cleanly to raster formats. CI green.

---

## TASK-A4: HEIC / HEIF input and output

**Scope:** Enable HEIC/HEIF conversion.

**What to do:**
- Check ImageMagick HEIC support: requires `libheif`. Run `convert -list format | grep HEIC` in CI environment
- If missing: add detection + user-facing error "HEIC requires ImageMagick built with libheif"
- If present: set `heic` and `heif` `live: true`, add sweep cases
- macOS: `sips` can also decode HEIC — add fallback path if ImageMagick fails

**Done when:** HEIC/HEIF converts on supported environments; clear error on unsupported. CI green.

---

## TASK-A5: JPEG XL input and output

**Scope:** Enable JPEG XL conversion.

**What to do:**
- Check ImageMagick JXL support: `convert -list format | grep JXL`
- If missing: fall back to `cjxl`/`djxl` CLI tools (libjxl)
- Add detection logic that tries ImageMagick first, then libjxl binaries
- Set `jpegxl` `live: true`, add sweep cases

**Done when:** JXL converts on supported environments. CI green.

---

## TASK-A6: HEIF / PSD / EXR / HDR / DDS / XCF

**Scope:** Enable remaining ImageMagick-supported advanced image formats.

**What to do (per format):**
- **PSD**: ImageMagick supports PSD natively. Set `live: true`, sweep.
- **EXR**: Requires ImageMagick with OpenEXR. Detect + error or live.
- **HDR** (Radiance RGBE): ImageMagick supports `.hdr`. Set `live: true`, sweep.
- **DDS**: ImageMagick supports DDS via `DDS:` prefix. Verify and sweep.
- **XCF** (GIMP): ImageMagick supports XCF input. Set `live: true` as input-only (output is unusual).

For each: add sweep cases; if dependency missing emit clear diagnostic.

**Done when:** All 5 formats have sweep coverage and pass or emit clear missing-dep errors. CI green.

---

## TASK-A7: RAW camera format input (CR2, CR3, NEF, ARW, DNG, ORF, RW2)

**Scope:** Accept camera RAW files as input, output to JPEG/PNG/TIFF/AVIF.

**What to do:**
- Evaluate two paths: (a) `dcraw`/`libraw` CLI → TIFF → ImageMagick, (b) ImageMagick with `dcraw` delegate
- Check CI environment: `which dcraw` or `which rawtherapee-cli`
- Implement decode step in `convert/image.rs`: if input is RAW, decode to temp TIFF first, then hand to ImageMagick
- Add RAW input cases to sweep (one fixture per manufacturer format if possible)
- If no decoder available: emit clear error "RAW formats require dcraw or LibRaw"

**Done when:** At least DNG and one manufacturer RAW convert cleanly or emit the dependency error. CI green.
