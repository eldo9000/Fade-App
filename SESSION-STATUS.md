# Fade — Session Status

Last updated: 2026-04-22

---

## Current Focus

Four arcs landed since the B1–B18 audit closed:

1. **UI polish sweep** — canonical `seg-active`, `fade-check`, `fade-range`, `--surface-hint`, `space-y-3`, `py-[5px]` patterns propagated across AudioOptions, ImageOptions, ArchiveOptions, DataOptions, FormatPicker. Shared `src/lib/segStyles.js` replaces per-file `seg`/`segV` duplicates.
2. **VideoOptions overhaul** — collapsible Advanced fold; categorized codec dropdown (Common/Professional/Broadcast/Archival/Legacy); CRF/VBR/CBR mode switcher; inverted quality slider (Worst→Lossless); ProRes profile picker exposed; image sequence export (`seq_png`, `seq_jpg`, `seq_tiff`); AudioOptions Length accordion (Trim + Silence Padding). `overlay.svelte.js` portal-style dropdown store added, renders at App root to escape overflow/stacking context.
3. **Blender headless backend** — USD/USDZ/Alembic/Blend conversion via headless Blender + bundled `blender_convert.py`. STEP/IGES explicitly deferred.
4. **B16 phase 2 — async IPC migration (7 tasks, CI-green)** — all 14 analysis/probe/preview IPC commands converted to non-blocking. Key deliverables: `ConvertResult` typed enum replaced string sentinels across 14 converter modules (TASK-2); 6 fast probe commands (`get_file_info`, `get_streams`, `subtitle_probe`, `diff_images`, `lint_video`, `get_image_quality`) migrated to async via `tokio::task::spawn_blocking` (TASK-3); 8 long-running commands migrated to full job-based lifecycle with cancellation and `analysis-result:{job_id}` events (TASKs 4–5); `createLimiter.run()` gained optional `timeoutMs` parameter with guaranteed slot release in `finally` (TASK-6); 16 unit tests added to `args/video.rs` covering VBR/CBR paths and image sequence arg builder (TASK-7).

CI green on `main`.

## Next action

**GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` directly into `run:` steps. Not exploitable without repo write access, but a hygiene issue. Fix: use an intermediate env var to pass the tag value rather than interpolating directly into the shell command.

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

- **`$bindable` chain through extracted components.** App.svelte → QueueManager → OperationsPanel / ChromaKeyPanel mutate `selectedItem.status`/`.percent`/`.error` in place during job execution. Any future change that accidentally converts `selectedItem` to a read-only prop will silently stop progress updates.
- **Blender backend: `blender_convert.py` path resolution at runtime is fragile.** See KNOWN-BUG-CLASSES BC-003/BC-004. Binary discovery and script path construction are not hardened for all deployment contexts. USD import empty-scene silent success remains unmitigated.
- **analysis-result one-shot listener race.** The one-shot event listener introduced in TASKs 4–5 is set up before the invoke call; if the event fires before `unlisten` is registered on a very fast completion, the result may be missed. Structurally possible, not yet observed.

**Lower-urgency known gaps (deferred):**
- **AudioOffset i64→i32 precision drift** — ts-rs generates `bigint` for `offset_ms`; frontend passes `number`. Correctness issue at the IPC boundary for values above 2^53.
- **Windows non-C drive preview** — `assetProtocol.scope` too narrow for secondary drives. Affects Windows users only.
- **GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` directly into `run:` steps. Not exploitable without repo write access, but a hygiene issue.

## Mode

Active development. B16 phase 2 complete. Next: AudioOffset precision drift micro-patch.
