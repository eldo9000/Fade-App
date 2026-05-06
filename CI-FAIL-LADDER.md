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

---

## Fail #1 — 2026-04-30 — cargo fmt: assert! in find_blender_does_not_panic too long

- **Q1 in-last-commit:** yes — `src-tauri/src/args/model_blender.rs` (commit `555e602`) added the test
- **Q2 named-error:** yes — `Diff in .../args/model_blender.rs:114` — assert! line over rustfmt column limit
- **Q3 seen-before:** no — first failure in this arc
- **Q4 broken-vs-missing:** broken — test code written without running rustfmt; format wrong
- **Verdict:** QUICK (budget: 1 attempt)
- **Hypothesis:** Worker ran `cargo check` but not `cargo fmt --check`; assert! on one line exceeds rustfmt line width.
- **Next:** `src-tauri/src/args/model_blender.rs` — run `cargo fmt --manifest-path src-tauri/Cargo.toml`

## Fail arc closed — 2026-04-30 — 1 entry — green CI 25140539416

---

## Fail #1 — 2026-05-05 — tag v0.7.0-beta.1 vs tauri.conf.json version 0.7.0

- **Q1 in-last-commit:** yes — `src-tauri/tauri.conf.json` version changed in commit `45f935e`
- **Q2 named-error:** yes — `tag version (0.7.0-beta.1) does not match src-tauri/tauri.conf.json version (0.7.0)`
- **Q3 seen-before:** no — first failure in this arc
- **Q4 broken-vs-missing:** broken — version bumped to `0.7.0` but tag pushed as `v0.7.0-beta.1`; config must match tag exactly
- **Verdict:** QUICK (budget: 1 attempt)
- **Hypothesis:** Version bump commit used `0.7.0` as the version string; tag pushed as `0.7.0-beta.1`. Release workflow exact-match gate rejects any mismatch. Fix: bump all three version fields to `0.7.0-beta.1`, commit, retag.
- **Next:** `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` — update `0.7.0` → `0.7.0-beta.1`
