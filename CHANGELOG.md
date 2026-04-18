# Changelog

All notable changes to Fade will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Infrastructure

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

[Unreleased]: https://github.com/eldo9000/Fade-App/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/eldo9000/Fade-App/releases/tag/v0.2.1
[0.2.0]: https://github.com/eldo9000/Fade-App/releases/tag/v0.2.0
