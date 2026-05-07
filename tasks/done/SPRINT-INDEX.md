# Fade — TODO Sprint Index

Generated: 2026-05-07. Covers all scaffolded/todo-flagged features from the implementation audit.

---

## Sprints

| Sprint | File | Focus | Tasks | Dependencies |
|--------|------|-------|-------|--------------|
| A | [SPRINT-A-image-formats.md](SPRINT-A-image-formats.md) | Image format expansion | A1–A7 | ImageMagick in PATH |
| B | [SPRINT-B-audio-formats.md](SPRINT-B-audio-formats.md) | Audio format expansion | B1–B4 | FFmpeg codec availability |
| C | [SPRINT-C-video-expansion.md](SPRINT-C-video-expansion.md) | Video formats + sequences | C1–C4 | FFmpeg, libtheora env |
| D | [SPRINT-D-office-formats.md](SPRINT-D-office-formats.md) | Office document conversion | D1–D6 | LibreOffice headless |
| E | [SPRINT-E-3d-formats.md](SPRINT-E-3d-formats.md) | 3D format expansion | E1–E4 | Blender ≥ 3.5, FreeCAD |
| F | [SPRINT-F-optical-media.md](SPRINT-F-optical-media.md) | DVD / Blu-ray rip + author | F1–F4 | HandBrakeCLI, dvdauthor |
| G | [SPRINT-G-ai-tools.md](SPRINT-G-ai-tools.md) | AI tools (local, offline) | G1–G5 | Python, demucs, whisper, rembg |
| H | [SPRINT-H-chroma-archive-misc.md](SPRINT-H-chroma-archive-misc.md) | Neural matte, archives, inserts | H1–H6 | RVM, CorridorKey, 7z |

**Total tasks: 35**

---

## Suggested order

1. **Sprint A** — highest user value, lowest risk. ImageMagick already wired. No new infrastructure.
2. **Sprint B** — FFmpeg-only. Fast wins. AIFF and EAC3 likely zero-friction.
3. **Sprint C** — OGV env-dependent (may stay blocked). Sequence output (C2/C3) is independent and straightforward.
4. **Sprint F (F4 only)** — Web Video preset is a 1-task win, zero new infrastructure.
5. **Sprint D** — Needs LibreOffice; build the detection infra (D1) first, then unblock D2–D6 in parallel.
6. **Sprint E** — Blender path detection already done (TASK-29). E1–E3 are mostly script additions.
7. **Sprint H (H3–H6)** — Archive and video inserts are straightforward; neural matte/CorridorKey (H1–H2) are bigger lifts.
8. **Sprint G** — Most complex. All require managed Python installs. Build managed install framework once, reuse across G1–G5.
9. **Sprint F (F1–F3)** — DVD/Blu-ray rip requires HandBrake; authoring requires dvdauthor. Lower priority, niche use.

---

## CI pattern for all sprints

Every task follows the same gate:
1. Feature works locally
2. `full_sweep` or format-specific sweep updated with new cases
3. Cases pass (or are documented as env-blocked in INVESTIGATION-LOG)
4. CI green on `main`
5. SESSION-STATUS updated
