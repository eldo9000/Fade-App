# Fade — Session Status

Last updated: 2026-04-20

---

## Current Focus

Housekeeping + App.svelte split sprint. TASK-6, TASK-5a, TASK-5b complete. Remaining: TASK-5c (OperationsPanel / AnalysisTools / ChromaKeyPanel).

## Next action

**Dispatch TASK-5c in a fresh agent session.** Brief at `tasks/TASK-5c-operations-panel.md`. Operation runners can now safely bind to `selectedItem` via QueueManager's $bindable prop.

## Known Risks

- **Release workflow still broken** — `v0.6.1` tag points at a commit where `src-tauri/tauri.conf.json` reads `0.6.0`. Deferred by user. When unfrozen, cut v0.6.2 bundling the Gatekeeper fix and bump config in the same commit as the tag.
- **`$bindable` chain fragility** — 5a established the pattern. If 5b or 5c breaks it (e.g., makes `selectedItem` read-only), operation progress updates silently stop. Manual smoke-test after each extraction is mandatory.
- **Async pipeline cancellation in 5b** — `_loadGen` generation counter in `runLoadPipeline` must be preserved exactly. Rapid file selection without it would attach the wrong waveform to the wrong file.

## Recent progress (2026-04-20)

- Node 20 → 22 in both CI workflows
- `serde_yaml` (abandoned) → `serde_yml` drop-in
- `librewin-common` pinned to tag `v0.1.3` (was bare git hash)
- Mutex `.unwrap()` → `.expect()` at 10 sites (TASK-6 ✓)
- Extracted UpdateManager, PresetManager, CropEditor (TASK-5a ✓) — App.svelte 6014 → 5569

## Mode

Pre-ship polish + architectural cleanup. No new features; focus is splitting the God component and removing dependency debt before 1.0.
