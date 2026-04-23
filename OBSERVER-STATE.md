# Fade-App Observer State
Last updated: 2026-04-22  ·  Run 6

---

## Active development areas

The B16 phase 2 sprint completed in full this session — all 7 tasks dispatched, all CI-green. The work resolved the largest outstanding debt from the post-audit deferred list.

TASK-2 replaced the remaining string-sentinel bridge in the `convert/` path with a typed `ConvertResult` enum (`Done`, `Cancelled`, `Error(String)`), covering 14 converter modules. One sentinel remains intentionally in `operations/mod.rs:267` feeding the existing `op_result` bridge — this was explicitly scoped out.

TASK-3 converted 6 fast probe commands (`get_file_info`, `get_streams`, `subtitle_probe`, `diff_images`, `lint_video`, `get_image_quality`) from synchronous `#[tauri::command]` to async via `tokio::task::spawn_blocking`. This required adding an explicit `tokio` dependency to `Cargo.toml`, as Tauri 2 does not re-export it.

TASKs 4–5 migrated 8 long-running analysis/probe/preview commands to a full job-based lifecycle: `cancel_job` kills the child process; each command emits an `analysis-result:{job_id}` event instead of returning synchronously. The frontend gained an `invokeAnalysis` helper in `AnalysisTools.svelte` wrapping the listen+invoke pattern. Waveform streaming added cancellation checks inside the bucket-read loop.

TASK-6 added an optional `timeoutMs` parameter to `createLimiter.run()`. When provided, the task races against a timer; the `finally` block always releases the slot regardless of outcome. A factory-level `defaultTimeoutMs` option was also added.

TASK-7 added 16 unit tests to `args/video.rs` covering the VBR/CBR codec paths for h264/h265 and the image sequence arg builder (`seq_png`, `seq_jpg`, `seq_tiff`) including the JPEG CRF→q:v quality mapping formula and trim/framerate/no-audio assertions.

## Fragile / high-risk areas

The **Blender backend** fragilities (BC-003, BC-004) are unchanged. `blender_convert.py` path resolution at runtime remains the primary risk; USD import empty-scene silent success remains unmitigated. STEP/IGES remain deferred.

The **`$bindable` chain** — App.svelte → QueueManager → OperationsPanel/ChromaKeyPanel mutating `selectedItem.status`/`.percent`/`.error` in place — persists. No sprint work touched this pathway.

The **analysis-result event pattern** introduced in TASKs 4–5 is a new IPC surface. The one-shot event listener is set up before the invoke call; if the event fires before `unlisten` is registered (a race on a very fast completion), the result may be missed. This has not been observed but is structurally possible.

The **overlay back-compat shim** (`overlay.show`/`overlay.hide` aliases) flagged in Run 5 was not cleaned up. Run 5 noted the shim exists "in case callers still use" the old shape — if dead, it should be removed; if live, the old API is an unofficial surface.

The **missed TASK-5 commit** — vmaf.rs, spectrogram.rs, video_diff.rs — was not included in the TASK-5 sprint commit and had to be caught and committed separately as `6564d28`. The dispatcher staged only the files the worker explicitly listed in `files_changed`. This reflects a workflow gap: workers need to be explicit about all modified files.

## Deferred work accumulation

Three of the six formally promoted audit-close deferred items were resolved this sprint: B16 phase 2 / B19 (async IPC), slot-leak watchdog, and VBR/CBR unit coverage. Three remain:

1. **AudioOffset i64→i32 precision drift** — ts-rs generates `bigint` for `offset_ms` but frontend passes `number`. Micro-patch, no downstream breakage known.
2. **Windows non-C drive preview** — `assetProtocol.scope` too narrow for secondary drives. Affects Windows users only.
3. **GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` directly into `run:` steps. Not exploitable without repo write access, but a hygiene issue.

The `librewin_common` superset-vs-authoritative strategy question dropped from the active deferred list; no recent commits touched shared types or the `StoredPreset` workaround.

The Blender backend's STEP/IGES deferral and the VBR/CBR silent fallback for VP9/AV1 (mode picker has no effect on those codecs) remain untracked in any formal ledger.

SESSION-STATUS `## Next action` still reads "B16 phase 2 — convert 14 sync analysis/probe/preview IPC commands to non-blocking" and the Known Risks section still lists the createLimiter slot-leak and VBR/CBR coverage gaps as open. Both are now resolved. SESSION-STATUS requires a refresh.

## Pattern watch

The **CI two-step failure pattern** was absent from this sprint. All 5 CI runs in the sprint batch came back green on first push. The pattern last observed at Run 5 (two failures before green on `4bd1839`) did not recur. This may reflect the dispatch workflow enforcing local cargo fmt + clippy checks before each commit.

The **missed-files pattern** surfaced: the TASK-5 dispatcher committed only the worker-reported files, leaving vmaf.rs, video_diff.rs, and spectrogram.rs unstaged. This is the second instance of a multi-file backend task having partially incomplete commits — the first was the B18 parking_lot sweep. The pattern is: workers report `files_changed` honestly, but dispatchers stage only that list, and any files modified incidentally (e.g., by a prior failed attempt, or by a test run that writes artifacts) fall through.

BC-001 and BC-002 remain listed in KNOWN-BUG-CLASSES as active despite earlier indications of resolution. No update was made this session.

## CI health

`main` is green as of `19ca700` (last push, 2026-04-22). All 5 CI runs from this sprint completed with `success` in 1–2 minutes. No failures, no flakiness. This is the cleanest sprint run since the audit arc — zero red pushes.

## Observer notes

**Run 5 deferred items resolution rate: 3/6.** The three that landed were the highest-leverage ones (B16 phase 2, slot-leak, unit coverage). The three remaining are lower-urgency hygiene or platform-specific. The project is in materially better shape than it was at Run 5.

**SESSION-STATUS is stale.** The `## Next action` and `## Known Risks` sections reflect the pre-sprint state. B16 phase 2 is done; the createLimiter and VBR/CBR risks are closed. A SESSION-STATUS refresh is the most immediate protocol gap.

**Git note protocol remains cold.** None of the 7 sprint commits carry structured notes. The pattern from Run 5 — features landing without `deferred:/fragile:` fields — continued unbroken through this entire sprint. The Blender backend note (two sessions ago) remains the last commit with structured notes.

**CONDUCTOR-LOG.md committed to main.** This is a Blender headless implementation progress log, apparently generated during an earlier sprint. It is now tracked in the repo. Its content is useful for continuity but its presence in the root is unusual — most artifact files of this type would live under `docs/` or similar.
