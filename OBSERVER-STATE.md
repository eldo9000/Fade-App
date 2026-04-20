# Fade-App Observer State
Last updated: 2026-04-20  ·  Run 2

---

## Active development areas

Fade just finished a concentrated architectural cleanup arc. In a single day the project shed substantial debt: Node runtime in CI bumped 20→22, the abandoned `serde_yaml` crate swapped for `serde_yml`, `librewin-common` moved from a bare git hash to tag `v0.1.3` (a new tag cut in the Libre-Apps repo specifically to unblock this), all ten mutex `.lock().unwrap()` sites converted to informative `.expect()` calls, and — most consequential — `App.svelte` split from a 6,014-line God component into 3,103 lines supported by six newly-extracted sibling components (UpdateManager, PresetManager, CropEditor, QueueManager, ChromaKeyPanel, AnalysisTools, OperationsPanel). A 48% reduction in the top-level component. The split landed across three sequential dispatch sessions, each green on the first attempt; no rollbacks, no tripwires. Feature work is paused; the mode is architectural hygiene before 1.0.

## Fragile / high-risk areas

The `$bindable` chain from App.svelte down through QueueManager and the operation panels is the one live fragility. Operation runners mutate `selectedItem.status`, `.percent`, and `.error` in place while a job is running; if any layer of the chain is accidentally converted to a read-only prop in a future change, progress updates silently stop and jobs appear stuck at "converting". Every refactor commit since 5a flagged this in the `fragile:` field, and it remains true. The `_loadGen` generation counter inside QueueManager's async preview pipeline is the second trap: rapid reselection depends on it to abort in-flight `get_waveform` calls, and simplifying it would attach waveforms to the wrong file.

Release pipeline is still broken from before this arc. Tag `v0.6.1` points at a commit where `src-tauri/tauri.conf.json` reads `0.6.0`; the tag-vs-config version check in `release.yml` aborts before building. User has explicitly deferred re-release. No binaries have shipped since v0.6.0.

## Deferred work accumulation

One item deferred by user decision: the v0.6.2 release to unblock the pipeline and ship the Gatekeeper ad-hoc-signing fix. Everything else queued during the sprint landed — no accumulating drift. A mid-sprint deferral (`runSubLint`/`runSubDiff`/`runSubProbe` were flagged to move during 5c step 3) was resolved within the same session. The structured `deferred:` fields in git notes are being used as intended and clearing as work lands.

## Pattern watch

The two catalogued bug classes (BC-001 inverted toggle-state SVGs, BC-002 audio analysers black before first playback) both live in `Timeline.svelte`, which was intentionally out of scope for the split arc. Neither recurred this run. BC-003 was added and removed within the same day — a tag-debt issue resolved by cutting the Libre-Apps tag. No new bug classes emerged from the refactor, which is noteworthy given its scope.

## CI health

CI is durably green. The last ten main runs are all successful (CI workflow, 50s–1m40s each). One transient red during the sprint (24673210348) was GitHub infrastructure failing to download `dtolnay/rust-toolchain@stable`, not a code failure — the next run succeeded with the same action. Release workflow remains red on the v0.6.1 tag from before this arc; that is a separate pipeline and does not affect main CI.

## Observer notes

**Run 2 — the commit-notes protocol is now load-bearing.** Fifteen commits since Run 1 all carry structured notes with `state`, `context`, `deferred`, `fragile`, and `ci` fields. The observer can now synthesize from the notes themselves, not just commit titles — this is the first run where the protocol is producing meaningful signal.

Continuity check against Run 1: the two risks flagged then were (1) the release pipeline, which remains unresolved (user-deferred, not a protocol failure), and (2) "cannot measure deferred work yet, need ~10 commits under the protocol." That blocker has cleared — deferred work is measurable now and reads clean.

The refactor arc is the kind of case the Observer Loop was built for: a large, risky change decomposed into three sequential worker dispatches, each landing green in one shot, with the observer-visible state accurately reflecting progress throughout. No worker-to-main drift. The `$bindable` fragility flagged in every refactor commit is exactly the kind of lingering risk the observer should carry forward into future sessions — any commit that touches `selectedItem` should prompt a check against that chain.

Test coverage grew from 1 to 30 frontend tests during the sprint. Backend unit tests continue to pass. The project is measurably healthier than 24 hours ago; the last remaining item of substantive debt on the table is the release pipeline.
