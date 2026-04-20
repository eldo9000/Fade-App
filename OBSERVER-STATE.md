# Fade-App Observer State
Last updated: 2026-04-20  ·  Run 3

---

## Active development areas

The project has no active feature work in flight. The 2026-04-20 housekeeping arc — Node 22 upgrade, `serde_yml` migration, `librewin-common` tag pin, mutex `.expect()` sweep, and the full App.svelte split into seven extracted components — is complete and shipped as v0.6.2. The session that closed this arc wrote a clean SESSION-STATUS declaring no outstanding work and the project ready for either 1.0 feature planning or pre-ship polish. There are no branches, no deferred tasks, and no in-progress commits visible in the last 40 log entries. The project is dormant between phases.

## Fragile / high-risk areas

The `$bindable` chain from App.svelte through QueueManager and into the operation panels (OperationsPanel, ChromaKeyPanel, AnalysisTools) remains the single live fragility. Operation runners mutate `selectedItem.status`, `.percent`, and `.error` in place during job execution; this dependency was flagged in the `fragile:` field of every refactor commit from TASK-5a through TASK-5c and again in the final SESSION-STATUS. Any future commit that touches operation runners, queue state, or prop-passing along this chain should verify the binding chain is intact. The `_loadGen` generation counter inside QueueManager's async preview pipeline is the second trap: it guards against attaching waveforms to the wrong file during rapid reselection, and simplifying it away would introduce a silent data-binding race.

BC-001 (inverted toggle SVGs in Timeline.svelte) and BC-002 (audio analysers black before first playback) have been reported resolved by the user, but no fix commits appear in the visible log. KNOWN-BUG-CLASSES.md still carries both entries. This is a documentation gap — the bug class file should be updated to reflect resolved status, and ideally the fix commits would carry `fragile:` context so the observer can confirm resolution.

## Deferred work accumulation

Nothing is formally deferred. The v0.6.1 release pipeline skew (tag pointed at a config still reading `0.6.0`) that was carried as deferred across several commits has been resolved: v0.6.2 shipped with correct version alignment across tag, config, and binaries for all three platforms. The `deferred:` fields in git notes read `none` for the last two substantive commits. The tasks/ directory task files (TASK-5a/5b/5c, TASK-6) are staged for deletion but that deletion has not been committed — a minor hygiene item.

## Pattern watch

BC-001 and BC-002 were documented in KNOWN-BUG-CLASSES.md and have been reported resolved. Neither pattern recurred during the refactor arc, consistent with them being isolated to Timeline.svelte, which was intentionally out of scope for the split. BC-003 (librewin-common bare-hash pin) was resolved and removed during the sprint. No new bug classes emerged from the refactor despite its scope — all seven extracted components landed green on first dispatch.

The prop-drilling / `$bindable` fragility flagged across every TASK-5 commit is itself a pattern worth watching: the further the component tree grows from App.svelte, the more places a state-shape change has to propagate. This is not a bug class yet, but it is the structural risk that would produce one.

## CI health

CI is green. The last 40 commits show `ci: green` on the most recent (`842e11a`) and `ci: unknown` on all prior sprint commits — the `unknown` values reflect mid-sprint states where CI was not checked per commit, not failures. No GitHub remote slug is resolvable from the CLI in this environment, so live CI run status cannot be confirmed here; this has been the case for all three observer runs. OBSERVER-STATE Run 2 noted durable CI green across the last ten main runs; nothing in the subsequent commits suggests a regression.

## Observer notes

**Run 3 corrections to Run 2:** Run 2 stated "no binaries have shipped since v0.6.0" — this is now stale. v0.6.2 shipped with signed binaries for macOS (aarch64), Linux (x86_64), and Windows (x86_64). The release pipeline debt Run 2 carried as unresolved is resolved. INVESTIGATION-LOG is clean: all entries CONFIRMED, no OPEN items (the stale OPEN entry for BC-001/BC-002 was closed this session).

**Protocol gap — BC resolution:** Both BC-001 and BC-002 are reported fixed, but no fix commits appear in the log. KNOWN-BUG-CLASSES.md still describes them as active patterns. The commit protocol requires a `fragile:` field on fix commits and a corresponding log entry; this resolution happened outside the visible protocol trail. KNOWN-BUG-CLASSES.md should be updated, and if fix commits exist on a branch not yet merged or pushed, they should be surfaced.

**Commit note protocol health:** All 20 of the most recent substantive commits carry structured notes. The protocol is load-bearing and being followed consistently. The only gap is the `ci: unknown` pattern across the sprint, which is an acceptable tradeoff for mid-sprint velocity rather than a protocol failure.

**Phase transition:** This is the first observer run that has nothing to synthesize from active work. The project is between phases — refactor arc complete, 1.0 planning not yet started. The observer has no deferred threads, no OPEN investigations, and no fragile work in progress. The `$bindable` chain risk persists structurally but is not being actively stressed. This is a healthy resting state.
