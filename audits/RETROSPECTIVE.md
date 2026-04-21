# Audit Retrospective — Fade, 2026-04-20

**Arc:** 4 sessions over 1 day. Baseline (S1) → concern-based (S2) → adversarial (S3) → execution plan + implementation (S4).
**Scope:** Tauri 2 + Svelte 5 + Rust media converter, v0.6.2. ~12k Rust LOC, ~11k frontend LOC.
**Outcome:** 33 findings closed, 18 batches shipped, 0 regressions, 6 items deferred.

---

## Which findings actually mattered

These were confirmed real bugs that would have reached users:

**Cancel TOCTOU (F-02 → B9).** Flag registered before Child inserted into the process map. A cancel arriving in that window set the flag but found nothing to kill — ffmpeg ran to completion, the job emitted `job-done`, and the UI showed success on a cancelled item. Real window: ~10 ms on a fast machine, longer on slow hardware. The fix (`kill_if_cancelled` called immediately after insert) is two lines, but finding the window required tracing the exact sequence across two files.

**`write_fade_log` RMW race (F-15 → B2).** In batch-convert mode, multiple jobs finish within milliseconds of each other. Each read the log file, appended their entry to the in-memory vector, and wrote the whole file back. Last-writer-wins meant all but one entry were silently dropped. The sibling `diag_append` function at the same callsite already used the correct O_APPEND idiom — the bug existed because two functions for the same purpose were written independently.

**Filmstrip orphan processes (F-11 → B9).** `get_filmstrip` spawned N ffmpeg processes in a background thread with no registration in the process map and no cancel hook. Deleting a queue item while the filmstrip was loading left ffmpeg processes running until they finished naturally, consuming CPU and emitting events to a window that no longer cared. For long files with many frames, this could mean minutes of orphaned work.

**Batch fanout (F-04 → B10).** 100 queue items → 100 concurrent `convert_file` invokes, each spawning a thread that spawned ffmpeg. On a 4-core machine this meant 100 simultaneous ffmpeg processes all competing for the same CPU. The user-visible symptom was that the first few jobs would finish quickly and then everything would slow to a crawl. The fix is a ~40-line frontend semaphore; the insight that the invoke-promise doesn't actually throttle ffmpeg (because `convert_file` returns `Ok(())` immediately after spawning) required understanding the full dispatch path.

**Streaming waveform OOM (F-14 → B12).** `get_waveform` used `Command::output()` which buffers the entire decoded PCM stream before returning. A 1-hour file at 8 kHz mono = ~115 MB in a Vec<u8>. On podcast editors and long-form video this was a guaranteed OOM. The streaming fix is non-trivial (byte-boundary carry buffer, partial-chunk f32le parse) but the unit test surface is excellent — the bug is the kind that passes all tests and crashes in production on a specific file length.

**`serde_yml` unsound (F-07 → B1).** RUSTSEC-2025-0067/0068. Reachable via user-supplied `.yaml` preset files. Trivial XS fix (dep swap to `serde_yaml_ng`), but it was a live unsoundness surface that cargo-audit was actively flagging.

---

## Which were noise the adversarial pass correctly killed

**CONC-009 poison cascade ("undead app").** Session 2 constructed a narrative where a mutex-poisoning panic cascades to kill the Tauri runtime and drop all in-flight jobs. Session 3 correctly identified that the mutex holds are trivial (HashMap insert/remove, AtomicBool store) and no user code runs inside the lock. The panic path that would trigger poisoning doesn't exist. The residual `parking_lot` fix (B18) was worth doing on principle, but the "live crash path" framing was fiction.

**CONC-003 (listen-after-invoke filmstrip race).** Session 2 worried about the `listen()` call racing the backend emit. Session 3 confirmed the backend path is `spawn → nice → ffmpeg seek+decode → base64` — minimum ~100 ms before first emit. The `listen()` Promise resolves in a microtask. No real race.

**ARCH-012 (IPC command exposure via `core:default`).** Session 2 flagged that `core:default` exposes all `#[command]` handlers. Session 3 correctly identified this as a category error — Tauri 2 capability permissions gate plugin commands, not user-defined `generate_handler!` commands. Only the `assetProtocol.scope` component (F-08) survived as a real finding.

**PERF-009/010 (`.to_string()` × 133, clone-heavy probe parse).** Both are µs operations inside a workflow that blocks for minutes on ffmpeg. Ratio of optimization effort to measurable impact is effectively zero.

---

## Patterns that repeated

**Terminal-emission invariant.** Three separate findings (F-02, F-05, F-19) all involved the same broken invariant: a terminal event (`job-done`, `job-error`, `job-cancelled`) could be emitted in the wrong order or overwritten by a later in-flight event. The invariant needed to be stated explicitly and enforced at the emit site, not just documented. B11's `JobOutcome` enum and B6's `applyProgressIfActive` status guard both embody the same insight.

**Probe deduplication.** Four operations (rewrap, extract, replace_audio, conform) called `run_ffprobe` and then `probe_duration` as separate subprocess invocations on the same file (F-22). The pattern appeared independently in each operation module because `probe_duration` and `run_ffprobe` were separate functions with no shared cache. The fix (fold duration into probe result, `duration_from_probe`) was straightforward once the pattern was named.

**Traversal gates at IPC entry.** F-03, F-08, F-16 all came down to the same structural gap: frontend-supplied paths reaching filesystem operations without confinement. The fix is always the same shape — validate at the IPC boundary, not inside the operation. B15's `validate_output_name` / `validate_output_dir` / `validate_no_traversal` trio is the canonical form. This pattern will reappear on any new IPC command that takes a path parameter.

---

## What the next audit cycle should look for first

1. **B16 phase 2: async command discipline.** Zero `async fn` in the project at audit close. 14 analysis/probe/preview commands still block the IPC thread. The next cycle's static analysis will show the same "0 async fn" count as a leading indicator. Start there.

2. **Probe cache (F-32).** Four files probe the same path for different callers; no shared result. As the command set grows, this multiplies. A `ProbeCache` keyed by `(path, mtime)` would be a 1-day investment with compounding returns.

3. **`createLimiter` slot exhaustion.** The B10 semaphore has no timeout or drain valve. A job that never fires `job-done`/`job-error`/`job-cancelled` (possible if a crash-reporter swallows the event) will leak a slot permanently. The next cycle should add a watchdog.

4. **New IPC commands since B15.** Any command added after B15's trust gate needs manual verification that it calls `validate_no_traversal` / `validate_output_name` as appropriate. The pattern won't be enforced by the compiler unless `OperationPayload::validate_outputs()` is the only dispatch path — which it is for `run_operation`, but not for standalone commands.

5. **GHA hardening.** `release.yml` interpolates `inputs.tag` directly into shell. Low urgency (write-access-gated) but the fix is one line per site. Pick it up in the next polish batch.

---

## What the 4-session format got right vs. where it over/under-invested

**Got right:**

The adversarial pass (session 3) earned its cost. Without it, CONC-009 and ARCH-012 would have been in the execution plan — probably as a dedicated Mutex refactor and a capability-layer redesign. Session 3 killing both saved at least 2 days of work on non-bugs. The adversarial framing specifically — "find the weakest claim" — is the right posture for session 3 of any audit.

The phased execution structure (Phase 1 fast-wins → Phase 2 foundations → Phase 3 structural) worked well in practice. B1–B7 shipped in parallel because they genuinely had zero dependencies. Trying to do B16 (XL) before B8 (run_ffmpeg consolidation) would have been painful — the dependency graph was real, not decorative.

Stable finding IDs (F-01 through F-33) across all 4 sessions were load-bearing. The ability to write "B11 closes F-05 and F-29" in a commit message, and have that mean something without re-reading the original finding, reduced cognitive overhead significantly.

**Over-invested:**

Session 2 (concern-based) produced 79 findings, of which 18 were WEAKENED and 2 were REJECTED by session 3. That's a ~25% false-positive rate after adversarial review. The concern-based lens generates volume efficiently but with lower precision — next cycle could compress it to a single pass with a higher bar for promotion to "CONFIRMED" rather than four separate lenses.

The priority scoring formula (§2 of this file) was computed once and never consulted again. Actual execution order was driven by dependency structure and blast radius, not score. The formula is fine for a first-pass sanity check but it doesn't need to be 33 rows of arithmetic.

**Under-invested:**

The frontend was under-audited relative to its surface area. Sessions 1–3 focused heavily on Rust; the Svelte side got one lens pass and the vitest suite grew from 30 → 56 tests, but the component extraction arc (App.svelte 6014 → 3100 lines) happened outside the audit and wasn't re-scanned. The next cycle should run a dedicated frontend pass: event listener lifecycle (add/remove symmetry), `$effect` cleanup functions, IPC call sites that don't call `validate_output_name` equivalents on the JS side.

CI was not re-run from scratch during the audit arc — the audit relied on per-commit CI results rather than a single clean green run over HEAD. This is fine for an in-flight audit but the close verification should always include one full CI run. The `async fn: 0` finding was caught by grep, not by a test that would have failed.
