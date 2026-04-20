# Adversarial Review — Session 3 of 3

**Date:** 2026-04-20
**Branch:** main
**Commit base:** 1f9fc77 (session 2); inputs: 01-static-analysis.md (746b936), 02-concerns.md (1f9fc77)
**Repo:** Fade (Tauri 2 + Svelte 5 + Rust, v0.6.2)
**Method:** Four parallel Critic agents (one per lens), each opened referenced file:lines and rendered a verdict per finding. Verdicts: **CONFIRMED** / **WEAKENED** / **REJECTED** / **NEEDS-EVIDENCE**.
**Purpose:** Kill false positives in session 2's 79 findings. Rank survivors by confidence for session 4's fix plan.

Session 4 (synthesis + attack plan) consumes this file + 01 + 02 with no prior context.

---

## Executive summary — verdict counts

| Lens | CONFIRMED | WEAKENED | REJECTED | NEEDS-EVIDENCE | Total |
|---|---:|---:|---:|---:|---:|
| Performance | 13 | 7 | 0 | 1 | 21 |
| Concurrency | 14 | 2 | 1 | 2 | 19 |
| Convention | 19 | 5 | 0 | 1 | 25 |
| Architecture | 11 | 4 | 1 | 5 | 21 |
| **Total** | **57** | **18** | **2** | **9** | **86** |

**Headline:** ~72% of session 2's findings survive as-claimed. ~21% overstated (scope, count, or severity inflated). Only 2 outright rejections — session 2's overall signal quality is high, but several high-visibility framings (`CONC-003` filmstrip listen-after-invoke race, `ARCH-012` capability "every command callable", `CONC-009` poison cascade "undead app" scenario) are materially wrong and must not propagate.

**Top-of-report corrections session 4 must carry forward:**
1. **CONC-003 (REJECTED).** The "listen-after-invoke loses early filmstrip frames" race is not reachable — backend `thread::spawn → nice → ffmpeg seek+decode → base64` takes >100 ms before the first emit; the `listen()` Promise resolves in a microtask tick. Related lens claims (ARCH-005/006 about the broadcast) are still valid on their own merits.
2. **ARCH-012 capability scope:** Tauri 2 `#[command]` handlers from `generate_handler!` are **not** gated by capability permissions — only plugin commands are. "`core:default` means every command callable from any origin" is a category error. Real concerns on this row: `assetProtocol.scope: ["**"]` (valid) and the absence of a Rust-side path validator (ARCH-004, which stands).
3. **CONC-009 poison cascade:** Mutex holds are trivial (HashMap insert/remove, AtomicBool store). No user code runs inside the lock. The "any panic poisons the mutex" chain-of-disasters narrative is not reachable from current code. Switch to `parking_lot` on principle, but do not sell this as a live crash path.
4. **CONV-006 / Cargo.toml `unwrap_used=warn`:** CI **does** invoke `cargo clippy -- -D warnings` (`.github/workflows/ci.yml:94`). The 176→actual 102 `.unwrap()/.expect(` sites either compile clean (clippy's `unwrap_used` is `restriction`-tier, not on by default under `-W clippy::all`) or CI is red. Finding's framing "declared but unenforced" conflates two possibilities.
5. **CONV-022 `diag_append`:** It **does** rotate (size-threshold rename at `lib.rs:787-792`). Finding's "O_APPEND no rotation" claim is false. The two rotation strategies differ (line-count RMW vs byte-threshold rename), but rotation exists in both.
6. **CONC-002 / CONC-010 mutex contention "per progress line":** Workers take the process-map mutex only at child insert + remove — **not** per progress line. The executive-summary item 5 in session 2 overstates the bottleneck mechanism. Fanout cost is real; the mutex is not the bottleneck.
7. **PERF-001 "15+ sites":** Actual double-probe sites are 4 ops (rewrap/extract/replace_audio/conform), not 15.
8. **PERF-015 filmstrip preload "20 items × 20 ffmpegs":** Preload is serialized via `_bgBusy` — fan-out across time, not instantaneous concurrency.
9. **ARCH-010 ConvertOptions "95 Option fields, every op gets all":** Actual ~87 fields; only `convert_file` consumes the full struct. Mechanical ops dispatch via `OperationPayload` with narrow variant fields and **do not** see the god-struct.
10. **Count nits:** 41 vs 43 `let _ = window.emit` (actual: 41); 30 vs 32 registered commands (actual: 32, session 2 itself oscillates between 30 and "24+6=30" — off by 2). Minor but flags counting discipline.

---

## Top 15 CONFIRMED findings, ranked by (impact × confidence)

Ranking lens: each finding must (a) name a concrete user-visible or correctness failure, (b) survive Critic review at full severity, (c) ideally converge across 2+ lenses or cross-reference session 1.

### 1. `run_ffmpeg` duplicated 3× + per-file re-rolls — any fix silently diverges
**Lenses:** PERF-007 · CONV-002 · ARCH-002 · CONC-008 · **multi-agent (4)**
**Evidence:** `operations/mod.rs:129` canonical; `operations/merge.rs:201 run_ffmpeg_merge` verbatim copy with obsolete justification comment at `:198`; `convert/subtitle.rs:121` third copy. All three implement the same spawn / stderr-drain / progress / cancel pattern. `operations/analysis/mod.rs:18 run_ffmpeg_capture` is a 4th variant with different semantics.
**Session 1 xref:** none direct, but compounds session-1 #6 (progress cadence) — any progress rate-limit fix has to land in every copy.
**Severity:** **HIGH** — load-bearing for cancel + progress correctness. Already drifted.

### 2. Cancel TOCTOU — flag registered before Child inserted; cancel-in-gap lets ffmpeg run to completion
**Lens:** CONC-001
**Evidence:** `lib.rs:542-549` inserts `cancellations[job_id]=false` then `thread::spawn`s worker. `operations/mod.rs:141-150`: `Command::spawn()` then `processes.lock().insert(job_id, child)`. `cancel_job` (`lib.rs:733-737`): `map.get_mut(&job_id)` returns None during the gap, no kill issued. The progress loop (`mod.rs:163-181`) never checks the cancel flag; only the post-wait branch does. Net effect: user clicks cancel, ffmpeg completes fully (wasted minutes), worker then reports CANCELLED, output file is removed.
**Session 1 xref:** none direct; overlaps session-1 #2 (sync-command mainthread block makes cancel slow).
**Severity:** **HIGH** — user-visible correctness bug, not a theoretical race.

### 3. `validate_output_name()` invariant doesn't exist; every `run_operation` variant bypasses validation
**Lenses:** ARCH-004 · CONV-021 · session-1 #3 · **multi-agent (3)**
**Evidence:** `grep "fn validate_" src-tauri/src` yields only `validate_suffix` / `validate_separator` (lib.rs:325, 340), both called only from `convert_file`. `OperationPayload` 29 variants (lib.rs:838-1021) take raw `output_path: String` / `output_dir: String` and pass through `run_operation` (lib.rs:1025-1562) into ops unvalidated. `scan_dir`, `file_exists`, `diff_subtitle`, `open_url` also accept raw paths/strings.
**Session 1 xref:** #3 (HIGH).
**Severity:** **HIGH** — documented invariant load-bearing for CLI-arg safety; currently fiction.

### 4. Batch convert unbounded fanout — 100 items = 100 concurrent ffmpegs
**Lens:** CONC-002
**Evidence:** `App.svelte:910-929`: `for (const item of willRun) { ... invoke('convert_file', {...}).catch(...); }` — no await. Backend worker is `std::thread::spawn` per invoke. Frontend has pause but no worker-pool.
**Session 1 xref:** #4 (HIGH — general unbounded input).
**Severity:** **HIGH** — degrades to CPU/IO thrash on any reasonable batch.
**Correction:** Contention is at worker insert/remove only — **not** per progress line. Still want a semaphore/worker-pool.

### 5. `convert_file` vs `run_operation` diverged on logging + sentinel handling
**Lenses:** CONV-003 · ARCH-003 · multi-agent (3)
**Evidence:** `lib.rs:672-712` `convert_file` finalizer writes `write_fade_log` on all 4 outcomes + handles both `"CANCELLED"` and `"__DONE__"`. `lib.rs:1528-1559` `run_operation` finalizer writes zero log entries + only matches `"CANCELLED"`. Same conceptual shape, diverged concretely.
**Session 1 xref:** #8 (`write_fade_log` race).
**Severity:** **HIGH** — silent behavioral gap; a user-visible operation ending via `__DONE__` from a non-convert op path falls through to `job-error`.

### 6. Three disjoint job lifecycles — analyze_* / probe / preview emit no progress, register no Child, are uncancellable
**Lenses:** ARCH-003 · ARCH-007 · CONC-011 · CONV-003
**Evidence:** `invoke_handler!` (`lib.rs:1596-1628`) registers `analyze_loudness`, `analyze_cut_detect`, `analyze_black_detect`, `analyze_vmaf`, `analyze_framemd5`, `get_waveform`, `get_spectrogram`, `preview_diff`, `preview_image_quality`, `chroma_key_preview`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, `get_file_info`, `get_streams` all as sync commands returning blocking. No event emission; no cancel plumbing.
**Session 1 xref:** #2 (HIGH — sync commands on IPC thread).
**Severity:** **HIGH** — UI has no cancel for any of these; blocks for minutes on long files.

### 7. `serde_yml` unsound + reachable via user YAML
**Lenses:** ARCH-008 · session-1 #10
**Evidence:** `convert/data.rs:224`: `"yaml" | "yml" => serde_yml::from_str(raw)` where `raw` is the contents of any queued `.yaml`/`.yml`. `:312` serializes back via `serde_yml::to_string`. RUSTSEC-2025-0068 / RUSTSEC-2025-0067 directly reachable on any user file drop.
**Session 1 xref:** #10 (MEDIUM — reachability was flagged as "needs confirmation", now confirmed).
**Severity:** **MEDIUM** — unsound ≠ exploitable RCE, but live surface on any malformed YAML input. Drop-in migration to `serde_yaml_ng` available.

### 8. Capability manifest `assetProtocol.scope: ["**"]` — unscoped asset-protocol read
**Lens:** ARCH-012 (partial — the component that survives critic review)
**Evidence:** `tauri.conf.json` `"scope": ["**"]`. Any file path served via `asset://` is readable by any webview content.
**Session 1 xref:** #1 (capability manifest flagged as not-analyzed).
**Severity:** **MEDIUM** — real arbitrary-file-read surface via the webview's asset protocol, conditional on webview content ever reaching attacker-controllable state.
**Correction:** The sibling claim "every #[command] callable because core:default is granted" is a **category error** and rejected — Tauri 2 doesn't gate user-defined app commands via capability permissions.

### 9. `open_url` accepts any URL scheme
**Lens:** ARCH-013
**Evidence:** `lib.rs:1574-1583`: `open_url(url: String)` passes `&url` directly into `open` / `xdg-open` / `cmd /C start` with no scheme parse, no allowlist.
**Session 1 xref:** none.
**Severity:** **MEDIUM** — `file://`, custom schemes, shell-parsed edge cases (`cmd /C start`) all reachable from any webview-origin call.

### 10. Preset type drift — `normalize_loudness` sent from frontend, silently dropped on Rust save
**Lenses:** ARCH-009 · CONV-016 (related)
**Evidence:** `PresetManager.svelte:17,20` built-in presets include `normalize_loudness: true`. `presets.rs:16-42 save_preset` parameters are `(name, media_type, output_format, quality, codec, bitrate, sample_rate)` — no `normalize_loudness`. User saves persist nothing; field silently dropped.
**Session 1 xref:** none.
**Severity:** **MEDIUM** — persisted presets quietly miss a user setting.

### 11. Filmstrip orphan processes — no Child registry, no cancel hook
**Lenses:** CONC-006 · PERF-015 (partial) · ARCH-005 (related)
**Evidence:** `probe/filmstrip.rs:44-93`: `std::thread::spawn(move || { for i in 0..count { Command::new("nice").args([...]).output(); ... window.emit("filmstrip-frame", ...) } })`. No `processes.lock().insert`, no cancel Arc passed.
**Session 1 xref:** #4 (unbounded `count`).
**Severity:** **MEDIUM** — item delete / nav does not stop in-flight loop; orphan ffmpegs continue running and emitting. Resource waste + post-hoc events.

### 12. Filmstrip + job-progress broadcast — two listeners per event, JS-side re-filter
**Lenses:** ARCH-005 · ARCH-006 · CONC-004 · PERF-004 (related waveform)
**Evidence:** `probe/filmstrip.rs:84` emits via `window.emit("filmstrip-frame", ...)` which is Tauri's broadcast variant. `QueueManager.svelte:195-208` registers a persistent bg-filter listener; `Timeline.svelte:725-747` registers an active-id listener. Both fire on every emit; both deserialize.
**Session 1 xref:** #6 (progress cadence).
**Severity:** **MEDIUM** — CPU waste on filmstrip frames (50-200 KB base64 × 20 × N items × 2 listeners). Fix via `emit_to` / scoped event names.

### 13. 4000 unkeyed SVG `<rect>` per waveform (frontend overrides backend default)
**Lens:** PERF-004
**Evidence:** `Timeline.svelte:692`: `invoke('get_waveform', { path: it.path, draft: isDraft, buckets: 4000 })`. Backend `probe/waveform.rs:56` default is 500.
**Session 1 xref:** #7 (`get_waveform` unbounded allocation — more buckets compound).
**Severity:** **MEDIUM** — O(4000) DOM nodes + IPC payload + reconciliation per waveform swap.

### 14. Timeline waveform collects entire decoded PCM stream into memory
**Lens:** session-1 #7 (re-surfaced via PERF compound with PERF-004)
**Evidence:** `probe/waveform.rs:38-54`: `Command::output()` blocks until ffmpeg exits collecting full f32le stream. 1-hour 8 kHz mono ≈ 115 MB RAM.
**Session 1 xref:** #7 (direct).
**Severity:** **MEDIUM** — user-reachable OOM on long files.

### 15. `write_fade_log` RMW race — concurrent job finalizers drop entries
**Lens:** session-1 #8 · CONV-010 · CONV-022 (related)
**Evidence:** `lib.rs:420-431`: read full file → parse lines → push → rewrite. No lock, no O_APPEND. Called from worker finalizer which can run concurrently across batch jobs. Same-file sibling `diag_append` (`lib.rs:784`) uses `OpenOptions::append(true)` — shows the right idiom already exists in-house.
**Session 1 xref:** #8 (direct).
**Severity:** **MEDIUM** — observability gap; diagnostic log silently drops entries under batch.
**Correction:** `diag_append` also rotates (size-threshold rename at `lib.rs:787-792`). CONV-022's "no rotation" claim is wrong; the divergence is between rotation strategies, not rotation vs none.

---

## WEAKENED findings

| Finding | Original claim | Corrected claim | Impact on fix plan |
|---|---|---|---|
| PERF-001 | "15+ sites call `run_ffprobe` + `probe_duration`" | 4 ops double-probe (rewrap, extract, replace_audio, conform) | Scope is 4-site refactor, not 15 |
| PERF-002 | EBU loudnorm = 3 spawns, removable | EBU is inherently 2-pass (ffmpeg measure→apply); only `probe_duration` extra spawn removable | Reduction: 3→2, not 3→1 |
| PERF-006 | `check_tools` / `tool_available` MEDIUM | Cold-path startup/settings UI probe; LOW | De-prioritize |
| PERF-008 | stderr churn MEDIUM | Cold-path allocation (only used on error); LOW | De-prioritize |
| PERF-009 | `args/video.rs` 133 `.to_string()` MEDIUM | Per-conversion µs cost vs minutes of ffmpeg work; not a real finding | Drop |
| PERF-010 | conform/merge clone-heavy probe MEDIUM | Probe JSON is kB, happens once per op; micro | Drop |
| PERF-015 | bg preload "20×20 ffmpegs" | Serialized via `_bgBusy`; sequential over time, not peak fanout | Still-expensive-cumulatively; framing fixed |
| PERF-018 | subtitle lint `.chars().count()` 3× per cue | 2× per line inside cue; `:194` is a different computation | Smaller fix |
| PERF-019 | Preview sliders "no debounce" | Debounce exists via setTimeout; real gap is stale-result discard → merges under CONC-012 | Reframe as concurrency not perf |
| CONC-009 | Mutex poison cascade "undead app" | No user code runs inside lock; poison not reachably triggered; switch to `parking_lot` on principle | Downgrade MEDIUM→LOW |
| CONC-010 | Global mutex "bottleneck per progress line" | Only at insert/remove per worker; progress loop does not touch mutex | Small contention, not the thing to fix first |
| CONC-014 | `_bgBusy` "allows two concurrent preloads" | Single-threaded JS can't race past the gate; behavior is intentional fire-and-forget of filmstrip emits | Non-bug |
| CONV-006 | `unwrap_used=warn` unenforced; 176 sites | CI runs `clippy -- -D warnings` (ci.yml:94); 102 actual sites; enforcement status indeterminate without CI history | Needs CI-history check before acting |
| CONV-013 | 9 `.expect("…mutex poisoned")` sites | 10 sites; "consistent but panics" framing fair | Counting nit |
| CONV-022 | `diag_append` = "O_APPEND no rotation" | `diag_append` **does** rotate (byte-threshold rename); divergence is strategy, not rotation-vs-not | Reframe |
| CONV-023 | `media_type_for` import "marked unused" as shadow dup | `#[allow(unused_imports)]` for test consumers, not dead code | Non-bug |
| ARCH-001 | 478-line god-dispatch is a bug | Line/variant counts correct; "field rename breaks wire format" is serde-tagged-enum universal, not fixed by trait layer | Architectural taste, not a defect |
| ARCH-010 | `ConvertOptions` 95 fields, every op sees all | ~87 fields; only `convert_file` consumes full struct; mechanical ops bypass via `OperationPayload` | Reduces blast radius by 29 ops |
| ARCH-012 | Capability `core:default` = every command callable from any origin | Category error — Tauri 2 doesn't gate app commands via capability permissions. `assetProtocol.scope:["**"]` component valid; rest rejected | Kill the capability-scope angle; keep asset-protocol scope + ARCH-004 |
| ARCH-015 | 43 `let _ = window.emit` | Actual 41; requires window-destroyed path to manifest as failure | Counting nit |
| "30 commands" | 30 registered | Actual 32 (`#[tauri::command]` + `#[command]`) | Counting nit |

---

## REJECTED findings

| Finding | Why rejected |
|---|---|
| **CONC-003** filmstrip listen-after-invoke race | Backend `thread::spawn → nice → ffmpeg seek+decode → base64` takes ≥100 ms before first emit; `listen()` Promise resolves in a microtask tick. No realistic window for the listener to miss the first frame. Author comment at call site ("Subscribe before invoking so no frames are missed") shows ordering was already considered. Other lenses (ARCH-005, PERF-015, multi-agent table) inherited this claim — they should not rely on it. |
| **ARCH-012 IPC "every command callable from any webview origin"** | Tauri 2's capability permission system gates plugin commands (`core:*`, `fs:*`, `shell:*`), not user-defined `#[command]` handlers from `generate_handler!`. `core:default` is the standard minimum; granting it is not an over-permission. The real trust-boundary issue is absence of Rust-side path validation (ARCH-004), which stands independently. |

---

## NEEDS-EVIDENCE — explicit followups, not fixes

These are plausible but the critic could not verify without more context. Session 4 should either run the specific check before including them in the fix plan, or drop them.

1. **CONC-007** — cancel_job kill + worker `remove_file` race on reused output_path. Window is small (post-wait → CANCELLED branch). Needs a scripted reproduction: rapid cancel→re-convert with identical output_path.
2. **CONC-015** — unbatched diag_append. Referenced `src/lib/utils/diagnostics.svelte.js:55` not found at that exact path in this checkout. Locate the actual file and verify the fire-and-forget per-error pattern.
3. **PERF-016** — preset save RMW with YAML parse. `read_presets`/`write_presets` live in `librewin_common::config` (external crate). Open that crate and confirm: YAML vs JSON format, in-memory cache presence, locking.
4. **CONV-012** — error-prefix drift ("`<tool> not found:`"). Pattern is visible; impact is cosmetic. Worth a grep-count before deciding whether to centralize.
5. **ARCH-007** — `analyze_vmaf`/loudness/cut_detect/black_detect/framemd5 uncancellable. Per-command confirmation of absent Child registration needed (opening each analysis file). Divergent lifecycle is already confirmed (ARCH-003); this is just the per-command inventory.
6. **ARCH-011** — probe data reshaped in 5 places (FileInfo/StreamInfo/SubStream). Claim plausible but not opened.
7. **ARCH-016** — dup allowlist `classify_ext` (Rust) vs `mediaTypeFor` (JS); `sqlite`/`parquet` missing from JS. Rust side confirmed (`lib.rs:364,507`); JS side not opened.
8. **ARCH-017** — dup helpers `ext_of` + `tool_in_path`. Not examined.
9. **ARCH-020** — no Rust module cycles. INFO-only; no cycle tool run.

---

## Cross-lens agreement register (multi-source ≥2 lenses + critic-CONFIRMED)

The highest-trust items for session 4:

| Site | Lenses converging | Status after critic |
|---|---|---|
| `operations/mod.rs:129` ↔ `merge.rs:201` ↔ `convert/subtitle.rs:121` (run_ffmpeg 3×) | PERF-007 · CONV-002 · ARCH-002 · (CONC-008 adjacent) | **CONFIRMED** |
| `lib.rs:454` ↔ `lib.rs:1025` (convert_file vs run_operation diverged) | CONV-003 · ARCH-003 | **CONFIRMED** |
| `lib.rs:325` scope gap + missing `validate_output_name` | session-1 #3 · ARCH-004 · CONV-021 | **CONFIRMED** |
| `convert/data.rs:224` serde_yml reachable | ARCH-008 · session-1 #10 | **CONFIRMED (was NEEDS-EVIDENCE in session 1)** |
| `lib.rs:547/666/733` `.expect("mutex poisoned")` | CONV-013 · CONC-009 · session-1 #5 | **WEAKENED** — sites correct, cascade scenario not reachable |
| `probe/filmstrip.rs:44,84` | PERF-015 · CONC-006 · ARCH-005 | **CONFIRMED** for orphan + broadcast; **WEAKENED** for preload concurrency |
| `App.svelte:910` batch fanout | CONC-002 | **CONFIRMED** |
| `App.svelte:669` post-cancel flicker | CONC-013 | **CONFIRMED** |
| `PresetManager.svelte:17` ↔ `presets.rs:16` type drift | ARCH-009 | **CONFIRMED** |

---

## Methodology notes / coverage

- **Critic model:** 4 parallel general-purpose agents, one per lens. Each opened the cited files directly before rendering verdicts. No verdict was issued from summary alone. Raw transcripts appended below.
- **Not covered:** critic did not re-run Semgrep / cargo-audit (session 1's baseline stands); did not open `librewin_common` external crate (relevant to PERF-016); did not inspect CI run history (relevant to CONV-006).
- **Rebuttal phase skipped.** 9 NEEDS-EVIDENCE items instead of a rebuttal round — session 4 can either resolve them or drop them. Rejection count is low (2), so rebuttal would have been low-yield.

---

## Appendix — Raw critic transcripts

### Performance critic
See task output `a87c92af91b5216f0` (full transcript retained in session log). Key quotes preserved in the findings above; verdict breakdown: 13 CONFIRMED / 7 WEAKENED / 0 REJECTED / 1 NEEDS-EVIDENCE.

### Concurrency critic
See task output `a901ad6f3b4ba831a`. Verdict breakdown: 14 CONFIRMED / 2 WEAKENED / 1 REJECTED / 2 NEEDS-EVIDENCE. Headline kill: **CONC-003 REJECTED** (listen-after-invoke race unreachable). Headline downgrade: **CONC-009 WEAKENED** (poison cascade narrative overstated).

### Convention critic
See task output `a4d3282ad9937bbe7`. Verdict breakdown: 19 CONFIRMED / 5 WEAKENED / 0 REJECTED / 1 NEEDS-EVIDENCE. Headline corrections: **CONV-006** (CI does run `clippy -D warnings`, 102 sites not 176); **CONV-022** (`diag_append` does rotate); **CONV-023** (media_type_for import is deliberately unused for tests).

### Architecture critic
See task output `ac625ec9d55e8bd1b`. Verdict breakdown: 11 CONFIRMED / 4 WEAKENED / 1 REJECTED / 5 NEEDS-EVIDENCE. Headline kill: **ARCH-012** capability "every command callable" component rejected as a Tauri-2 category error; `assetProtocol.scope: ["**"]` survives.

---

*End of session 3. Session 4 (synthesis + fix plan) consumes 01 + 02 + this file.*
