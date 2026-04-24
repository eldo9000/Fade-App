# Fade-App Observer State
Last updated: 2026-04-24  ·  Run 10

---

## Active development areas

The session that concluded today was a regression hunt, not a feature sprint. Adding SilencePad to VideoOptions in the `fix(ui)` commit (`23cc377`) introduced a CT test regression: the Advanced section in VideoOptions never rendered after its toggle button was clicked. Three consecutive red CI runs followed before the regression closed on `4a23608`.

Also landed in this session: right sidebar color polish (category headers white, subcategories gray, live-button hover normalized to full-accent blue), native `title=` attribute removal across all Svelte files, and diagnostics box made read-only (click handler removed). The computer-use test harness (`112f85d`) was added but carries no CI artifact — it is untracked scripts, not integrated into the CI gate.

No feature work is currently in flight. The project is in a clean milestone state post-regression-close.

## Fragile / high-risk areas

The **Svelte 5 `$bindable` default + `{#if}` nesting** pattern is now a confirmed fragile area. Two commits were required to close the regression, and the first fix (`3fd62b9`, a guard in `$effect`) was insufficient. The root cause is that `$bindable(null)` defaults trigger a synchronous `undefined→null` write-back during component initialization — before any `$effect` runs — causing a reactive cascade that can collapse a parent `{#if}` block. The fix (`4a23608`) removes the null defaults from `$bindable()` in SilencePad and initializes `pad_front`/`pad_end` to `null` explicitly in the CT test's `baseOpts`. The pattern is now documented in CLAUDE.md Known Patterns (pending — not yet written). Any future `$bindable(default)` usage inside a conditionally-rendered block is a candidate for this failure mode.

The **Blender backend path resolution** fragility persists unchanged from Run 9. The code bugs (BC-003, BC-004) are resolved; the runtime script path resolution for non-dev deployments is not.

The **analysis-result one-shot listener race** persists unchanged from Run 9.

## Deferred work accumulation

The `$bindable` cascade pattern should be appended to CLAUDE.md Known Patterns — the debugging arc produced a non-obvious finding that future sessions will re-discover without it. This was not done during the session.

The three informal off-ledger items from Run 9 remain untouched: STEP/IGES format support, `librewin_common` superset-vs-authoritative strategy, and the `createLimiter` formal slot-leak watchdog.

An untracked `CODEX.md` file and `codex/` directory appear in `git status`. Their purpose is unknown from the git log — they may be planning artifacts from the computer-use work. They are neither committed nor gitignored.

## Pattern watch

A new `$bindable` failure mode was confirmed this session. The prior Run 9 note that "$bindable chain topology clarified" was accurate, but the audit only checked mutation paths — it did not catch initialization-time write-back behavior for `$bindable(default)` used with undefined parent values inside conditional blocks.

BC-001 through BC-004 remain resolved. The INVESTIGATION-LOG has all entries CONFIRMED; no open investigations.

The `lib.rs` IPC hub pattern did not surface this session. Pattern is stable.

## CI health

Three consecutive failures (45s, 3m44s, 3m36s) followed by one green (2m19s) on `4a23608`. The 45-second failure was cargo fmt only. The two longer failures both failed on E2E component tests — specifically the four video-options Advanced section tests. CI is currently green. The regression window was approximately 4 hours (02:49 UTC → 03:43 UTC on 2026-04-24).

## Observer notes

**SESSION-STATUS.md is stale.** It reflects state as of 2026-04-23 and describes the next action as "feature work or observer sync." It does not record the regression, the three red CI runs, the root cause discovery, or the resolution. The Known Risks section does not mention the new `$bindable(null)` cascade pattern.

**CLAUDE.md Known Patterns section needs a new entry.** The `$bindable(null)` cascade finding is exactly the kind of non-obvious, re-discoverable gotcha the section exists to capture. It was not appended during the session.

**Git note protocol remains cold.** No commit in the last 20 carries a structured note. The gap has persisted since Run 4 or earlier.

**Run 9 fragile area still open, Run 10 adds a new one.** The Blender path resolution risk carried forward; the `$bindable` cascade was newly confirmed. The session closed its own regression but left the CLAUDE.md knowledge artifact undone.

**macOS 26 beta (Darwin 25.4.0) cannot run Playwright CT locally.** Local CT test execution is not a reliable gate on this machine. CI is the only truth for CT results — consistent with CLAUDE.md protocol, but worth noting explicitly given the regression took 3 pushes to diagnose.
