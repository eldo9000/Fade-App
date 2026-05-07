# Fade-App Observer State
Last updated: 2026-05-07  ·  Run 21

---

## Active development areas

A single-session burst of feature dispatch landed Sprint A through H, clearing what Run 20 had described as the standing format-coverage backlog. Ten commits touched image, audio, video, office, 3D, optical, and AI subsystems in roughly that order, plus a final synthesis commit (Sprint H) that added neural matte, ISO/DMG/CBZ packaging, video inserts, and subtitle ops. Direction of work has shifted decisively from release/CI hardening (the focus through Run 20) into format-surface expansion. The README and SESSION-STATUS were updated in lockstep; no arc is currently in flight. Nothing visible is unlanded — the session ended with all 35 tasks committed, CI green, and SESSION-STATUS reflecting "no arc in flight."

## Fragile / high-risk areas

Today's churn was wide rather than deep — most commits added new modules rather than modifying hot spots, so churn concentration is low despite the commit volume. The two CI failures that did occur (cargo fmt on Sprint A, Clippy `too_many_arguments` on Sprint F's `dvd_rip.rs`) were both QUICK lint fixes resolved in one attempt; neither indicates a fragile region, only the cost of dispatching ten feature commits without a pre-push lint pass. The optical media path (`run_handbrake`, new `dvd_rip.rs`) is the youngest and least exercised surface and now carries a `#[allow(clippy::too_many_arguments)]` suppression that pattern-matches existing operations code — worth watching for the next refactor. Blender backend path resolution, flagged in prior runs, is now hardened by TASK-29 unit tests and has dropped off the risk surface. The unlanded `common-js/` design-system drift flagged in Run 20 is no longer mentioned in this evidence pack; whether it landed, was reverted, or simply remained untracked is not visible from the inputs provided.

## Deferred work accumulation

The standing backlog from Run 20 (image/audio/office/3D/AI format gaps) has fully resolved. The remaining deferred items are all environment-blocked rather than work-blocked: HAP encoder absent, HAP divisibility unverifiable, libtheora absent — all three CONFIRMED env-blocks tied to the Homebrew FFmpeg 8.1 build, stable since 2026-04-28. Today's session added a fourth in the same category: OGV (Theora) is now CONFIRMED env-blocked against the same root cause, with the sweep case `theora_default` left in place to surface the block on every run. h265-lossless remains explicitly deferred. The DNxHD/DNxHR 64×64 fixture limitation persists as a known sweep finding. No new TODOs or named stubs appeared in today's commits — CorridorKey shipped as a stub by design and is documented as such in the commit message.

## Pattern watch

"CI is the only truth" held: both failures were caught by CI before being marked PASSES, and SESSION-STATUS only declared Sprint A–H complete after run 25516160195 went green. The Clippy `too_many_arguments` recurrence is itself a pattern — operations.rs already carried the same allow, and `dvd_rip.rs` followed suit; the fix commit message explicitly invokes the precedent ("matches operations pattern"), which is the right way to handle a recurring lint shape. No `validate_output_name()` regressions surfaced; no `$bindable()` write-back cascades surfaced. The "diagnose the failing environment first" pattern produced four clean env-block confirmations rather than wasted investigation cycles on missing codecs.

## CI health

Green and stable. Last 5 runs: 4 success, 1 failure; the failure was a single Clippy lint on a fresh module, fixed in the next push. Average run time roughly 3–4 minutes, no flakes, no infrastructure failures. The two QUICK failures in this session both resolved on first retry, consistent with the sprint cadence — large feature drops, small lint corrections, no deeper signal.

## Observer notes

The format-coverage arc Run 20 implicitly flagged as the next major work surface has compressed into a single day. That is unusual momentum for this project, which has historically run testing-heavy in the 0.6 → 0.8 window — worth watching whether the next session shifts back to validation/sweep work to confirm the new surfaces, since today's commits added breadth without adding sweep cases for most of the new formats. The `theora_default` case is the one exception: it is present and self-reporting. Git notes remain absent on every recent commit (now 14+ consecutive observer runs without a notes entry); the protocol can be considered de facto retired. SESSION-STATUS was kept current within the session — the stale TASK-29 reference Run 20 flagged has been cleaned up, and Known Risks now lists the Blender path logic as HARDENED. INVESTIGATION-LOG continues to absorb only env-block confirmations, which is its quiet-state shape. No artifact files are missing from the evidence pack.

Run 20's flagged risks have either shrunk or resolved — the Blender hardening is fully landed, the format backlog is cleared, BC-005 stays quiet. The one Run 20 signal not addressed by the evidence pack is the unlanded `common-js/` design-system diff; without working-tree visibility this run cannot judge whether it persists.
