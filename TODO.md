# Fade — TODO

Beta punch list. Edit in place. Nothing here is dated or phased — just shrink the list.

---

## Must close before shipping

- [ ] Visual verify new shared components in native build — Checkbox, SectionLabel, SegmentedControl, Select (light + dark mode)
- [ ] Blender version check — currently silent when wrong Blender version is on PATH; add check in `find_blender()` before invoking
- [ ] h265-lossless guard — no encoder constraint for lossless + h265; document the limitation in-UI or add a guard to match the h264 treatment
- [ ] DNxHD resolution guard — guard in `convert/video.rs` only fires when `opts.resolution` is explicitly set; unconditional check still missing for the passthrough path
- [ ] Fill `CHANGELOG.md [Unreleased]` before next version cut
- [ ] Bump CI actions to Node 24 (`actions/cache`, `actions/checkout`, `actions/setup-node`) — GitHub forces this June 2026

---

## Decide: in or out

Each item is a binary. Pick one outcome, then either implement it or remove it from the picker.

- [ ] **aiff** — audio output. FFmpeg supports it; likely low-effort to wire up and sweep-verify.
- [ ] **ogv / theora** — video + codec. Requires non-standard FFmpeg build (theora absent from Homebrew 8.1). Probably drop unless there's a real user need.
- [ ] **hap** — video codec. Same blocker as theora (non-standard FFmpeg). Probably drop.
- [ ] **Image sequences** (`seq_png`, `seq_jpg`, `seq_tiff`) — video → frame extraction. Meaningful backend work; needs new arg-builder path and sweep coverage.
- [ ] **Managed-install formats** — `font`, `parquet`, `ipynb`, `timeline`, `usd`/`usdz`/`abc`/`blend` — all require external tools installed separately. Decide whether to gate behind a dependency check or drop from the picker.
- [ ] **Archive extras** — `iso`, `dmg`, `cbr`/`cbz`/`rar` — uncommon enough to drop; keep only if there's a clear use case.
