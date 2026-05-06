# Fade — TODO

Beta punch list. Edit in place. Nothing here is dated or phased — just shrink the list.

---

## Must close before shipping

- [ ] Visual verify new shared components in native build — Checkbox, SectionLabel, SegmentedControl, Select (light + dark mode)
- [x] Blender version check — `check_blender_version()` added; returns clear error if Blender < 3.0 (e21daf8)
- [x] h265-lossless guard — `effective_pix_fmt` forces `yuv444p` + `main444-8` when crf=0; mirrors h264 guard (08f4dbe)
- [x] DNxHD resolution guard — passthrough path now probes input dims via `probe_video_dimensions()`; guard fires for sub-1280×720 even when no explicit resolution set (631f0f4)
- [x] Fill `CHANGELOG.md [Unreleased]` before next version cut — filled (b7fdbb1)
- [x] Bump CI actions to Node 24 — `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` added to all three workflows (b7fdbb1)

---

## Decide: in or out

Each item is a binary. Pick one outcome, then either implement it or remove it from the picker.

- [ ] **aiff** — audio output. FFmpeg supports it; likely low-effort to wire up and sweep-verify.
- [ ] **ogv / theora** — video + codec. Requires non-standard FFmpeg build (theora absent from Homebrew 8.1). Probably drop unless there's a real user need.
- [ ] **hap** — video codec. Same blocker as theora (non-standard FFmpeg). Probably drop.
- [ ] **Image sequences** (`seq_png`, `seq_jpg`, `seq_tiff`) — video → frame extraction. Meaningful backend work; needs new arg-builder path and sweep coverage.
- [ ] **Managed-install formats** — `font`, `parquet`, `ipynb`, `timeline`, `usd`/`usdz`/`abc`/`blend` — all require external tools installed separately. Decide whether to gate behind a dependency check or drop from the picker.
- [ ] **Archive extras** — `iso`, `dmg`, `cbr`/`cbz`/`rar` — uncommon enough to drop; keep only if there's a clear use case.
