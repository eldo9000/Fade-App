# Fade — Session Status

Last updated: 2026-04-22

---

## Current Focus

Three arcs landed since the B1–B18 audit closed:

1. **UI polish sweep** — canonical `seg-active`, `fade-check`, `fade-range`, `--surface-hint`, `space-y-3`, `py-[5px]` patterns propagated across AudioOptions, ImageOptions, ArchiveOptions, DataOptions, FormatPicker. Shared `src/lib/segStyles.js` replaces per-file `seg`/`segV` duplicates.
2. **VideoOptions overhaul** — collapsible Advanced fold; categorized codec dropdown (Common/Professional/Broadcast/Archival/Legacy); CRF/VBR/CBR mode switcher; inverted quality slider (Worst→Lossless); ProRes profile picker exposed; image sequence export (`seq_png`, `seq_jpg`, `seq_tiff`); AudioOptions Length accordion (Trim + Silence Padding). `overlay.svelte.js` portal-style dropdown store added, renders at App root to escape overflow/stacking context.
3. **Blender headless backend** — USD/USDZ/Alembic/Blend conversion via headless Blender + bundled `blender_convert.py`. STEP/IGES explicitly deferred.

CI green on `main`.

## Next action

**B16 phase 2** — convert 14 sync analysis/probe/preview IPC commands to non-blocking. Batched across 3 sessions (3–5 commands per batch). See `tasks/TASK-3-async-probe-spawn-blocking.md` through `TASK-5`.

## Audit outcome summary

**33 findings closed across 18 batches (B1–B18).** Key structural wins:

- `JobOutcome` typed enum replaced string sentinels `"CANCELLED"` / `"__DONE__"` (B11)
- ts-rs codegen: 12 TypeScript types generated at build time from Rust structs (B17)
- Streaming waveform RMS: O(file) → O(n) memory for `get_waveform` (B12)
- `run_ffmpeg` consolidated from 3 diverged copies to 1 canonical with rate-limiter (B8)
- `createLimiter` batch concurrency semaphore: 100 unbounded ffmpegs → clamped to `hardwareConcurrency` (B10)
- validate_output_name umbrella covering all 29 `OperationPayload` variants (B15)
- parking_lot::Mutex across 32 files, return-shape drift normalized (B18)

**6 deferred items** promoted to followup list (see `audits/04-attack-plan.md §7`):
- B16.2 / B19: async lifecycle for 14 analysis/probe/preview commands
- AudioOffset.offset_ms i64→i32 precision drift (ts-rs)
- Windows non-C: drive preview (assetProtocol runtime allow_file)
- Slot-leak watchdog for createLimiter
- librewin_common superset-vs-authoritative strategy
- GHA shell injection hardening (release.yml inputs.tag)

**Verify pass:** cargo-audit RUSTSEC-2025-0067/0068 (serde_yml/libyml) confirmed absent. All other advisories unchanged (Linux-only transitive). Semgrep: 20 INFO-only findings, all temp-dir, all uuid-namespaced.

## Known Risks

- **B16 phase 2 not landed.** 14 analysis/probe/preview commands remain synchronous. They block the IPC thread for their full duration and are uncancellable. Acceptable for current feature state; must land before any heavy-use release.
- **`$bindable` chain through extracted components.** App.svelte → QueueManager → OperationsPanel / ChromaKeyPanel mutate `selectedItem.status`/`.percent`/`.error` in place during job execution. Any future change that accidentally converts `selectedItem` to a read-only prop will silently stop progress updates.
- **`createLimiter` slot-leak.** No drain valve if a terminal job event is missed. A stalled job can permanently consume a concurrency slot, blocking the queue.
- **VBR/CBR and image sequence backend paths have no unit test coverage.** New VideoOptions quality modes and `seq_*` formats are exercised only by manual testing.
- **Blender backend: `blender_convert.py` path resolution at runtime is fragile.** See KNOWN-BUG-CLASSES BC-003/BC-004. Binary discovery and script path construction are not hardened for all deployment contexts.

## Mode

Active feature development. B16 phase 2 queued.
