# Fade-App Observer State
Last updated: 2026-04-25  ·  Run 15

---

## Active development areas

One commit landed after Run 14: `5c1d58d` canonicalized the encoder-constraint class as BC-005 in `KNOWN-BUG-CLASSES.md`. No code was changed — this was documentation of an already-fixed recurring pattern. CI confirmed green in 3 minutes.

No arc is in flight. SESSION-STATUS continues to reflect the same clean post-arc state written at the close of the third arc.

## Fragile / high-risk areas

The **Blender backend path resolution** residual fragility is unchanged from Run 14. `fc1d9d3` added macOS bundle and Linux FHS path coverage; the general "not hardened for all deployment contexts" caveat in SESSION-STATUS persists. No new incidents.

The **analysis-result one-shot listener race** was structurally closed by `ce5b37e` and remains closed. The SESSION-STATUS Known Risks section still lists it without correction — this documentation inconsistency has now persisted across two Observer runs (14 and 15) without being addressed.

## Deferred work accumulation

The encoder-constraint naming gap flagged in Runs 12–14 is now resolved: BC-005 is in `KNOWN-BUG-CLASSES.md`. The remaining exposure in this class — untested codec paths (DNxHD bitrate-resolution matching, CineForm quality bounds, HAP sub-format compatibility) — is still unswept. The class is named; the potential instances are not yet hunted.

Of the three architectural migration items from the `&Window` decoupling arc: partial `run()` wrapper consolidation via `window_progress_emitter`, `lib.rs` → `convert()` migration, and `ProgressFn` contract redesign remain unchanged and low-urgency.

The three informal off-ledger items persist: STEP/IGES format support, `librewin_common` superset-vs-authoritative strategy, and the `createLimiter` formal slot-leak watchdog.

## Pattern watch

All five KNOWN-BUG-CLASSES (BC-001 through BC-005) are now documented and resolved. BC-005 closes the gap that had been accumulating since the first encoder-constraint fix landed three arcs ago. The `$bindable()` cascade pattern in `CLAUDE.md` has not recurred. No new pattern class is emerging from recent commit activity.

## CI health

All five most recent CI runs are green, spanning 2026-04-25 to 2026-04-26. The streak now includes `5c1d58d`. No flaky steps or recurring failures observed across any run in this window. CI configuration is unchanged.

## Observer notes

**The encoder-constraint documentation gap is closed.** This was flagged starting in Run 12 as a pattern class accumulating without a name. BC-005 is now the canonical entry. What remains is not documentation but sweep coverage — whether additional instances of the same class exist in untested codec paths.

**SESSION-STATUS Known Risks inconsistency persists for a second run.** The analysis-result listener race entry in Known Risks describes the race as structurally possible; `ce5b37e` made it structurally impossible. The "Next action" block acknowledges the fix but the Known Risks prose was not updated. This is a documentation gap that has now been noted in Runs 14 and 15.

**Git note protocol remains absent across the entire commit history.** Every commit in the 40-commit window carries no structured `git notes`. The gap has now persisted across at least 8 consecutive Observer runs.

**`CODEX.md` and `codex/`** remain untracked and uncommitted. Unchanged from Runs 10–14.

**macOS 26 beta (Darwin 25.4.0) Playwright CT note carries forward** — local CT execution remains unreliable; CI is the only truth for CT results.
