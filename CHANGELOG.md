# Changelog

All notable changes to Fade will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Shared `@libre/ui` design system: Checkbox, SectionLabel, SegmentedControl,
  and Select components. CSS primitives (`--accent`, `--surface-hint`,
  `.fade-check`, `.seg-active`/`.seg-inactive`) lifted from Fade-local into the
  shared token layer so all Libre apps share the same design system.
- Format picker state flags (`live` / `building` / `todo`) on all format groups.
  `building` formats are hidden from the picker until ready; `live` reflects
  sweep-verified formats as of the 0.6.x validation passes.
- Email conversion (`eml`, `mbox`) promoted to live — validated end-to-end via
  extra_sweep.
- Full test sweep infrastructure: `matrix.rs` (33-case smoke matrix, pre-release
  gate), `full_sweep.rs` (~700-case Cartesian diagnostic), `extra_sweep.rs`
  (3D model, subtitle, email, document). All `#[ignore]` — manual only.
- `&Window`-decoupled `convert()` functions across all 15 conversion modules with
  a `convert::progress::ProgressFn` contract, enabling direct test invocation via
  `noop_progress()`.
- `JobOutcome` typed enum — replaces string sentinels `"CANCELLED"` / `"__DONE__"`.
- ts-rs codegen: 12 TypeScript types generated from Rust structs at build time
  (eliminates hand-maintained TS/Rust type drift).
- `TODO.md` beta punch list.

### Changed

- AV1 encoder switched from `libaom-av1` to `libsvtav1` (Homebrew FFmpeg 8.1
  compatibility). `av1_speed` remapped from `-cpu-used` to `-preset` with
  0–10 → 0–13 scaling.
- `run_ffmpeg` consolidated from 3 diverged copies into 1 canonical function
  with a rate-limiter.
- `createLimiter` batch concurrency clamped to `hardwareConcurrency` (was
  unbounded — up to 100 simultaneous ffmpeg processes on large queues).
- Streaming waveform RMS: O(file-size) → O(n) memory for `get_waveform`.
- `parking_lot::Mutex` applied consistently across 32 files; return-shape drift
  normalised.

### Fixed

- H.265 profile names: added `h265_effective_profile()` and split the h264/h265
  arg-builder branch — libx265 was receiving h264 profile names, causing 27
  sweep failures.
- H.264 lossless (crf=0): force `yuv444p` + `high444` profile — closed 120
  sweep failures.
- DNxHD/DNxHR resolution guards: sub-1280×720 inputs return a clear error;
  `convert()`-only contract documented for the passthrough path.
- AVIF speed cap: clamped to 9 in arg builder and UI slider.
- 7zz tar.gz/tar.xz repack: two-step `repack_tar_compressed` function added.
- `$bindable()` defaults removed from all conditionally-rendered Svelte 5
  components — eliminating the write-back cascade that collapsed component
  subtrees on initialisation.
- Image stderr drain: background thread added to prevent ffmpeg deadlocking on
  full stderr pipe.
- Merge race: deterministic concat-list cleanup after `run_ffmpeg` completes.

### Security

- `validate_input_path()`: traversal check + allowed-roots constraint wired
  into all frontend-supplied read paths across IPC entry points.
- Per-job `mkdtemp 0700` sandbox for renderer-facing temp files and all
  medium-priority conversion temp sites.
- Zip-slip containment, subtitle filter argument escaping, archive portability
  hardening.
- DuckDB SQL path character validation before interpolation.
- `file_exists` and `scan_dir` scoped to allowed filesystem roots.
- `rustls-webpki` resolved to 0.103.13 (patched version).
- Cancel race fix in audio/image/video convert modules: cancellation arriving
  between process spawn and map-insert now kills the child immediately.

### Infrastructure

- Backend integration tests (conversion smoke) added to CI gate.
- E2E Playwright component tests added to CI gate.
- CI and release workflows opt into Node 24 runner via
  `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24` (GitHub-forced June 2026).

## [0.2.2] - 2026-04-18

### Added

- 3D model conversion via `assimp` CLI. Input + output formats: OBJ, STL,
  PLY, glTF, GLB, COLLADA (.dae), FBX (ASCII only — binary FBX requires
  the proprietary Autodesk SDK), 3DS, X3D. Binary variants preferred
  where available (STL → stlb, PLY → plyb, glTF → glb2). Requires
  `assimp` on PATH; `.deb`/`.rpm` depends now include `assimp-utils` /
  `assimp`. macOS/Windows users install via brew/scoop.
- Proxy folders, quick output controls, separator in output naming.

### Changed

- Flattened `.btn-bevel` (removed hard-edge doubling shadow + :active
  translate), promoted the format chip in the convert bar to match.
- Version stamp in the right-panel footer pulses slowly between 20%
  white and accent (20s cycle).

### Infrastructure

- Auto-cleanup workflow deletes GitHub releases older than 10 days
  (weekly Sunday 09:00 UTC). Latest release always preserved so the
  in-app updater never 404s. Artifact retention lowered 7d → 3d.

## [0.2.1] - 2026-04-18

### Added

- Silence padding controls on audio output (SilencePad sliders, 0–60s
  exponential response) — prepends/appends silence via ffmpeg
  `adelay`/`apad`.
- Waveform zoom (1–20× via mouse wheel) and middle/right-click pan,
  smoothed. Waveform re-fetches at 4000 buckets for sharp detail at all
  zoom levels; opacity ramps up when zoomed out.
- Timeline silence regions render inline with the waveform, interpolated.
- Backend unit test coverage across `args/image`, `args/video`,
  `presets`, `fs_commands`, `probe/waveform`, `preview/image_quality`,
  `preview/video_diff` (~67 new tests).
- User-facing install guide (`docs/INSTALL.md`) and contributor guide
  (`CONTRIBUTING.md`).
- `CHANGELOG.md`, GitHub issue/PR templates.

### Changed

- Trim and fade handles now extend into silence regions and block each
  other as a chain with no gap.
- Video audio control collapsed to a single "Remove audio" checkbox.
- CRF default for video encoding: 23 → 20 (slightly higher quality).

### Fixed

- `webm_video_bitrate` now correctly overrides the audio bitrate in CBR
  and CVBR modes.

### Infrastructure

- Release workflow now builds a 3-platform matrix: macOS (aarch64),
  Linux (x86_64 — .deb + AppImage), Windows (x86_64 — .msi + NSIS).
  `latest.json` covers all three updater channels.
- CI workflow runs on every PR: vitest, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and backend unit tests
  on macOS aarch64.
- Release signing key handling hardened: secret is base64-armored to
  survive GitHub's secret UI pasting, decoded once in CI to the 348-byte
  minisign keyfile.

## [0.2.0] - 2026-04-18

First tagged release of Fade as a standalone app, split out of the Libre-Apps
monorepo. Delivers a Tauri v2 macOS media converter with audio, video, image,
data, document, and archive conversion, plus a redesigned queue-driven UI.

### Added

- Audio conversion pipeline with DSP, visualizers, and loudnorm LUFS/TP controls.
- Video, image, data, document, and archive conversion formats.
- Video and image preview, compression diff view, and crop UI.
- Audio timeline with scrubbing, waveform, and filmstrip scrubber above the
  waveform for video files.
- Per-format output controls and expanded output picker covering the full format
  matrix, with an output-format-driven right panel.
- Queue thumbnails, hover info popover, compact queue toggle, and queue
  deselect.
- Sidebar settings panel with Updates, UI, and Data sections.
- Tauri auto-updater (unsigned distribution) wired into the app.
- Preserve-metadata toggle, shared tooltip store, and zoom cursor-warp.
- Home panel zoom controls, wider scrollbars, and VU clip indicators.
- Convert-to-top-right flow, columnar format picker, and crossfade tooltip.
- File-extension column shown separately in the queue list.
- Per-format webm video bitrate field.

### Changed

- Major UI pass: convert flow polish, queue redesign, bottom panel controls,
  brighter grid lines, darker background.
- Extracted Svelte stores and a `FormatPicker` component; redesigned queue and
  progress surfaces.
- Split the Rust backend `lib.rs` into `args`, `convert`, `probe`, and `preview`
  modules.
- Sequential load pipeline and stereo-width DSP fix.
- Filmstrip load deferred until playback is ready; ffmpeg runs at low priority;
  frames stream via events with one ffmpeg call per frame; `skip_frame noref`
  and `fast_bilinear` scale for faster decode.
- README rewritten around the "utility knife" concept for the public landing.

### Fixed

- Queue popover flushed to sidebar edge; speech-bubble arrow removed; toggle
  icons corrected; standalone info modal removed.
- `get_waveform` and `get_filmstrip` accept a `draft` param.
- Clippy: collapsed nested if-let and removed unit let-bindings.
- Jsdom localStorage test for webm video bitrate field.
- Added `icon.ico` required for the Windows resource file.

### Infrastructure

- macOS release workflow with updater artifacts; cross-platform build workflow.
- Release trigger switched to `workflow_dispatch` with a version input.
- Private cargo dep `librewin-common` authenticated via `LIBRE_APPS_TOKEN`
  (oauth2 PAT URL form).
- CI clippy, cargo check, and npm build gates; npm cache dropped (no lock file).
- Tauri signing key handling hardened: passed as file path, stripped to base64
  charset, require armored secret, unset `TAURI_SIGNING_PRIVATE_KEY` before
  invoking the signer.
- Bumped to v0.2.0.

[Unreleased]: https://github.com/eldo9000/Fade-App/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/eldo9000/Fade-App/releases/tag/v0.2.2
[0.2.1]: https://github.com/eldo9000/Fade-App/releases/tag/v0.2.1
[0.2.0]: https://github.com/eldo9000/Fade-App/releases/tag/v0.2.0
