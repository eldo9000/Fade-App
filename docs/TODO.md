# Fade — TODO

Beta punch list. Edit in place. Nothing here is dated or phased — just shrink the list.

---

## Must close before shipping

- [ ] Visual verify new shared components in native build — Checkbox, SectionLabel, SegmentedControl, Select (light + dark mode)
- [ ] Sweep re-baseline after Sprint A–H — `image_full`, `full_sweep`, `extra_sweep` all need a fresh run against the expanded format set. Prior baseline (0.6.6) predates 35 new features.
- [ ] Update CHANGELOG.md `[Unreleased]` section for next version cut — Sprint A–H features not yet logged there.
- [x] Blender version check — `check_blender_version()` added; returns clear error if Blender < 3.0 (e21daf8)
- [x] h265-lossless guard — `effective_pix_fmt` forces `yuv444p` + `main444-8` when crf=0 (08f4dbe)
- [x] DNxHD resolution guard — passthrough path probes input dims via `probe_video_dimensions()` (631f0f4)
- [x] Fill `CHANGELOG.md [Unreleased]` before next version cut — filled (b7fdbb1)
- [x] Bump CI actions to Node 24 — done (b7fdbb1)

---

## Env-blocked (needs toolchain fix, not code)

- [ ] **hap** — video codec. Absent from Homebrew FFmpeg 8.1; no `--enable-hap` in brew configure flags. Needs custom FFmpeg build or bundled binary.
- [ ] **ogv / theora** — wire is in place (Sprint C); env-blocked on Homebrew FFmpeg 8.1 (libtheora absent). Will unblock when Homebrew formula includes it.

---

## Needs UI wiring (backend done — frontend pages not yet connected)

- [x] **Office conversion** — `convert/document.rs` LibreOffice pipeline implemented
- [x] **AI tools** — `operations/ai_tools.rs` demucs/whisper/argostranslate/ddcolor/rembg implemented
- [x] **DVD/Blu-ray rip** — `operations/dvd_rip.rs` HandBrakeCLI commands implemented
- [x] **DVD authoring** — `operations/dvd_author.rs` dvdauthor+mkisofs pipeline implemented
- [x] **Video inserts** — `operations/video_inserts.rs` FFmpeg complex-filter approach implemented
- [x] **Subtitle burn/embed/shift** — `operations/subtitle_ops.rs` all three ops implemented
- [x] **Neural matte** — `run_neural_matte` RVM command implemented
- [x] **3D format picker** — USD/USDZ/Alembic/.blend/STEP/IGES live in `App.svelte`

Frontend work still needed for all of the above — dedicated options panels, input pickers, and parameter controls.

---

## Implemented (Sprint A–H, 2026-05-07)

- [x] **Image formats** — GIF, ICO, SVG, HEIC/HEIF, JPEG XL, PSD, EXR, HDR, DDS, XCF, RAW camera (CR2/CR3/NEF/ARW/DNG/ORF/RW2) — all live via ImageMagick; RAW decode via dcraw if present
- [x] **Audio formats** — AIFF, Vorbis, EAC3 (Dolby Digital+), TrueHD — wired in `convert/audio.rs`
- [x] **Video sequences** — PNG/JPEG/TIFF frame sequences wired; OGV/Theora wire landed (env-blocked)
- [x] **XAVC/AVC-Intra stubs** — removed from picker (no viable FFmpeg encoder path)
- [x] **Office docs** — LibreOffice headless pipeline; DOCX/DOC/RTF/ODT/XLSX/XLS/ODS/PPTX/PPT/ODP/iWork/MSG all routed through `convert/document.rs`
- [x] **3D formats** — USD/USDZ (Blender 3.5+ guard), Alembic, .blend native I/O, STEP/IGES via FreeCAD
- [x] **Optical media** — DVD rip (HandBrakeCLI), DVD authoring (dvdauthor+mkisofs), Blu-ray rip, Web Video preset (H.264 + faststart)
- [x] **AI tools** — audio separation (demucs), transcription (whisper), translation (argostranslate), colorize (ddcolor), background removal (rembg) — all local/offline
- [x] **Neural matte** — RVM frame-by-frame matting with alpha output
- [x] **CorridorKey** — command slot wired; returns "not yet available" until standalone CLI ships
- [x] **Archive extras** — ISO (7z), DMG (hdiutil on macOS), CBZ creation (zip crate)
- [x] **Video inserts** — replace segment of video with insert clip, original audio preserved
- [x] **Subtitle ops** — burn (ffmpeg -vf subtitles), embed (stream copy + subtitle track), shift (pure Rust SRT timing adjustment)
- [x] **Web Video preset** — one-click H.264 + AAC + faststart, 1080p max
