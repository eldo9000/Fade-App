# Fade — Session Status

Last updated: 2026-04-20

---

## Current Focus

4-session audit arc complete. All 18 PR batches shipped. Verify pass clean. CI green on `main` after post-wrap fmt sweep (`7b8901d`). Project in stable post-audit state.

## Next action

No outstanding work. Deferred items tracked in `audits/04-attack-plan.md §7`. Highest-leverage next investment: **B16 phase 2** (async lifecycle for 14 read-only sync commands) — own session, phase 3–4 commands at a time.

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

- **B16 phase 2 not landed.** 14 analysis/probe/preview commands remain synchronous. They block the IPC thread for their full duration and are uncancellable. Acceptable for current stable/feature state; must land before any heavy-use release.
- **`$bindable` chain through extracted components.** App.svelte → QueueManager → OperationsPanel / ChromaKeyPanel mutate `selectedItem.status`/`.percent`/`.error` in place during job execution. Any future change that accidentally converts `selectedItem` to a read-only prop will silently stop progress updates.

## Mode

Stable. Audit arc closed. Ready for 1.0 feature planning or continued pre-ship polish.
