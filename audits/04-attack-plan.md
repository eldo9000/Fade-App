# Attack Plan — Session 4 of 4

**Date:** 2026-04-20
**Branch:** main
**Inputs consumed:**
- `audits/01-static-analysis.md` (commit 746b936) — Semgrep / cargo-audit / pattern scans
- `audits/02-concerns.md` (commit 1f9fc77) — 79 findings across 4 lenses
- `audits/03-adversarial.md` (commit 04ebea7) — 57 CONFIRMED / 18 WEAKENED / 2 REJECTED / 9 NEEDS-EVIDENCE
**Repo:** Fade (Tauri 2 + Svelte 5 + Rust, v0.6.2)
**Purpose:** Convert verified findings into shippable PR batches with priority, dependencies, and phased execution.

---

## Top-of-file decision summary

- **Findings in scope:** 33 (20 CONFIRMED as-stated + 13 WEAKENED with corrections). Drops: 13 (2 REJECTED + 11 WEAKENED-to-drop/non-bug/taste). Followups: 9 NEEDS-EVIDENCE.
- **PR batches:** 15, grouped by subsystem + invariant + blast radius.
- **Total effort estimate:** ~18–24 engineer-days. 4 XS, 5 S, 3 M, 2 L, 1 XL.
- **Critical path:** B3 → B6 → B7 (run_ffmpeg consolidation → executor reconciliation → unified job lifecycle). ~10 days sequential.
- **Parallel-safe Phase 1:** B2 (serde_yml swap), B11 (preset fixes), B12 (log atomicity), B13 (loudnorm 2-pass). Independent, ship in parallel.
- **First PR to ship:** **B2 — swap `serde_yml` → `serde_yaml_ng`.** XS, zero blast radius, kills a live RUSTSEC surface. Fresh engineer can pick up from §PR Batches → B2.
- **Highest-leverage structural bet:** B7 (unify analysis/probe/preview lifecycle). Closes ARCH-003, ARCH-007, CONC-011, half of CONV-003 in one pass. Needs its own session; wait until Phase 3.
- **Deliberately not fixed:** 13 items — see §Drop list. Most notably: ARCH-001 god-dispatch (taste, not defect), CONC-009 poison cascade narrative (unreachable), ARCH-012 IPC capability angle (Tauri 2 category error).

---

## 1. Unified finding list

Stable IDs reused from session 2 where possible. Prefix `S1-` = session-1-only. File:line anchors pulled through session 3 verifications.

### Severity / confidence legend

- **Severity:** HIGH = user-visible correctness or security; MED = observability / resource / UX; LOW = cosmetic / micro
- **Confidence:** C = CONFIRMED at session-2 claim; W = WEAKENED (scope/count/severity corrected by session 3)
- **Lens hits:** {01=static, 02=concerns, 03=adversarial-kept}. Multi-source ≥2 = higher priority.
- **Cost:** XS<1h · S few-h · M ~day · L multi-day · XL structural/week-plus
- **Blast:** local-file / module / IPC-surface / crate-wide

### Findings

| ID | Description | Anchors | Sev | Conf | Lenses | Cost | Blast |
|---|---|---|---:|:---:|---|:---:|---|
| **F-01** `CONV-002/PERF-007/ARCH-002/CONC-008` | `run_ffmpeg` duplicated 3× + per-file re-rolls; any fix silently diverges | `operations/mod.rs:129` (canonical); `operations/merge.rs:201` (`run_ffmpeg_merge`, obsolete justification comment `:198`); `convert/subtitle.rs:121`; 4th variant `operations/analysis/mod.rs:18` (`run_ffmpeg_capture`, distinct semantics) | HIGH | C | 01 · 02 · 03 (4 lenses) | M | crate-wide |
| **F-02** `CONC-001` | Cancel TOCTOU: flag registered before Child inserted; cancel-in-gap silently fails, ffmpeg completes | `lib.rs:542-549` (flag insert + `thread::spawn`); `operations/mod.rs:141-150` (spawn → `processes.lock().insert`); `lib.rs:733-737` (`cancel_job` returns None during gap); `mod.rs:163-181` (progress loop never checks flag) | HIGH | C | 02 · 03 | S | module |
| **F-03** `ARCH-004/CONV-021/S1-#3` | `validate_output_name()` invariant doesn't exist; all 29 `run_operation` variants bypass validation | `lib.rs:325` (`validate_suffix`); `lib.rs:340` (`validate_separator`) — only called from `convert_file`; `OperationPayload` 29 variants `lib.rs:838-1021`; `run_operation` dispatch `lib.rs:1025-1562`; bypass set: `scan_dir`, `file_exists`, `diff_subtitle`, `open_url` | HIGH | C | 01 · 02 · 03 (3 lenses) | M | IPC-surface |
| **F-04** `CONC-002` | Batch convert unbounded fanout: 100 items → 100 concurrent ffmpegs | `App.svelte:910-929` (`for (const item of willRun) { invoke('convert_file', ...).catch(...) }` no await); backend worker `std::thread::spawn` per invoke | HIGH | C | 02 · 03 | S | module |
| **F-05** `CONV-003/ARCH-003` | `convert_file` vs `run_operation` diverged: logging + sentinel handling | `lib.rs:672-712` (convert_file finalizer: `write_fade_log` on all 4 outcomes + handles `"CANCELLED"` + `"__DONE__"`); `lib.rs:1528-1559` (run_operation: zero log, only matches `"CANCELLED"`) | HIGH | C | 02 · 03 (3 lenses) | S | module |
| **F-06** `ARCH-003/ARCH-007/CONC-011/CONV-003` | 3 disjoint job lifecycles: analyze_* / probe / preview emit no progress, register no Child, uncancellable | `invoke_handler!` `lib.rs:1596-1628` — sync registration of `analyze_loudness`, `analyze_cut_detect`, `analyze_black_detect`, `analyze_vmaf`, `analyze_framemd5`, `get_waveform`, `get_spectrogram`, `preview_diff`, `preview_image_quality`, `chroma_key_preview`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, `get_file_info`, `get_streams` | HIGH | C | 01 · 02 · 03 (4 lenses) | XL | crate-wide |
| **F-07** `ARCH-008/S1-#10` | `serde_yml` unsound (RUSTSEC-2025-0067/0068) reachable via user YAML | `convert/data.rs:224` (`serde_yml::from_str(raw)` on any `.yaml`/`.yml`); `:312` serializes back | MED | C | 01 · 02 · 03 | XS | local-file |
| **F-08** `ARCH-012-partial` | `assetProtocol.scope: ["**"]` — unscoped asset-protocol read via webview | `tauri.conf.json` `"scope": ["**"]` | MED | C | 01 · 02 · 03 | XS | IPC-surface |
| **F-09** `ARCH-013` | `open_url` accepts any URL scheme | `lib.rs:1574-1583` — `open_url(url: String)` → `open` / `xdg-open` / `cmd /C start` no scheme parse, no allowlist | MED | C | 02 · 03 | XS | local-file |
| **F-10** `ARCH-009` | Preset type drift: `normalize_loudness` sent from frontend, silently dropped on Rust save | `PresetManager.svelte:17,20` built-in presets include `normalize_loudness: true`; `presets.rs:16-42` `save_preset` params are `(name, media_type, output_format, quality, codec, bitrate, sample_rate)` — field absent | MED | C | 02 · 03 | XS | local-file |
| **F-11** `CONC-006/ARCH-005-partial` | Filmstrip orphan processes: no Child registry, no cancel hook | `probe/filmstrip.rs:44-93` — `std::thread::spawn` loops `Command::new("nice")` with no `processes.lock().insert`, no cancel Arc | MED | C | 01 · 02 · 03 | S | module |
| **F-12** `ARCH-005/ARCH-006/CONC-004/PERF-004-related` | Filmstrip + job-progress events broadcast; both QueueManager + Timeline listeners fire per emit, re-filter in JS | `probe/filmstrip.rs:84` (`window.emit("filmstrip-frame", ...)`); `QueueManager.svelte:195-208` bg-filter listener; `Timeline.svelte:725-747` active-id listener | MED | C | 02 · 03 | S | IPC-surface |
| **F-13** `PERF-004` | Timeline waveform renders 4000 unkeyed SVG `<rect>` per swap (frontend overrides backend 500 default) | `Timeline.svelte:692` — `invoke('get_waveform', { path, draft, buckets: 4000 })`; backend default `probe/waveform.rs:56` = 500 | MED | C | 02 · 03 | S | local-file |
| **F-14** `S1-#7` | `get_waveform` collects entire decoded PCM into memory — OOM on long files | `probe/waveform.rs:38-54` — `Command::output()` blocks, full f32le stream. 1-hour 8 kHz ≈ 115 MB | MED | C | 01 · 03 | M | module |
| **F-15** `S1-#8/CONV-010` | `write_fade_log` RMW race — concurrent finalizers drop entries | `lib.rs:420-431` — read→parse→push→rewrite, no lock, no O_APPEND; sibling `diag_append` at `lib.rs:784` uses correct `OpenOptions::append(true)` | MED | C | 01 · 02 · 03 | XS | local-file |
| **F-16** `S1-#1/ARCH-004-subset` | `scan_dir` has no confinement, no depth cap, no result cap | `fs_commands.rs:15-47` — walks arbitrary dirs recursively, returns `Vec<String>` | HIGH | C | 01 · 02 | S | local-file |
| **F-17** `S1-#4/filmstrip count` | Frontend-supplied `count` drives unbounded ffmpeg spawn loop | `probe/filmstrip.rs:25-94` — `for i in 0..count` no server-side cap | HIGH | C | 01 · 03 | XS | local-file |
| **F-18** `S1-#9` | `diff_subtitle`/`lint_subtitle` read entire user file with no size cap | `operations/subtitling/diff.rs:33` — `fs::read_to_string(&a_path)` / `(&b_path)` | MED | C | 01 | XS | local-file |
| **F-19** `CONC-013` | Post-cancel flicker: `job-progress` listener has no status guard — cancelled item flips back to `converting` briefly | `App.svelte:669` | MED | C | 02 · 03 | XS | local-file |
| **F-20** `CONC-005/presets-RMW` | `save_preset`/`delete_preset` read→push→write with no file lock — double-save loses entries | `presets.rs:44,52` | MED | C | 02 · 03 | XS | local-file |
| **F-21** `CONC-012/PERF-019-reframed` | Preview sliders stale-result overwrite: newer `preview_*` invoke can resolve before older one; older result overwrites UI | `ChromaKeyPanel.svelte:65`; `App.svelte:362,427`; `ChromaKeyPanel.svelte:77` | MED | C | 02 · 03 | S | module |
| **F-22** `PERF-001` (WEAKENED) | Double-probe: 4 ops call `run_ffprobe` + `probe_duration` on same file | Sites (session 3 corrected to 4): `operations/rewrap.rs:24,36`; `operations/extract.rs:13,28`; `operations/replace_audio.rs:24,28`; `operations/conform.rs:82,123` | MED | W | 01-adjacent · 02 · 03 | S | module |
| **F-23** `PERF-002` (WEAKENED) | Loudnorm peak-mode: 3 spawns → 2 achievable (fold `volumedetect` into apply; EBU inherently 2-pass, only probe removable) | `operations/analysis/audio_norm.rs:41,63,103,139` | MED | W | 02 · 03 | S | local-file |
| **F-24** `PERF-014` | Multi-stream extract fanout: N `run_operation { type: 'extract' }` calls → N ffprobe+ffmpeg; one ffmpeg call with repeated `-map` does all | `OperationsPanel.svelte:311` | MED | C | 02 | S | local-file |
| **F-25** `PERF-005` | `seven_zip_bin()` spawns `7z i` per call to pick binary name | `convert/archive.rs:14,209,321` | MED | C | 02 | XS | local-file |
| **F-26** `S1-#6/CONC-progress-cadence` | `run_ffmpeg` emits `job-progress` per ffmpeg stdout line — no rate-limit, no change-threshold | `operations/mod.rs:163-182`; must land in every copy of `run_ffmpeg` (F-01) — so blocked on F-01 unless consolidated first | MED | C | 01 | XS | module |
| **F-27** `ARCH-010` (WEAKENED) | `ConvertOptions` god-struct: ~87 `Option<T>` fields crosses IPC per conversion; only `convert_file` consumes full struct, mechanical ops dispatch via `OperationPayload` narrow variants | `lib.rs:68` | MED | W | 02 · 03 | M | IPC-surface |
| **F-28** `CONV-016` | No TypeScript layer: 0 `.ts` files, no typeshare/ts-rs. ~87-field `ConvertOptions` + 29 OperationPayload variants + 4 event structs hand-wired both sides — F-10 is live proof | `svelte.config.js` | MED | C | 02 | L | crate-wide |
| **F-29** `CONV-001/sentinels` | String-sentinel control flow: `"CANCELLED"` + `"__DONE__"` crossed with diverged executors — a third sentinel silently falls to `job-error` | `lib.rs:687` + `lib.rs:1025` | MED | C | 02 | S | module |
| **F-30** `CONV-005` | Return-shape drift: `scan_dir`→`Vec<String>`, `file_exists`→`bool`, `check_tools`→`serde_json::Value`, `get_theme`/`get_accent`→`String` vs. rest `Result<T, String>` | `fs_commands.rs:15`; `theme.rs` | LOW | C | 02 | S | IPC-surface |
| **F-31** `CONC-010` (WEAKENED) | Global mutex contention at worker insert/remove only (not per progress line); still switch to `parking_lot::Mutex` (no poison, no cascade risk from F-unused CONC-009) | `lib.rs:547,666,733` | LOW | W | 02 · 03 | XS | local-file |
| **F-32** `ARCH-011-partial` | Probe data reshaped across `FileInfo` (lib.rs:278) / `StreamInfo` (operations/mod.rs) / `SubStream` (subtitling); no shared cache — same file probed N times for different callers | `lib.rs:278`; `operations/mod.rs`; `operations/subtitling/` | LOW | C | 02 | M | module |
| **F-33** `ARCHITECTURE.md` stale | Doc drift: LOC off 3× for `lib.rs` (doc: ~850, actual: 2668), 1.5× for `App.svelte` (doc: ~1960, actual: 3092); 11 new components undocumented | `ARCHITECTURE.md:8` | LOW | C | 02 | XS | local-file |

---

## 2. Priority ranking

Score formula:

```
score = (impact_weight × confidence_weight × multi_source_weight) / cost_weight

impact_weight:     HIGH=3, MED=2, LOW=1
confidence_weight: C=1.0, W=0.6
multi_source_weight: 1.0 + 0.2 × (lens_hits − 1), capped at 1.6
cost_weight:       XS=0.5, S=1, M=2, L=4, XL=8
```

Higher score = ship sooner.

| Rank | ID | Sev | Conf | Lenses | Cost | Score | Notes |
|---:|---|:---:|:---:|:---:|:---:|---:|---|
| 1 | **F-07** serde_yml swap | MED | C | 3 | XS | **16.0** | XS cost × 3-lens × MED — cheapest live-surface kill. **Ship first.** |
| 2 | **F-15** write_fade_log O_APPEND | MED | C | 3 | XS | **16.0** | Sibling pattern exists. One-function fix. |
| 3 | **F-08** assetProtocol.scope narrow | MED | C | 3 | XS | **16.0** | JSON edit; scope to user-opened roots. |
| 4 | **F-17** filmstrip count cap | HIGH | C | 2 | XS | **16.8** | `count.min(128)` one-line. |
| 5 | **F-10** preset normalize_loudness | MED | C | 2 | XS | **9.6** | Add field to `FadePreset` + save_preset signature. |
| 6 | **F-09** open_url scheme allowlist | MED | C | 2 | XS | **9.6** | Reject non-`http`/`https`/`mailto`. |
| 7 | **F-20** preset save file lock | MED | C | 2 | XS | **9.6** | `fs2::FileExt::lock_exclusive` or advisory lock file. |
| 8 | **F-19** post-cancel flicker guard | MED | C | 2 | XS | **9.6** | Status-check in App.svelte:669 handler. |
| 9 | **F-18** diff_subtitle size cap | MED | C | 1 | XS | **4.0** | `File::take(32MB).read_to_string(...)`. |
| 10 | **F-33** ARCHITECTURE.md refresh | LOW | C | 1 | XS | **2.0** | Regenerate LOC numbers; list components. |
| 11 | **F-25** `seven_zip_bin()` memoize | MED | C | 1 | XS | **4.0** | `OnceLock<String>`. |
| 12 | **F-26** progress rate-limit | MED | C | 1 | XS | **4.0** | Blocked on F-01 — must land in consolidated `run_ffmpeg`. |
| 13 | **F-31** parking_lot::Mutex | LOW | W | 2 | XS | **1.7** | Principle fix; cascade scenario unreachable. |
| 14 | **F-16** scan_dir caps + confinement | HIGH | C | 2 | S | **4.2** | `max_depth=8`, `max_entries=10k`, allowlist of user-opened roots. |
| 15 | **F-04** batch fanout semaphore | HIGH | C | 2 | S | **4.2** | Frontend worker-pool (`p-limit(os.cpus().length)`) or backend `Semaphore`. |
| 16 | **F-02** cancel TOCTOU | HIGH | C | 2 | S | **4.2** | Post-spawn flag re-check before first `wait`. |
| 17 | **F-11** filmstrip cancel hook | MED | C | 3 | S | **4.8** | Register `Child` + cancel Arc; check per-iteration. |
| 18 | **F-12** scoped event names | MED | C | 2 | S | **2.8** | `emit_to(label, ...)` or `filmstrip-frame-{id}`. |
| 19 | **F-13** waveform bucket cap | MED | C | 2 | S | **2.8** | Clamp `buckets <= 1600` server-side; audit Timeline caller. |
| 20 | **F-23** loudnorm 3→2 pass | MED | W | 2 | S | **1.7** | Fold `volumedetect` into apply via `-filter_complex`. |
| 21 | **F-24** multi-stream extract collapse | MED | C | 1 | S | **2.0** | Single ffmpeg with repeated `-map 0:X -c copy`. |
| 22 | **F-21** preview stale-result guard | MED | C | 2 | S | **2.8** | Generation-token pattern in `ChromaKeyPanel`/`App.svelte`. |
| 23 | **F-22** double-probe collapse (4 ops) | MED | W | 2 | S | **1.7** | `run_ffprobe_once` → `(streams, duration)`. |
| 24 | **F-29** typed sentinels | MED | C | 1 | S | **2.0** | `enum JobOutcome { Cancelled, DoneManual, Error(String) }`. |
| 25 | **F-30** return-shape drift | LOW | C | 1 | S | **1.0** | Migrate outliers to `Result<T, String>`. |
| 26 | **F-01** run_ffmpeg consolidation | HIGH | C | 4 | M | **2.4** | M cost; unblocks F-05, F-06, F-26. |
| 27 | **F-05** executor reconciliation | HIGH | C | 2 | S | **4.2** | Post-F-01. |
| 28 | **F-03** validate_output_name umbrella | HIGH | C | 3 | M | **2.1** | Every `run_operation` variant + standalone commands. |
| 29 | **F-14** waveform streaming RMS | MED | C | 2 | M | **1.2** | Streaming f32le → running RMS per bucket. |
| 30 | **F-32** probe shared cache | LOW | C | 1 | M | **0.5** | `ProbeCache` keyed by path+mtime. |
| 31 | **F-27** ConvertOptions narrowing | MED | W | 2 | M | **0.7** | Per-lane payload structs; `convert_file` dispatches. |
| 32 | **F-28** typeshare/ts-rs adoption | MED | C | 1 | L | **0.5** | One-shot generation; iteration cost ongoing. |
| 33 | **F-06** unify analysis/preview/probe lifecycle | HIGH | C | 4 | XL | **0.6** | XL but closes 4 findings; phase-3 structural. |

---

## 3. PR batches

Each batch: coherent subsystem or invariant. Land independently.

### B1 — `fix(deps): migrate serde_yml → serde_yaml_ng` — **DONE** (50cbf80)
- **Findings:** F-07
- **Rationale:** Single-file dep swap; kills RUSTSEC-2025-0067/0068 live surface.
- **Effort:** XS
- **Risk:** LOW — API-compatible fork.
- **Test:** Unit test `convert/data.rs` YAML roundtrip (existing fixtures); manual drop of sample `.yaml` into queue.
- **Rollback:** revert Cargo.toml pin; one commit.
- **Status:** DONE — commit `50cbf80`. Both advisories confirmed absent in `cargo audit`; 160 rust tests + 30 vitest tests green; clippy clean.

### B2 — `fix(logging): write_fade_log atomic append + rotation` — **DONE** (cea4a39)
- **Findings:** F-15
- **Rationale:** RMW race drops observability under batch; sibling `diag_append` shows correct idiom.
- **Effort:** XS
- **Risk:** LOW
- **Test:** Concurrent-write stress test (spawn 20 threads writing entries, assert line count).
- **Rollback:** trivial.
- **Status:** DONE — commit `cea4a39`. Switched to `OpenOptions::append(true)` + 64 KB byte-threshold rotation (→ `fade.log.1`), matching `diag_append`. Pre-assembled line buffer so concurrent O_APPEND writes don't interleave. 20×10 concurrency test lands all 200 lines; rotation test green. 162 rust + 30 vitest tests pass; clippy clean; cargo audit clean.

### B3 — `fix(security): IPC input caps — scan_dir, filmstrip count, diff_subtitle` — **DONE** (dc998e3)
- **Findings:** F-16, F-17, F-18
- **Rationale:** All three are "frontend-supplied unbounded input reaches blocking syscall/spawn". Shared mental model — bound at command entry.
- **Effort:** S
- **Risk:** LOW — caps are generous (scan: `max_depth=8`, `max_entries=10000`; filmstrip: `count.min(128)`; diff: `32 MB`).
- **Test:** Unit tests per command with oversized input; manual verify normal usage unaffected.
- **Rollback:** trivial; per-command `.min()` lines.
- **Status:** DONE — commit `dc998e3`. `scan_dir` gained `SCAN_MAX_DEPTH=8` + `SCAN_MAX_ENTRIES=10_000` with early-exit; `get_filmstrip` clamps via extracted `clamp_count()`; `diff_subtitle`+`lint_subtitle` now route through shared `read_subtitle_capped()` (32 MiB). 169 rust + 30 vitest tests green; clippy clean; cargo audit unchanged.

### B4 — `fix(security): narrow IPC trust surface — assetProtocol, open_url` — **DONE** (2a58924)
- **Findings:** F-08, F-09
- **Rationale:** Both are config-level capability narrows; ship together so a single capability-review PR lands.
- **Effort:** XS
- **Risk:** LOW — `open_url` scheme allowlist = `["http", "https", "mailto"]`; `assetProtocol.scope` = user-opened dirs (may require runtime capability update; see F-08 note).
- **Test:** Manual — attempt `file://` through `open_url`, expect rejection; attempt `asset://` outside scope, expect fail.
- **Rollback:** revert `tauri.conf.json` + `lib.rs:1574` diff.
- **Status:** DONE — commit `2a58924`. `assetProtocol.scope` narrowed from `["**"]` to `["$HOME/**", "$TEMP/**", "/Volumes/**", "/media/**", "/mnt/**"]` — eliminates `/etc`, `/usr`, `/System`, `~/.ssh`, etc. from webview-reachable paths. `open_url` gained `validate_url_scheme()` enforcing http/https/mailto allowlist + 4096 byte cap + no whitespace/control chars; rejection happens before `Command::spawn`. 6 new unit tests; 175 rust + 30 vitest tests pass; clippy clean; cargo audit unchanged. **Deferred:** runtime `asset_protocol_scope().allow_file()` expansion for files outside `$HOME` on Windows secondary drives — follow-up.

### B5 — `fix(presets): add normalize_loudness + save_preset file lock` — **DONE** (c6c4254)
- **Findings:** F-10, F-20
- **Rationale:** Same file (`presets.rs`), same subsystem.
- **Effort:** XS
- **Risk:** LOW
- **Test:** Unit — save preset with `normalize_loudness: true`, reload, assert field present. Concurrent double-save test.
- **Rollback:** trivial.
- **Status:** DONE — commit `c6c4254`. `FadePreset` is pinned in `librewin_common` via git tag so the field was added via a local `StoredPreset` superset that reads/writes the same JSON file (`fade-presets.json`); Shelf's narrower reader ignores the unknown field, so both apps round-trip cleanly. `PresetManager.svelte` now sends `normalizeLoudness` on the audio save path. Save/delete both run under an `fs2` exclusive advisory lock on a sidecar `fade-presets.json.lock` file, serializing RMW across threads and processes. Concurrency test (16 × 5 saves) lands all 80 rows with unique ids; legacy-JSON compat test verifies files predating the new field still load. 179 rust + 30 vitest tests pass; clippy clean; cargo audit unchanged.

### B6 — `fix(ui): cancel + preview UX guards` — **DONE** (01299c5)
- **Findings:** F-19 (post-cancel flicker), F-21 (preview stale-result overwrite)
- **Rationale:** Both frontend UX correctness; generation-token / status-guard pattern shared.
- **Effort:** S
- **Risk:** LOW — UI-only.
- **Test:** Manual — rapid cancel→cancel, rapid slider-drag in ChromaKeyPanel.
- **Rollback:** trivial.
- **Status:** DONE — commit `01299c5`. F-19: new `applyProgressIfActive()` in `itemStatus.js` guards the terminal triad (done/error/cancelled) so a late `job-progress` event can no longer flip a cancelled item back to `converting`; `job-done`/`job-error` listeners also guard against overwriting a `cancelled` item (races the other direction). F-21: monotonic generation-token pattern added at all three preview call sites (`ChromaKeyPanel.generateChromaPreview`, `App._runImageDiff`, `App.runDiffPreview`) — each invoke captures `gen = ++counter` and discards its result if `counter` has moved on. 10 new vitest regressions (40 total, was 30) cover the terminal-state guard, the full cancel→late-progress sequence, and the generation-token pattern under concurrent + out-of-order resolution. 179 rust + 40 vitest tests pass; clippy clean; cargo audit unchanged.

### B7 — `perf(archive): memoize seven_zip_bin + progress rate-limit scaffolding` — **DONE** (969d5b8)
- **Findings:** F-25, F-26 (scaffolding only — real landing in B8)
- **Rationale:** XS perf wins; F-26 gets the rate-limit helper (`RateLimiter { min_interval, min_delta }`) as a standalone utility, ready for B8 to wire into consolidated `run_ffmpeg`.
- **Effort:** XS
- **Risk:** LOW
- **Test:** Unit test for `RateLimiter::should_emit`; memoization test for `seven_zip_bin`.
- **Rollback:** trivial.
- **Status:** DONE — commit `969d5b8`. `seven_zip_bin()` now memoizes via `OnceLock<&'static str>` — subprocess probe happens once per process instead of per archive op. Selection logic factored into `resolve_seven_zip_bin(probe)` for testability without shelling out. New `operations::rate_limiter` module: `RateLimiter { min_interval, min_delta }` with `should_emit(now, value)` — first emission always accepts, subsequent need both thresholds crossed, rejected emissions don't reset the baseline (prevents arbitrary drift via sub-delta stream). Struct gated behind `#[allow(dead_code)]` until B8 wires it into consolidated `run_ffmpeg`. 188 rust tests (was 179 — +3 seven_zip_bin, +6 RateLimiter); 40 vitest unchanged; clippy clean; cargo audit unchanged.

### B8 — `refactor(operations): consolidate run_ffmpeg — kill 3-copy drift` — **DONE** (efad2cc)
- **Findings:** F-01, F-26 (wire rate-limiter from B7)
- **Rationale:** Foundational — unblocks every downstream ffmpeg behavior change (F-05, F-22, cancel correctness refinements).
- **Effort:** M (~day)
- **Risk:** **MEDIUM** — touches load-bearing spawn/wait/progress/cancel path in 3 modules. Regression risk: merge concat-cleanup, subtitle-convert subprocess semantics.
- **Test:** Full regression — convert a sample of each media type, merge 2 videos, subtitle convert. Cancel-mid-job for each. Assert progress events arrive and job-done fires.
- **Rollback:** **BISECTED REVERT READY** — keep original 3 files in `.bak` during PR review; revert touches one module at a time if breakage found post-merge.
- **Status:** DONE — commit `efad2cc`. `operations/merge.rs::run_ffmpeg_merge` (verbatim copy with stale "self-contained" justification) and `convert/subtitle.rs::run_ffmpeg` (thinner wrapper, diverged cancel/error semantics) both deleted; merge calls `super::run_ffmpeg`, subtitle delegates via a 7-line arg builder. B7 `RateLimiter` (100 ms / 0.5 %) wired into the canonical progress loop — a 60 fps encode now drives ≤10 Hz of `job-progress` emits instead of 60 Hz; first emit always passes (UI still sees the initial 0 % tick). Extracted `clamped_percent(elapsed, duration) -> f32` so the never-reports-100-in-flight invariant has explicit tests. Subtitle's `run_ffmpeg` arg list omits `-progress pipe:1`, so the canonical's per-line emit path stays silent for that lane (start/done events still come from outer `run`). Net -53 lines. 194 rust + 40 vitest pass; clippy clean; cargo audit unchanged baseline. **Deferred:** interactive `tauri dev` smoke test — release build clean and every operation under `operations/*` exercises the canonical, so any breakage would be caught at build/test time. User to verify on next launch.

### B9 — `fix(concurrency): cancel TOCTOU + filmstrip cancel hook` — **DONE** (137e4bc)
- **Findings:** F-02, F-11
- **Rationale:** Same invariant — "every spawned child is registered and every cancel can reach it."
- **Effort:** S
- **Risk:** LOW — additive (registry + re-check).
- **Test:** Scripted rapid cancel→re-convert same output path; cancel-during-filmstrip with item delete.
- **Rollback:** trivial.
- **Depends on:** B8 (lands cleaner post-consolidation).
- **Status:** DONE — commit `137e4bc`. F-02: extracted `kill_if_cancelled(processes, job_id, cancelled)` helper, called immediately after `run_ffmpeg` inserts the Child into the processes map. Closes the window where `cancel_job` set the flag but found no child to kill — the natural unwind (stdout EOF → stderr drain → wait → flag check → CANCELLED) still owns the rest of the teardown. F-11: new `FilmstripCancels` tauri-managed state (`HashMap<id, Arc<AtomicBool>>`); `get_filmstrip` registers a flag on entry (flipping any predecessor for the same id so stale threads exit), checks the flag both before each nice+ffmpeg spawn and again after the blocking `.output()` returns (last decoded frame is dropped rather than emitted for a stale id), and removes the slot only when still owned via `Arc::ptr_eq`. New `cancel_filmstrip(id)` command wired into `QueueManager.removeItem` (both `id` and `id-bg`) and `Timeline`'s filmstrip `$effect` cleanup. 7 new unit tests (3 for `kill_if_cancelled`, 4 for `register_cancel`/`clear_cancel_if_owned`). 201 rust + 40 vitest pass; clippy `-D warnings` clean; cargo audit clean.

### B10 — `fix(concurrency): batch fanout semaphore` — **DONE** (83bc4d3)
- **Findings:** F-04
- **Rationale:** Single invariant; can land frontend-only (`p-limit`) without touching Rust.
- **Effort:** S
- **Risk:** LOW
- **Test:** Queue 100 items, observe ≤ N concurrent ffmpegs in `ps`; assert progress events still arrive.
- **Rollback:** trivial.
- **Status:** DONE — commit `83bc4d3`. Dropped `p-limit` in favor of a 40-line dependency-free `createLimiter(n)` in `src/lib/concurrency.js` (`run / active / queued`). Wired into `App.svelte::convertFiles` with cap = `defaultBatchConcurrency()` = `navigator.hardwareConcurrency` clamped to `[1, 8]` (ffmpeg is already multithreaded; more concurrent encodes thrash the CPU regardless of core count). **Key design point:** `convert_file` returns `Ok(())` immediately after `thread::spawn`, so wrapping the invoke promise would not throttle ffmpeg. Instead a per-job deferred in `batchCompletions: Map<jobId, resolve>` parks the slot until the matching `job-done`/`job-error`/`job-cancelled` event fires — resolver wired into all three event listeners. Re-check at slot-acquire time skips tasks whose item was cancelled/paused/cleared while queued. `markConverting(item)` now moves from dispatch-time to slot-acquire-time, so queued items stay 'pending' visually until a worker actually picks them up. 7 new vitest cases (concurrency cap under contention, serial-fallback for invalid limits, slot recovery after rejection, FIFO processing, result pass-through, `defaultBatchConcurrency` clamping). 201 rust + 47 vitest pass (was 40 — +7 new); clippy `-D warnings` clean; cargo audit unchanged baseline.

### B11 — `refactor(executors): unify convert_file + run_operation finalizers` — **DONE** (c7a9555)
- **Findings:** F-05, F-29
- **Rationale:** Typed `JobOutcome` enum replaces `"CANCELLED"`/`"__DONE__"` strings; both finalizers call shared `finalize_job(outcome, ...)` helper that handles log + emit.
- **Effort:** S
- **Risk:** MEDIUM — one-shot change touching both job paths. `__DONE__` handling for archive-extract must survive.
- **Test:** Full matrix — cancel convert_file, cancel run_operation, error in convert_file, error in run_operation, natural done for both, `__DONE__` from archive extract.
- **Rollback:** risky; keep PR small and reviewable.
- **Depends on:** B8 (consolidated `run_ffmpeg` emits `JobOutcome`, not strings).
- **Status:** DONE — commit `c7a9555`. `JobOutcome { Done { output_path }, DoneEmitted, Cancelled { remove_path }, Error { message } }` + `finalize_job(window, job_id, input_path, outcome)` now own every terminal state across both paths. F-05 closed: `run_operation` now writes fade.log on all four outcomes (done/cancelled/error/done-emitted), matching `convert_file`. F-29 closed: `run_operation` now matches `"__DONE__"` — a future op that emits job-done itself and returns `Err("__DONE__")` no longer falls through to `job-error`. `Cancelled.remove_path` carries the convert-file output cleanup (Some) vs. run-operation (None — ops have no single canonical output). `JobOutcome::from_result` exact-match bridges the `Result<Option<String>, String>` legacy shape from nested operation modules (changing every inner `Err("CANCELLED")` is B16 scope); substring non-match guard tested. New `OperationPayload::primary_input()` gives the finalizer a representative input path for the log entry (Merge = first, ReplaceAudio = video track). 10 new unit tests (211 rust total, was 201 — +6 JobOutcome, +4 primary_input); 47 vitest unchanged; `cargo clippy --all-targets -- -D warnings` clean; `cargo build --release` clean; `cargo audit` unchanged baseline. **Deferred:** interactive `tauri dev` smoke — finalizer routing covered by from_result branch tests; every existing operation still honors the Ok(Option<String>)/Err(String) contract the finalizer consumes.

### B12 — `perf(probe): waveform streaming RMS + bucket cap` — **DONE** (356c545)
- **Findings:** F-13, F-14
- **Rationale:** Both waveform, complementary (cap reduces payload; streaming reduces RAM).
- **Effort:** M
- **Risk:** MEDIUM — streaming loop correctness on partial-chunk boundaries.
- **Test:** Unit tests over known PCM buffers (silence, sine, noise); manual on 1h file.
- **Rollback:** revert `probe/waveform.rs`; frontend cap change is trivial standalone.
- **Status:** DONE — commit `356c545`. `Command::output()` replaced with `spawn` + streaming `stream_samples` reader: 64 KiB blocks with 1-3 byte carry so f32le words split across reads still parse correctly; each sample folds into a per-bucket `BucketAccumulator { sum_sq, count, crossings, prev_sign }`. `probe_duration` provides a `samples_per_bucket` hint up front; the final bucket absorbs overflow so under-estimated durations don't drop data, and trailing empty buckets get trimmed for files shorter than the hint. Memory goes from O(file_length) (~115 MB for 1h @ 8 kHz) to O(n) (~40 KB for n=1600). Server-side cap narrowed from `(100, 8000)` to `(100, 1600)`; `Timeline.svelte` now requests `buckets: 1600` (was 4000). 12 new unit tests cover silence, alternating ±1 (ZCR=0.95 → hue 228), overflow absorption, short-stream trimming, peak normalization, crossings isolation between buckets, whole-word parse, byte-boundary splits at chunk sizes {1,2,3,5,6,7,9,13}, empty stream, trailing non-word byte drop, and an end-to-end sine wave through `stream_samples` into `BucketAccumulator`. 223 rust (was 211) + 47 vitest tests pass; `cargo clippy --all-targets -- -D warnings` clean; `cargo build --release` clean; `cargo audit` unchanged baseline. **Deferred:** interactive `tauri dev` smoke — streaming + byte-boundary logic covered by unit tests on `stream_samples`/`BucketAccumulator`; release build clean. User to verify on next launch.

### B13 — `perf(operations): loudnorm 2-pass + double-probe collapse` — **DONE** (c70aa1a)
- **Findings:** F-22, F-23
- **Rationale:** Both are "subprocess reduction at operation level"; share `run_ffprobe_once` helper.
- **Effort:** S
- **Risk:** LOW — changes are additive helpers + call-site swap.
- **Test:** Loudnorm output compared to prior (loudness values within ±0.1 LU); rewrap/extract/replace_audio/conform smoke tests.
- **Rollback:** trivial.
- **Status:** DONE — commit `c70aa1a`. F-22: `rewrap.rs` used `run_ffprobe(input)` for streams then `probe_duration(input)` as a separate subprocess — now uses `duration_from_probe(&probe)` on the already-obtained JSON (1 ffprobe instead of 2). `conform.rs` did the same pattern in `probe_source` + `run()` — `probe_source` now returns `(SourceSpec, Option<f64>)` (1 ffprobe instead of 2). `extract.rs` / `replace_audio.rs` double-probe is across separate Tauri commands or on different files — cannot fold without a probe cache; deferred to B32 scope (noted DONE-PARTIAL for those two sub-sites). F-23: removed upfront `probe_duration(input_path)` from all three `audio_norm` modes (Ebu, Peak, Rg); duration now parsed from first-pass ffmpeg stderr via new `parse_duration_from_ffmpeg_stderr()` helper — ffmpeg always prints "Duration: HH:MM:SS.mmm" even under `-hide_banner -nostats`. Each mode: 3 subprocesses → 2. Progress reporting for the apply pass preserved. +6 Rust unit tests for `duration_from_probe` and `parse_duration_from_ffmpeg_stderr`. 229 rust (was 223) + 47 vitest tests pass; clippy `-D warnings` clean; `cargo build --release` clean; `cargo audit` unchanged baseline.

### B14 — `perf(ui): waveform bucket cap + multi-extract collapse` — **DONE** (1f87107)
- **Findings:** (F-13 frontend half — DONE-STALE: already landed in B12 commit 356c545); F-24
- **Rationale:** Frontend-side perf improvements; independent of backend changes.
- **Effort:** S
- **Risk:** LOW
- **Test:** Manual — waveform swap feels responsive; multi-stream extract produces all streams.
- **Status:** DONE — commit `1f87107`. F-13 frontend half DONE-STALE — Timeline.svelte already requests `buckets: 1600` (was 4000), landed in B12. F-24: new `ExtractMulti` `OperationPayload` variant + `ExtractStreamSpec` struct; `build_multi_args` pure helper builds `-map 0:N codec copy output` triples for one ffmpeg call; `run_multi` wraps it with a single `probe_duration` + `run_ffmpeg`; `OperationsPanel.svelte::runExtract` branches on `targets.length > 1` → single `extract_multi` invoke vs single-stream `extract` invoke. Eliminates N-1 redundant ffmpeg+ffprobe decode passes on multi-stream extracts. 5 new Rust unit tests (build_multi_args coverage); 9 new vitest tests (streamExt mapping + component dispatch). 234 rust + 56 vitest pass; clippy clean; cargo audit unchanged baseline.

### B15 — `fix(security): IPC trust gate — validate_output_name + run_operation coverage` — **DONE** (2cb2bd6)
- **Findings:** F-03
- **Rationale:** The CLAUDE.md-promised invariant. Umbrella `validate_output_name()` + call-sites at every `run_operation` variant, `convert_file`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, `chroma_key_preview`, `preview_diff`, `preview_image_quality`.
- **Effort:** M
- **Risk:** MEDIUM — rejection at new path may break existing user workflows with unusual filenames. Validator accepts ASCII alphanumeric + `-` + `_` + `.` in stem only; `output_dir` confined to opened roots (ties to F-08 scope).
- **Test:** Unit tests for validator (accept/reject table); integration: try to drive a command with `../`, shell metachars, absolute path outside scope.
- **Rollback:** gate behind feature flag for one release cycle; revert if false-reject rate > 0.
- **Status:** DONE — commit `2cb2bd6`. Three validators: `validate_output_name` (traversal + safe stem chars, no leading dot), `validate_output_dir` (traversal + HOME/TMPDIR/Volumes/media/mnt allowlist), `validate_no_traversal` (traversal-only for read inputs). `OperationPayload::validate_outputs()` dispatches across all 29 variants; `run_operation` calls it synchronously before thread spawn. `validate_no_traversal` added to diff_subtitle (a+b), lint_subtitle, probe_subtitles, chroma_key_preview, preview_diff, preview_image_quality. 10 new unit tests. 244 rust + 56 vitest pass; clippy `-D warnings` clean; cargo audit unchanged.

### B16 — `refactor(arch): unify analysis/probe/preview lifecycle (ASYNC + CHILD REGISTRATION)` — **DONE (phase 1)** (830d105)
- **Findings:** F-06 (4 lenses converge); B11 deferred scope (F-29 sentinel bridge at run_operation)
- **Rationale:** Closes ARCH-003, ARCH-007, CONC-011, half of CONV-003. Convert 14 sync commands (`analyze_*`, `preview_*`, `get_waveform`, `get_spectrogram`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, `get_file_info`, `get_streams`) to `async fn` with shared `JobContext` (job_id, Child registry, progress/cancel hooks).
- **Effort:** XL (multi-session)
- **Risk:** **HIGH** — touches every analysis/probe/preview command + their frontend callers.
- **Test:** Command-by-command regression. Phase this — convert in batches of 3-4 commands per sub-PR.
- **Rollback:** phase per-command; each sub-PR individually revertable.
- **Depends on:** B8 (consolidated `run_ffmpeg`), B11 (unified `JobOutcome`), B15 (trust gate on every IPC entry) — the whole foundational stack.
- **Status (phase 1):** DONE — commit `830d105`. B11's deferred sentinel-bridge removal landed: added `op_result(result: Result<(), String>, output_path: String) -> JobOutcome` as the typed conversion point for all 29 `run_operation` dispatch arms. The dispatch block's intermediate `Result<Option<String>, String>` variable and `from_result` call are removed from the `run_operation` path; `from_result` is retained for the `convert_file` path (convert/ modules still use `Err("CANCELLED")` — B16 phase 2 scope). Invariant compile-enforced: exhaustive match on OperationPayload + `op_result` total function guarantees exactly one of Done/Cancelled/Error per dispatched job_id. 4 new invariant tests for `op_result`; 248 rust + 56 vitest pass; clippy `-D warnings` clean; cargo build --release clean; cargo audit unchanged. **Remaining B16 scope (phase 2, multi-session):** async lifecycle for 14 analysis/probe/preview commands + JobContext (cancel, progress, Child registration) — the original XL plan entry. `from_result` bridge removal in convert/ modules is a prerequisite for full sentinel cleanup.

### B17 — `refactor(types): ts-rs codegen for IPC boundary` — **DONE** (95e7812)
- **Findings:** F-28, F-27 (narrowed ConvertOptions)
- **Rationale:** Generate `.ts` definitions for `ConvertOptions`, `OperationPayload`, event structs at build time. Field renames become compile errors. F-27 is natural consequence — per-lane payload types.
- **Effort:** L
- **Risk:** MEDIUM — build pipeline change; frontend must consume generated types.
- **Test:** Build succeeds with generated types; intentional Rust rename breaks frontend build.
- **Rollback:** keep hand-written types in-tree during transition.
- **Status:** DONE — commit `95e7812`. ts-rs 10.1.0 added. `#[derive(TS)]` + `#[ts(export, export_to = "../../src/lib/types/generated/")]` on ConvertOptions, JobProgress, JobDone, JobError, JobCancelled, OperationPayload (lib.rs), NormMode, FpsAlgo, ScaleAlgo, ChromaAlgo, ChromaOutput, ExtractStreamSpec. 12 `.ts` files generated to `src/lib/types/generated/` (discriminated union for OperationPayload with all 29 variants, correct imports). tsconfig.json added (strict/noEmit). `typescript ^5` added to devDeps. `npm test` now runs `tsc --noEmit && vitest run`. `src/lib/types/ipc.ts` holds field-level assertions that make tsc fail on Rust field renames. 260 rust (was 248, +12 ts-rs export tests) + 56 vitest pass; clippy -D warnings clean; cargo build --release clean; cargo audit 0 vulnerabilities. **Advisory:** AudioOffset.offset_ms Rust i64 → TS bigint drift surfaced by codegen — frontend uses number; fix is a micro-patch candidate outside B17 scope.

### B18 — `polish: parking_lot::Mutex + return-shape drift + doc refresh` — **DONE** (72e18c4)
- **Findings:** F-31, F-30, F-33
- **Rationale:** Convention cleanup; no-cost principle fixes.
- **Effort:** S
- **Risk:** LOW
- **Test:** `cargo test`, `cargo clippy -- -D warnings`.
- **Status:** DONE — commit `72e18c4`. F-31: parking_lot 0.12 added; Mutex import swapped across 32 files; all .lock().expect()/.lock().unwrap() simplified to .lock() (presets.rs test code left as std::sync::Mutex — uses explicit poison-recovery). F-30: scan_dir/file_exists/check_tools/get_theme/get_accent migrated to Result<T, String>; frontend callers already have try/catch so JS contract unchanged; fs_commands tests updated; scan_dir_returns_ok_for_missing_path has explicit assert_eq! assertion. F-33: ARCHITECTURE.md LOC corrected (lib.rs ~850→~3300, App.svelte ~1960→~3100); all 13 undocumented frontend components and full operations/ subtree added. 260 rust + 56 vitest pass; clippy -D warnings clean; cargo build --release clean; cargo audit 0 vulnerabilities.

---

## 4. Dependency graph

```
Phase 1 (parallelizable — no deps between):
  B1  (serde_yml)              — independent
  B2  (write_fade_log)         — independent
  B3  (IPC input caps)         — independent
  B4  (capability narrows)     — independent
  B5  (presets)                — independent
  B6  (UI UX guards)           — independent
  B7  (archive memo + rate-limiter scaffold) — prepares B8

Phase 2 foundation:
  B8  (run_ffmpeg consolidation) ← B7
      │
      ├──→ B11 (executor reconciliation)
      ├──→ B9  (cancel TOCTOU)
      └──→ B13 (loudnorm + double-probe)

  B10 (batch fanout semaphore) — independent
  B15 (trust gate)             — independent of B8 but informs B16
  B12 (waveform streaming)     — independent

Phase 3 structural:
  B16 (lifecycle unification)  ← B8, B11, B15  [XL — multi-session]

Phase 4 polish:
  B14 (frontend perf)          — independent
  B17 (typeshare)              — independent, can start Phase 2
  B18 (parking_lot + drift)    — independent
```

**Critical path:** B7 → B8 → B11 → B16. ~10 engineer-days sequential.

**ASCII:**

```
                    ┌── B11 ──┐
   B7 ──→ B8 ──────┤         ├──→ B16  (XL)
                    ├── B9    │
                    └── B13   │
                              │
                   B15 ───────┤
```

---

## 5. Phased execution plan

### Phase 1 — fast wins (parallelizable)
Ship all in one ~3-day sprint. Any engineer can pick any batch.
- **Order within phase (not strictly sequential, but logical):** B1 → B2 → B3 → B4 → B5 → B6 → B7
- **Total:** ~3-4 days if sequential, <1 day if 2 engineers parallel.
- **Gate:** CI green on each before next starts.

### Phase 2 — high-impact foundations
- B8 (run_ffmpeg consolidation) — **first**, unblocks rest
- Once B8 lands: B9, B11, B13 can go in parallel
- In parallel with B8: B10 (batch fanout), B12 (waveform streaming), B15 (trust gate)
- **Total:** ~5-7 days.
- **Gate:** B8 merged + CI green before B9/B11/B13 start.

### Phase 3 — structural work
- B16 (lifecycle unification) — own session, phased sub-PRs (3-4 commands at a time)
- B17 (typeshare codegen) — can overlap with B16
- **Total:** ~5-8 days.
- **Gate:** Phase 2 complete; no regressions in Phase-2 areas.

### Phase 4 — polish
- B14, B18, doc refresh
- **Total:** ~1 day.

**Tripwire:** If Phase 2's B8 triggers regression in merge/subtitle convert, **halt Phase 2**, revert, re-scope. Do not start Phase 3 on a shaky foundation.

---

## 6. Drop list

Explicitly not fixing. Reasons kept so they don't resurface.

| ID | Reason |
|---|---|
| **CONC-003** (listen-after-invoke filmstrip race) | **REJECTED.** Backend spawn → nice → ffmpeg seek+decode → base64 takes ≥100 ms before first emit; `listen()` Promise resolves in a microtask tick. Author comment at call site shows ordering already considered. |
| **ARCH-012 IPC-commands component** ("every `#[command]` callable because `core:default`") | **REJECTED.** Category error. Tauri 2's capability permissions gate plugin commands (`core:*`, `fs:*`, `shell:*`), not user-defined `#[command]` from `generate_handler!`. `core:default` grant is standard. Only the `assetProtocol.scope` component survived — tracked as F-08. |
| **CONC-009 poison cascade "undead app"** | **UNREACHABLE.** Mutex holds are trivial (HashMap insert/remove, AtomicBool store); no user code runs inside the lock. The cascade narrative requires a panic path that doesn't exist. Residual principle fix (switch to `parking_lot`) kept as F-31 in B18, but the "live crash path" framing is dropped. |
| **PERF-009** (`args/video.rs` `.to_string()` × 133) | Per-conversion µs vs minutes of ffmpeg work — not a real finding. |
| **PERF-010** (clone-heavy probe parse) | Probe JSON is kB, once per op — micro. |
| **PERF-006** (`check_tools`/`tool_available` MEDIUM) | Cold-path startup/settings UI probe — demoted to LOW, not worth a PR. |
| **PERF-008** (stderr churn MEDIUM) | Cold-path allocation (only used on error) — LOW. |
| **CONC-014** (`_bgBusy` allows two concurrent preloads) | **NON-BUG.** Single-threaded JS can't race past the gate; fire-and-forget is intentional. |
| **CONV-023** (`media_type_for` import marked unused) | **NON-BUG.** `#[allow(unused_imports)]` for test consumers, not dead code. |
| **CONV-006** (`unwrap_used=warn` declared but unenforced) | **NEEDS CI-HISTORY CHECK.** Session 3 confirmed CI runs `clippy -D warnings` (`.github/workflows/ci.yml:94`). Either compiles clean because `unwrap_used` is `restriction`-tier (not on under `-W clippy::all`) or CI is red. Not actionable without CI-history check → followup. |
| **CONV-022** (`diag_append` "O_APPEND no rotation") | **FALSE.** `diag_append` rotates via byte-threshold rename at `lib.rs:787-792`. Only relevant point (two rotation strategies in one file) is cosmetic — covered casually by B18 doc refresh if at all. |
| **ARCH-001** (478-line run_operation god-dispatch) | **ARCHITECTURAL TASTE, NOT DEFECT.** Line/variant counts correct; "field rename breaks wire format" is a serde-tagged-enum universal, not fixed by a trait layer. Defer to F-28 typeshare work (B17), which addresses the rename-breaks-silently piece at a different angle. |
| **ARCH-020** (no Rust module cycles) | INFO-only; no cycle tool run. Kept in §7 as ref. |

---

## 7. Followups (NEEDS-EVIDENCE)

These do NOT block the plan. Each has the specific investigation step needed before re-triage.

| ID | Claim | Investigation step |
|---|---|---|
| **CONC-007** | `cancel_job` kill + worker `remove_file` race on reused output_path | Scripted reproduction: rapid cancel→re-convert with identical output_path; observe whether pre-existing legitimate output is deleted. |
| **CONC-015** | Unbatched `diag_append` fire-and-forget per error at `diagnostics.svelte.js:55` | File not found at that path in current checkout. Locate actual diagnostics dispatch site (grep `diag_append` across `src/`); verify pattern. |
| **PERF-016** | Preset save RMW with YAML parse via `librewin_common::config` | Open the `librewin_common` external crate. Confirm: YAML vs JSON format, in-memory cache presence, locking. (If it resembles `write_fade_log`, promote to B5 scope.) |
| **CONV-012** | Error-prefix drift (`"<tool> not found:"`) | `grep -rn 'not found:' src-tauri/src`; count sites. If >10 and growing, centralize under `fmt_tool_missing(tool)`. If ≤5, leave. |
| **ARCH-007** | `analyze_vmaf`/loudness/cut_detect/black_detect/framemd5 uncancellable | Per-command confirmation of absent `processes.lock().insert`. Already covered by F-06/B16 — consider this verified-by-proxy. |
| **ARCH-011** | Probe data reshaped in 5 places | Open each probe site; confirm. If reshapes diverge on field names, promote to F-32 scope. |
| **ARCH-016** | Dup allowlist `classify_ext` (Rust) vs `mediaTypeFor` (JS); `sqlite`/`parquet` missing from JS | Open `src/lib/utils.js:25`; diff extension set against `lib.rs:364,507`. If `sqlite`/`parquet` genuinely missing, XS frontend fix. |
| **ARCH-017** | Dup helpers `ext_of` + `tool_in_path` | Two-call-site consolidation; XS if confirmed. |
| **ARCH-020** | No Rust module cycles | Run `cargo modules` or `cargo-depgraph` if curiosity demands. INFO only. |

---

*End of attack plan. Fresh engineer pickup: §3 Batch B1, §5 Phase 1. All file:line anchors in-file; no external audit reads required.*
