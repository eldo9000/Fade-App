# Fade-App Observer State
Last updated: 2026-04-23  ·  Run 5

---

## Active development areas

Run 4 declared the project in its strongest post-audit state with no feature work in flight. That lasted roughly one day. A significant feature sprint has since landed across two areas.

First, a full Blender headless backend (`9b13f41`) added USD, USDZ, Alembic, and Blend conversion routes using a bundled `blender_convert.py` script invoked via Tauri's sidecar path. STEP/IGES were explicitly deferred as requiring a different toolchain (FreeCAD). This is a net-new backend pathway with its own fragilities (see below).

Second, a broad VideoOptions overhaul (`4bd1839`) landed: collapsible Advanced fold exposing only Codec, Resolution, and Quality at top level; a fully categorized codec dropdown (Common/Professional/Broadcast/Archival/Legacy groups with named headers); CRF/VBR/CBR quality mode switcher (mode picker buried in Advanced, quality slider promoted to top level); inverted CRF slider (left=Worst, right=Lossless); ProRes profile picker exposed directly in the visible section when ProRes is selected; and image sequence export targets (`seq_png`, `seq_jpg`, `seq_tiff`) including backend directory creation and frame pattern FFmpeg output. The VBR/CBR paths add new `video_bitrate_mode` and `video_bitrate` fields to `ConvertOptions`. The sequence path adds `prores_profile`. A companion change folded Trim and Silence Padding into a "Length" accordion inside AudioOptions.

An overlay store (`src/lib/stores/overlay.svelte.js`) was added during this work — a portal-style shared dropdown state that renders at the App root to escape the panel overflow/stacking context. It carries a back-compat shim (`overlay.show`/`overlay.hide` aliased to `showOverlay`/`hideOverlay`), suggesting the API shape evolved mid-work.

A UI polish sweep across AudioOptions, ImageOptions, ArchiveOptions, DataOptions, and FormatPicker preceded the VideoOptions work, propagating the canonical `seg-active` underline, `fade-check` tiles, `fade-range` accent fill, `--surface-hint` token, and spacing conventions. That work is complete and CI-green.

## Fragile / high-risk areas

The **Blender backend** carries the same runtime path fragility flagged in KNOWN-BUG-CLASSES BC-003/BC-004: `blender_convert.py` script path resolution at runtime is the named fragility. USD import empty-scene silent success (BC-004) is also active and unresolved.

The **image sequence path dispatch** in `lib.rs` introduces a new branching path: `ext.strip_prefix("seq_")` triggers directory creation at IPC time (before thread spawn), then the args builder appends the `frame_%04d.ext` pattern to the directory path. If any intermediate step (directory creation, path construction) fails, the error surfaces before conversion begins — which is correct — but the code path is new and untested by the existing backend unit test suite. The MJPEG quality mapping (`q:v` 2–31 from CRF 0–51) in `build_image_sequence_args` is a new non-trivial mapping with no unit coverage.

The **VBR/CBR codec paths** in `codec_quality_args` are new. The CBR path sets `minrate`, `maxrate`, and `bufsize` all equal to the target bitrate — correct for strict CBR — but this combination is known to cause issues with some container formats and decoders. It has not been exercised in CI.

The **`$bindable` chain risk** from Run 4 — App.svelte → QueueManager → OperationsPanel/ChromaKeyPanel mutating `selectedItem.status`/`.percent`/`.error` in place — persists unchanged. The VideoOptions overhaul did not touch this pathway.

The **`createLimiter` slot-leak** from Run 4 persists. No timeout or drain valve was added during the feature sprint.

The overlay back-compat shim (`overlay.show`/`overlay.hide`) suggests there may be callers using the old API shape. If those callers exist in the codebase and the shim is later removed, they will break silently at runtime.

## Deferred work accumulation

The six items formally promoted at audit close (Run 4) remain untouched:

1. **B16 phase 2 / B19** — async lifecycle for 14 sync analysis/probe/preview IPC commands. Largest single outstanding debt.
2. **AudioOffset i64→i32 precision drift** — ts-rs generates `bigint` but frontend passes `number`. Micro-patch.
3. **Windows non-C drive preview** — `assetProtocol.scope` too narrow for secondary drives.
4. **Slot-leak watchdog for `createLimiter`** — no drain valve for unreleased semaphore slots.
5. **`librewin_common` superset-vs-authoritative strategy** — `StoredPreset` workaround is implicit.
6. **GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` directly into `run:` steps.

The Blender backend explicitly deferred STEP/IGES support (requires FreeCAD or alternate toolchain). This has not been added to any formal deferred ledger.

The VideoOptions CRF/VBR/CBR implementation deferred quality mode support for codecs other than h264/h265 — VP9, AV1, and other codecs fall through to the existing CRF-only path silently. If a user selects VBR/CBR with VP9 or AV1, the mode picker will have no effect.

## Pattern watch

The **CI two-step failure pattern** recurred this session: the main feature commit (`4bd1839`) failed on `cargo fmt --check` (line-length violations in new CBR args and sequence path code); the formatting fix (`8f5946b`) then failed on Clippy (`map_identity` at video.rs:524, `manual_strip` at lib.rs:741–745). Three commits were needed to achieve a green run. This is the same pattern as the B18 audit close (where the wrap commit failed fmt and was fixed in a follow-up). The root cause is consistent: handwritten Rust that isn't run through `cargo fmt` before commit, combined with Clippy lints that aren't caught locally when warnings are treated as errors only in CI. Both failures were caught and fixed within minutes, but the pattern has now occurred twice.

The **traversal-gate pattern** (B15, flagged in Run 4) has a new surface to check: the image sequence IPC path in `lib.rs` validates `ext` via `is_ascii_alphanumeric() || c == '_'` inline (not via the canonical `validate_output_name`/`validate_no_traversal` trio). The validation is sufficient for ext-only inputs but the divergence from the canonical pattern is worth noting.

BC-001 (inverted SVG chevrons) and BC-002 (audio analysers black before first playback) continue to be listed as active in KNOWN-BUG-CLASSES despite Run 3 noting user-reported resolution. No update was made during this sprint.

## CI health

`main` is green as of `034ebf7`. The current session produced two CI failures before landing green — see Pattern watch above. Prior to this session the last CI failure was `daa854f` (audit wrap), also a formatting issue. The net pattern is: CI fails on formatting/linting approximately once per major feature session, then recovers in under 30 minutes. No test failures. No flakiness.

## Observer notes

**Run 5 vs Run 4 — feature velocity resumed.** Run 4 closed a deep audit arc and flagged B16 phase 2 as the next highest-leverage investment. Instead, a feature sprint landed — VideoOptions overhaul, Blender backend, UI polish. This is not a problem, but it means the deferred items from Run 4 continue to age. B16 phase 2 in particular is the kind of work that compounds: every new analysis/probe command added while sync IPC is the norm extends the eventual async migration effort.

**SESSION-STATUS is stale.** It still reflects the audit close state (2026-04-22 with B16 phase 2 as next action). The VideoOptions overhaul, Blender backend, and UI polish work are not reflected. The "Mode: Stable" declaration predates the feature sprint.

**Git note protocol remains cold.** Of the last 20 commits, only `99144ed` (fmt fix) and `9b13f41` (Blender backend) carry structured notes. The entire VideoOptions feature sprint — 9 commits from `e26b44f` (audio-options polish) through `034ebf7` (clippy fixes) — shipped without notes. The deferred/fragile fields on the Blender backend note were filled correctly; that discipline did not carry through to the VideoOptions work, which introduced more new surface area.

**INVESTIGATION-LOG is stale.** No entries have been added since 2026-04-20 (the component extraction arc). The VideoOptions work involved multiple architectural decisions (sequence output path dispatch, CRF slider inversion, overlay portal design) that would normally warrant log entries if issues had arisen. None did, which is the correct reason for no entries — but the log format being entirely bypassed during a feature sprint is a protocol gap worth noting.

**overlay.svelte.js back-compat shim.** The `overlay.show`/`overlay.hide` aliases exist "in case callers still use" the old shape — the comment suggests the API was renamed mid-session. Worth checking whether any live callers still reference the old shape; if not, the shim can be removed.
