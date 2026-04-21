# Fade-App Observer State
Last updated: 2026-04-21  ·  Run 4

---

## Active development areas

Run 3's "dormant between phases" posture has been overtaken. A 4-session audit arc opened and closed in a single day (2026-04-20), shipping 18 PR batches (B1–B18) covering security, concurrency, performance, and type-safety work across the whole Tauri/Rust/Svelte surface. Headline landings: `serde_yml` → `serde_yaml_ng` (RUSTSEC kill), `write_fade_log` atomic append, IPC input caps and trust gate across all 29 `OperationPayload` variants, `run_ffmpeg` consolidation from 3 diverged copies to 1 canonical with rate-limited progress, a `JobOutcome` typed enum replacing `"CANCELLED"`/`"__DONE__"` sentinel strings, a streaming-RMS waveform path (O(file) → O(n) memory), a frontend `createLimiter` semaphore capping batch fanout at `hardwareConcurrency`, and ts-rs codegen producing 12 TypeScript type files at build time. B16 landed phase 1 (sentinel cleanup in `run_operation` dispatch); phase 2 (async lifecycle for 14 analysis/probe/preview commands) is explicitly deferred. A post-wrap fmt sweep (`7b8901d`) cleared a CI red on the wrap commit. The arc is closed; no feature work is in flight; `main` is green.

## Fragile / high-risk areas

The Run 3 `$bindable` chain risk (App.svelte → QueueManager → OperationsPanel / ChromaKeyPanel / AnalysisTools mutating `selectedItem.status`/`.percent`/`.error` in place) persists structurally and was not touched by the audit arc — SESSION-STATUS still calls it out. The audit did not stress it, but every new extracted component or prop-shape change remains the failure mode.

A new structural fragility surfaced at B16 phase 1 close: the convert/ modules still use `Err("CANCELLED")` string sentinels, bridged into `JobOutcome` via `from_result` exact-match. The typed invariant now holds at the `run_operation` dispatch but is not yet uniform across the codebase — a third sentinel introduced in convert/ would not be caught by the compiler. Phase 2 is the cleanup path; until it lands, the bridge is a known-leaky boundary.

The B10 `createLimiter` semaphore parks slots until a terminal `job-done`/`job-error`/`job-cancelled` event fires. If an event is emitted with an unknown jobId, if the frontend tears down mid-batch, or if a crash-reporter swallows the event, slots are never released and the limiter drains to zero — future conversions silently queue forever. No timeout or drain valve exists. Called out in audit followups at close.

B5 carries an unresolved strategy question around `librewin_common`: the pinned `FadePreset` type was worked around via a local `StoredPreset` superset that reads/writes the same JSON file. The pattern is implicit — if another Fade field conflicts with a pinned common type in the future, the workaround will be repeated without guidance.

KNOWN-BUG-CLASSES.md still carries BC-001 and BC-002 as active patterns despite Run 3 noting them as user-reported resolved. This documentation gap persists into Run 4.

## Deferred work accumulation

Six items were formally promoted at audit close (2026-04-20) and are now the ledger of outstanding work:

1. **B16 phase 2 / B19** — async lifecycle for 14 sync analysis/probe/preview commands. Own-session XL work. Prerequisite: `from_result` bridge removal from convert/ modules.
2. **AudioOffset i64→i32 precision drift** — ts-rs generates `bigint` in TS but frontend passes `number`. Cosmetic in practice; real in type contract. Micro-patch.
3. **Windows non-C drive preview** — B4 narrowed `assetProtocol.scope`; files on Windows secondary drives outside `%USERPROFILE%` are unreachable. Needs runtime `allow_file` expansion.
4. **Slot-leak watchdog for createLimiter** — see Fragile section above.
5. **librewin_common superset-vs-authoritative strategy** — see Fragile section above.
6. **GHA shell injection hardening** — `release.yml:110,365` interpolate `${{ inputs.tag }}` directly into `run:` steps. Write-access-gated (not externally exploitable) but standard hardening applies.

The original audits also left 9 NEEDS-EVIDENCE items from session 2 that were not promoted — they are triaged but not scheduled. Nothing else is in-flight-but-uncommitted.

## Pattern watch

Three cross-cutting patterns were named in the RETROSPECTIVE:

- **Terminal-emission invariant** — F-02 / F-05 / F-19 all expressed the same broken invariant: a terminal job event could be overwritten or reordered. B11's `JobOutcome` enum and B6's `applyProgressIfActive` status guard embody the fix. Any new code path that emits `job-done`/`job-error`/`job-cancelled` needs to honor the guard — it is not yet compiler-enforced outside `run_operation`.
- **Probe deduplication** — rewrap/extract/replace_audio/conform all separately called `run_ffprobe` + `probe_duration`. B13 folded the shared cases; two sites (extract, replace_audio across separate Tauri commands on different files) remain as "DONE-PARTIAL" until a `ProbeCache` keyed by `(path, mtime)` exists. F-32 deferred.
- **Traversal gates at IPC entry** — F-03/F-08/F-16 converged on the same structural gap. B15's `validate_output_name` / `validate_output_dir` / `validate_no_traversal` trio is canonical for `run_operation` via `OperationPayload::validate_outputs()`, but standalone commands outside the dispatch path need manual verification. Any new IPC command taking a path parameter should be audited against this pattern.

BC-001 (inverted SVG chevrons) and BC-002 (audio analysers black before first playback) — neither recurred during the audit arc. Still listed as active in KNOWN-BUG-CLASSES despite Run 3 noting user-reported resolution.

The RETROSPECTIVE notes the frontend was under-audited relative to its surface area: Svelte got one lens pass, the component extraction arc (App.svelte 6014 → 3100 lines) happened outside the audit, and event-listener lifecycle / `$effect` cleanup / JS-side path validation were not re-scanned after extraction. This is a standing pattern-watch item for any future cycle.

## CI health

`main` is green. CI run 24702207324 on commit `7b8901d` (fmt sweep) completed successfully after the preceding wrap commit `daa854f` failed on `cargo fmt --check` on macOS aarch64 (parking_lot import drift from B18 + long-call reformatting). The doc-only commit `b8cbf97` is in flight at observer time but no code changed and no test surface is touched.

The audit arc's 18 substantive commits all landed green per-commit, with a single post-wrap CI red that was fixed in ~25 minutes. This is the first CI failure in the last 40 commits. Sustained green is intact.

## Observer notes

**Run 4 vs Run 3 — phase transition reversed.** Run 3 declared the project "dormant between phases" and noted the observer had "nothing to synthesize from active work." That posture lasted less than a day before the audit arc opened, executed, and closed. Run 4 is rich again — audit artifacts (01-static-analysis, 02-concerns, 03-adversarial, 04-attack-plan, RETROSPECTIVE), 6 new deferred items, 3 named patterns, and a post-close CI skirmish all need synthesis. Observer cadence worked: Run 3's thin observation correctly reflected thin inputs; Run 4 thickens appropriately.

**Commit note protocol went cold during the audit sprint.** Of the last 40 commits, only `7b8901d` (the fmt sweep) carries a git note. Every audit batch commit, every `chore(audit)` status update, and the wrap commit itself shipped without structured notes. This is a regression from the Run 3 era (when all 20 most recent commits carried notes). The practical impact is low for this arc because the audit files themselves (especially 04-attack-plan §8 and RETROSPECTIVE) are a richer ledger than git notes would have been — but the protocol expectation is that both exist, and audit sessions in particular warrant `deferred:` and `fragile:` fields per batch. Worth flagging to the next session.

**INVESTIGATION-LOG is stale.** Last entry is 2026-04-20 pre-audit (the QueueManager/ChromaKey/Analysis/Operations extraction). No audit-related investigations were logged, even though the arc produced 33 root-cause findings across 4 sessions. The 18-batch execution plan is the equivalent record and is comprehensive, but the standing log format was bypassed. Same observation as note-protocol: low practical impact here, protocol gap in principle.

**KNOWN-BUG-CLASSES documentation debt persists.** BC-001 and BC-002 were flagged as user-reported resolved in Run 3. The file was not updated during the audit arc. Adding a third item from audit findings (e.g., terminal-emission invariant as a pattern) would be a natural expansion — but first the existing entries should be reconciled with reality.

**New file in repo:** `audits/` directory with 5 markdown files is the new center of gravity for project knowledge. Future observer runs should read it alongside SESSION-STATUS. RETROSPECTIVE §"what the next audit cycle should look for first" is specifically scoped as the entry point for cycle 2.

**Structural health read.** The project is in the strongest post-audit state of any observer run to date: 33 findings closed, 18 batches shipped with zero regressions, verify pass clean, CI green. The B16 phase 2 async work is the single biggest outstanding debt but it is explicitly scoped and deferred rather than forgotten. The `$bindable` chain and the `createLimiter` slot-leak are the two live fragilities worth watching if any new work touches those areas.
