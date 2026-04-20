# Fade — Session Status

Last updated: 2026-04-20

---

## Current Focus

Post-sprint stable state. All housekeeping and architectural cleanup from the 2026-04-20 arc has landed and shipped. v0.6.2 published with signed binaries for macOS (aarch64), Linux (x86_64), and Windows (x86_64).

## Next action

No outstanding work. Project is ready for either feature work toward 1.0 or continued pre-ship polish. No blockers, no known-red CI, no deferred items on the board.

## Known Risks

- **`$bindable` chain through the extracted components.** App.svelte → QueueManager → OperationsPanel / ChromaKeyPanel mutate `selectedItem.status`/`.percent`/`.error` in place during job execution. Any future change that accidentally converts `selectedItem` to a read-only prop along the chain will silently stop progress updates. This is the one lingering fragility from the split arc — worth flagging on any commit that touches operation runners or queue state.

## Recent progress (2026-04-20)

- Node 20 → 22 in both CI workflows
- `serde_yaml` (abandoned) → `serde_yml` drop-in
- `librewin-common` pinned to tag `v0.1.3` (cut a new tag in Libre-Apps)
- Mutex `.unwrap()` → `.expect()` at 10 sites (TASK-6)
- App.svelte 6,014 → ~3,100 lines across three dispatches (TASK-5a/5b/5c): extracted UpdateManager, PresetManager, CropEditor, QueueManager, ChromaKeyPanel, AnalysisTools, OperationsPanel
- 1 → 30 frontend tests
- v0.6.2 released with binaries for all three platforms (skipped broken v0.6.1 tag)

## Mode

Stable. Feature surface complete. Ready for 1.0 planning.
