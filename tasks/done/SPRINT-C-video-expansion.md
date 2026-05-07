# Sprint C — Video Format Expansion

**Goal:** Unlock OGV output and image sequence output (PNG/JPEG/TIFF sequences), both currently `building`.

---

## TASK-C1: OGV (Ogg/Theora) output

**Scope:** Enable OGV as a live output format (currently `building: true`).

**What to do:**
- Check CI: `ffmpeg -codecs | grep libtheora` — known env-blocked on some runners (see SESSION-STATUS)
- If present: set `ogv` to `live: true`, wire `-c:v libtheora -c:a libvorbis` in `convert/video.rs`
- Add quality options (quality 0–10 maps to `-q:v`)
- Add sweep cases; if env-blocked mark in INVESTIGATION-LOG (same pattern as existing HAP/libtheora entries)

**Done when:** OGV output passes sweep or is documented as env-blocked with diagnostic. CI green.

---

## TASK-C2: PNG image sequence output from video

**Scope:** Export video frames as a numbered PNG sequence.

**What to do:**
- Add `png_sequence` as output target in video converter
- Wire FFmpeg `-vf fps=<N> -f image2 out_%04d.png` into `convert/video.rs`
- UI: expose FPS selector and output directory (sequences go to a folder, not a single file)
- Handle output naming: `<output_name>_%04d.png` in the validated output name
- Add sweep cases (short fixture → 5-frame PNG sequence)

**Done when:** Sequence output produces correct frame count. Sweep passes. CI green.

---

## TASK-C3: JPEG and TIFF image sequence output from video

**Scope:** Same as C2 but JPEG and TIFF formats.

**What to do:**
- Extend sequence output from C2 to support `-f image2` with `.jpg` and `.tiff` extensions
- JPEG: add quality slider (1–31 scale for FFmpeg `-q:v`)
- TIFF: no additional options needed (lossless by default)
- Add sweep cases

**Done when:** JPEG and TIFF sequences produce correct output. CI green.

---

## TASK-C4: Video codec expansion — AVC-Intra, XAVC, XAVC Long GOP

**Scope:** These are currently `todo: true` placeholders that fall back to H.264. Decide: implement or remove.

**What to do:**
- **AVC-Intra**: Panasonic intra-frame variant; requires specific profile/level params in libx264. Research if clean wrapper is feasible. If not, remove from picker entirely.
- **XAVC / XAVC Long GOP**: Sony camera codec; FFmpeg has no dedicated XAVC encoder — these are MXF-wrapped H.264/HEVC. Assess if `-f mxf -c:v libx264` with correct profile emulates XAVC sufficiently. If not, remove.
- Decision: implement real wrappers OR remove from picker and update README (already removed in README update 2026-05-07).

**Done when:** Each format either has a real tested implementation or is removed from the picker. CI green.
