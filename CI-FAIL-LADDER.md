# CI Fail Ladder

Append-only triage record per `/check-in`. Closing line marks each arc.

---

## Fail #1 — 2026-04-27 — TASK-3 left two stale `subtitles=in.mkv` assertions in lib.rs

- **Q1 in-last-commit:** yes — `src/args/video.rs` (commit `752ce14`) changed the emit shape; the assertions are in `src/lib.rs:2764,2776` (not in the diff but downstream of the same behaviour change)
- **Q2 named-error:** yes — `assertion failed: vf_contains(&args, "subtitles=in.mkv")` at exact file:line
- **Q3 seen-before:** no — first failure in this arc; CI-FAIL-LADDER did not exist
- **Q4 broken-vs-missing:** broken — tests assert prior emit shape; new emit wraps in single quotes
- **Verdict:** QUICK (budget: 1 attempt)
- **Hypothesis:** The TASK-3 worker was directed by the task file to look at `args/video.rs ~line 882` for the prior trivial-case test; the actual existing assertions live in `lib.rs` integration-style test block at lines 2764 and 2776. Two-line update: change expected literal from `"subtitles=in.mkv"` to `"subtitles='in.mkv'"`.
- **Next:** `src/lib.rs:2764` and `src/lib.rs:2776` — update assertion strings to match new single-quoted emit.

## Fail arc closed — 2026-04-27 — 1 entry — green CI 24986902424
