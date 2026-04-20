# Concern-Based Audit — Session 2 of 3

**Date:** 2026-04-20
**Branch:** main
**Commit:** 746b936 (session-1 baseline: 2253162)
**Repo:** Fade (Tauri 2 + Svelte 5 + Rust media converter, v0.6.2)
**Scope:** `src-tauri/src/**` (~12k Rust) + `src/**` (~11k Svelte/JS)
**Method:** Four concern agents dispatched in parallel, each reading the full codebase through one lens. Session 1 static-analysis findings treated as known baseline — not re-reported.
**Lenses:** Performance · Concurrency/async · Convention/consistency · Architecture/boundary
**Total new findings:** 86 (25 perf + 19 conc + 25 conv + 20 arch, minus cross-lens duplicates — 79 unique).

Session 3 (adversarial) consumes this file without prior context.

---

## Executive summary — top 5 findings across all lenses

1. **`run_ffmpeg` duplication (PERF-007 + CONV-002 + ARCH-002 + CONC-008).** A ~80-line ffmpeg-runner helper lives in *four* places: `operations/mod.rs:129` (canonical), `operations/merge.rs:201` (`run_ffmpeg_merge`), `convert/subtitle.rs:121`, plus bespoke re-rolls in every `convert/*.rs`. The merge.rs file-level comment justifies its duplicate with a reason that no longer applies — cleanup has moved to a `thread::spawn + sleep(2s)` out-of-band (merge.rs:127-131). Any session-1 fix (progress rate-limit, mutex-poison recovery) needs to land in each copy or silently diverges. Multi-agent confirmed on the same file:line.
2. **Two disjoint job lifecycles, one app (ARCH-003 + CONC-011 + CONV-003).** `convert_file` + `run_operation` emit `job-progress`/`job-done`/`job-error`/`job-cancelled` with cancellation via `AppState.processes`. Analysis commands (`analyze_vmaf`, `analyze_loudness`, `analyze_cut_detect`, `analyze_black_detect`, `analyze_framemd5`), probe commands (`get_waveform`, `get_spectrogram`, `diff_subtitle`, `probe_subtitles`), and preview commands (`preview_diff`, `preview_image_quality`, `chroma_key_preview`) are sync-blocking, emit no progress, register no Child, and can run for minutes with no cancel path. The frontend has separate UI for each style. This split is the root cause of ARCH-004, ARCH-007, CONC-011 and most of CONV-003.
3. **Trust boundary is spread across every command — no single gate (ARCH-004 + CONV-021 + session 1 #3).** Every `#[command]` that takes a path is its own validator, and most do zero validation. `validate_suffix`/`validate_separator` (lib.rs:325/340) only cover the suffix segment of `convert_file`; `run_operation`'s 29 variants pass frontend-supplied `output_path` straight into `run_ffmpeg`. The CLAUDE.md-promised `validate_output_name()` still doesn't exist. Capability manifest (`capabilities/default.json`) applies `core:default` with no per-command scoping, `assetProtocol.scope: ["**"]`, and `open_url` accepts arbitrary URL schemes (ARCH-012, ARCH-013).
4. **IPC events are broadcast, re-filtered by listeners in JS (ARCH-005 + ARCH-006 + CONC-004 + PERF-004).** `filmstrip-frame`, `job-progress`, `job-done/error/cancelled` all emit to *all* listeners; every Timeline/QueueManager `listen()` filters by `id` / `job_id` in the handler. With 20 frames per strip × N items the bg-preload and foreground load dispatch every JPEG to both listeners (2× payload, 2× wakeup). Under batch jobs, each ffmpeg progress tick wakes N listeners, each of which `queue.find(...)` scans. Compounds with listen-after-invoke races (CONC-003) and 4000-rect unkeyed waveform renders (PERF-004).
5. **Batch convert has no concurrency limit (CONC-002).** `startConvert` in App.svelte:910 fires `invoke('convert_file')` for every queued item without awaiting — each hands to Rust which `std::thread::spawn`s an ffmpeg. A batch of 100 videos spawns 100 concurrent ffmpeg processes. Each contends on the global `AppState.processes` Mutex on every progress line (CONC-010). Under this fanout the Mutex — not ffmpeg — becomes the bottleneck, and mutex poisoning from any one panicking job cascades to every other (CONC-009).

**Scorecard:** Performance: **C+** · Concurrency: **C** · Convention: **B−** · Architecture: **C+**

---

## Multi-agent confirmed findings (2+ lenses converged on same location)

| File:line | Lenses | Finding |
|---|---|---|
| `operations/merge.rs:201` | PERF-007 + CONV-002 + ARCH-002 | `run_ffmpeg_merge` duplicate |
| `lib.rs:420` / `write_fade_log` | CONC (session-1 #8 extended) + CONV-010 + CONV-022 | RMW race + two log-path conventions + two rotation conventions in one file |
| `lib.rs:547,666,733` | CONC-009 + CONV-013 + session-1 #5 | `.expect("…mutex poisoned")` — consistent but uniformly panics |
| `Cargo.toml:8` / clippy config | CONV-006 + CONV-013 | `unwrap_used=warn` declared but 176 sites compile clean (unenforced) |
| `convert/data.rs:224` | ARCH-008 + CONV-017 | `serde_yml` (RUSTSEC-2025-0067/0068) confirmed reachable |
| `probe/filmstrip.rs:44,84` | PERF-015 + CONC-006 + ARCH-005 | Unaddressed broadcast + no cancellation + bg/fg dedup bug |
| `operations/mod.rs:129` ↔ `merge.rs:201` | PERF-007 + CONV-002 + ARCH-002 + CONC-008 | 4-site duplication |
| `lib.rs:454` + `lib.rs:1025` | CONV-003 + ARCH-003 + ARCH-007 | `convert_file` vs `run_operation` diverged |
| `OperationsPanel.svelte:311` + `convert/archive.rs:14` | PERF-005 + PERF-014 + CONC-018 | Subprocess-proliferation hotspots |
| `QueueManager.svelte:197` + `Timeline.svelte:725,743` | CONC-003 + CONC-004 + ARCH-005 + PERF-015 | Filmstrip event broadcast race |
| `lib.rs:68` (`ConvertOptions`) + `PresetManager.svelte:17` | ARCH-009 + ARCH-010 + CONV-016 | 95-field god-struct + frontend/backend type drift (normalize_loudness) |

These multi-agent hits are the highest-confidence items; session 3 should treat them as settled and focus its adversarial budget elsewhere.

---

## Convention drift map

From the Convention agent. Dimensions where the codebase disagrees with itself:

| Dimension | State |
|---|---|
| **Error handling** | Uniformly `Result<T, String>` for commands (100%). Zero `anyhow`/`thiserror`. Error-prefix strings reinvented per-file (`"ffmpeg not found:"`, `"ffprobe not found:"`, `"failed to run duckdb:"`). String-sentinel control flow: `"CANCELLED"` + `"__DONE__"`. Plain-T return outliers: `scan_dir`, `file_exists`, `check_tools`, `get_theme`/`get_accent`. |
| **Logging** | No framework. `tracing`=0, `log`=0, `println!`=0, `eprintln!`=1 (`convert/data.rs:89`). 41 `window.emit` sites — all `let _ =`. Two file-loggers: `write_fade_log` (HOME-based, 100-line RMW) vs `diag_append` (app_log_dir, O_APPEND, no rotation). Same repo, two incompatible strategies. |
| **Command registration** | 30 commands. Macro style: `#[command]` (older, 12 files) vs `#[tauri::command]` fully-qualified (newer — all `analysis/`, `subtitling/` dirs). Perfect directory-split by file age. Return shape mostly `Result<T, String>` with 5 outliers. |
| **Command naming prefix** | `analyze_*` (5) + `probe_*` + `lint_*` + `diff_*` + `get_*` + `preview_*` — no written rule for which read-only probe uses which prefix. |
| **Operations entrypoint fn** | Most expose `pub fn run(…)` (15 files). `video_filters.rs`/`audio_filters.rs`/`frame_ops.rs` expose multiple `run_<verb>` (10 variants). No rule. |
| **Job executors** | `convert_file` writes `write_fade_log` on done/cancelled/error + handles both `"CANCELLED"` and `"__DONE__"` sentinels. `run_operation` writes zero log entries + only handles `"CANCELLED"`. Same conceptual shape, diverged. |
| **Op handler registration** | 28 mechanical ops → `OperationPayload` enum → `run_operation`. 6 analysis + 3 subtitling + 1 chroma-preview → 10 standalone `#[command]`s. No rule for which. |
| **Path construction** | `format!("{}/{}.{}", ...)` POSIX-only (`build_output_path`, `write_fade_log`, `split.rs:40`, `archive.rs:82`, `chroma_key.rs:253`) vs `PathBuf::join` (`diag_path`). Three sites, two idioms; HOME-based broken on Windows. |
| **Clippy config** | `Cargo.toml:5-10` declares `unwrap_used = "warn"`, `await_holding_lock = "deny"`. 176 unwrap/expect sites compile clean with zero `#[allow(clippy::unwrap_used)]` → CI not enforcing. 19 `#[allow(clippy::too_many_arguments)]` across the ops surface — missing params-struct pattern. |
| **Frontend runes** | Svelte 5 adoption 100%. 348 rune usages, 0 `export let`. Clean. |
| **TypeScript** | 0 `.ts` files, no `tsconfig`, no `typeshare`/`ts-rs`. ~100-field `ConvertOptions` + 4 event structs cross IPC as hand-written. Highest-leverage missing convention. |
| **Module organization** | `args/` + `convert/` (14 lanes) + `operations/` (20 mechanical ops + `analysis/` + `subtitling/`) + `preview/` + `probe/`. Rule unstated. Outliers: `operations/subtitling/` contains lint/diff/probe while `convert/subtitle.rs` contains subtitle conversion; `operations/chroma_key.rs` exposes a `preview` command that belongs under `preview/`. |
| **Cross-lang types** | Hand-written on both sides. `ConvertOptions` field renames break only at runtime. Event name strings duplicated. |
| **Magic numbers** | Some centralized (`ZOOM_STEPS`, `truncate_stderr=2000`, `write_fade_log`=100). Many inline in `merge.rs`: sleep=2s, anullsrc sample rate, CRF=18, bitrate=192k. No const module. |
| **TODO/FIXME** | Zero across 93 source files. Matches CLAUDE.md discipline ("use INVESTIGATION-LOG instead"); worth documenting in CLAUDE.md so contributors know why grep returns nothing. |

---

## IPC surface map

From the Architecture agent. 30 commands registered in `invoke_handler!`.

**Unbounded OUTPUT payloads (frontend receives):**
- `scan_dir` → `Vec<String>` of arbitrary length (session-1 #1)
- `get_spectrogram` → base64 PNG (50-300 KB typical)
- `diag_load` → up to 100× JSON diagnostic entries
- `diff_subtitle` → `Vec<SubDiffLine>` grows with file size (session-1 #9)

**Unbounded INPUT payloads (Rust receives):**
- `run_operation::Merge` → `Vec<String>` input paths, no cap
- `run_operation::Split` → `Vec<f64>` timecodes
- `convert_file::ConvertOptions` → 95 `Option<T>` fields, crosses IPC every conversion

**Streaming events:**
- `filmstrip-frame` — 20 events × 50-200 KB base64 JPEG per call, BROADCAST to all listeners (ARCH-005)
- `job-progress` — per-ffmpeg-line, BROADCAST, no rate-limit (ARCH-006, session-1 #6)
- `job-done/error/cancelled` — one-shot, BROADCAST (correct shape, wrong addressing)

**Should be streamed/batched but isn't:** `get_spectrogram` (could stream low-res first); analysis ops emit zero progress (ARCH-007).

**Capability posture:** `capabilities/default.json` grants `core:default` with no per-command allow/deny. Every command in `invoke_handler!` is callable from any webview origin. `assetProtocol.scope: ["**"]` — unlimited. No path-scope on `scan_dir`, no scheme-scope on `open_url` (ARCH-012, ARCH-013).

---

## Missing invariants (beyond session 1 #3)

- CLAUDE.md `validate_output_name()` — still doesn't exist. Session-1 finding unresolved.
- ARCHITECTURE.md §"Cross-cutting contracts" promises every job emits `job-progress`/`job-done`/`job-error`/`job-cancelled` keyed by `job_id`. True for `convert_file` + `run_operation`. False for all 5 `analyze_*` commands and 4 of 5 probe/preview commands.
- ARCHITECTURE.md says "every `#[command]` fn is re-exported at the crate root". False — `operations::analysis::*`, `operations::subtitling::*`, `operations::chroma_key::chroma_key_preview` registered by full path.
- Tauri capability discipline undocumented in CLAUDE.md. New commands land in `invoke_handler!` with no review step for scope.
- `FadePreset` (Rust) doesn't know about `normalize_loudness` field that `PresetManager.svelte:17` uses.
- `scan_dir` doc-comment says "Falls back to the current working directory if the given path fails to open" — code actually returns `[]`. Doc/code disagree.
- ARCHITECTURE.md says `lib.rs ~850 LOC`, `App.svelte ~1960 LOC`. Actual: 2668 and 3092. Doc is stale; 11 UI components added since last update.

---

## Per-lens findings

### Performance lens

| file:line | sev | pattern | description |
|---|---|---|---|
| `operations/rewrap.rs:24,36` | HIGH | duplicate ffprobe | `run_ffprobe` + `probe_duration` both spawn ffprobe on same file. Repeats in `extract.rs:13,28`, `replace_audio.rs:24,28`, `conform.rs:82,123`. |
| `operations/analysis/audio_norm.rs:41,63,103,139` | HIGH | multi-pass ffmpeg | EBU loudnorm = probe + pass-1 + pass-2 = 3 spawns. Peak mode = 3 spawns. |
| `QueueManager.svelte:108,222` | HIGH | duplicate probe | `get_file_info` invoked on add AND on select — ignores cached `q.info`. |
| `Timeline.svelte:692,1137` | HIGH | DOM amplification | 4000 unkeyed SVG `<rect>` per waveform; backend default is 500, frontend opts to 4000. |
| `convert/archive.rs:14,209,321` | HIGH | subprocess probe | `seven_zip_bin()` spawns `7z i` per call to pick binary name. |
| `lib.rs:411,744` | MEDIUM | subprocess probe | `tool_available` shells `which` per query; `check_tools` runs 5× back-to-back. |
| `operations/mod.rs:129`,`merge.rs:201`,`convert/subtitle.rs:121` | MEDIUM | helper dup | `run_ffmpeg` in 3 copies + per-file re-rolls. |
| `operations/mod.rs:152` | MEDIUM | stderr churn | Line-by-line `Vec<String>` push + join for stderr that's usually discarded. |
| `args/video.rs:1` | MEDIUM | arg String alloc | 133 `.to_string()` calls for fixed flags; pattern repeats in `args/audio.rs`, `args/image.rs`, and every op module. |
| `operations/conform.rs:83`,`merge.rs:55-83` | MEDIUM | clone-heavy probe parse | `json["streams"].as_array().cloned()` + double traversal. |
| `Queue.svelte:10,396` | MEDIUM | O(folders × queue) | `childrenOf()` scanned per folder per render; not memoized. |
| `App.svelte:2681-2683,2773,2836` | MEDIUM | filter chain | 3-4 filter passes over FORMAT_GROUPS in template (not `$derived`). |
| `App.svelte:1798` | MEDIUM | clone-per-render | `diagEntries.slice().reverse()` inline in `{#each}`. |
| `OperationsPanel.svelte:311` | MEDIUM | fanout | `extractMode='all'` loops invoke per stream; N ffprobe+ffmpeg could be one. |
| `QueueManager.svelte:183` | MEDIUM | preload fanout | `_bgPreloadNext` steps through 20 items, each spawning 20 ffmpegs (filmstrip compounded). |
| `presets.rs:44` | MEDIUM | RMW YAML parse | Save reloads+reparses full preset file; no in-memory cache. |
| `operations/mod.rs:163` | MEDIUM | per-line alloc | stdout `for line in reader.lines()` allocates a String per ffmpeg progress line; `parse_out_time_ms` runs on non-progress lines. |
| `operations/subtitling/lint.rs:194,232,241` | MEDIUM | char-count 3× | Per-cue `.chars().count()` repeated. |
| `App.svelte:362,427`, `ChromaKeyPanel.svelte:77` | MEDIUM | no debounce | Preview panels spawn subprocess per slider tick; no trailing-edge wait. |
| `operations/analysis/vmaf.rs:32,72` | LOW | disk roundtrip | VMAF writes JSON to disk then reads back. |
| `Timeline.svelte:684,708` | INFO | cache miss | `$effect` re-invokes `get_waveform`/`get_filmstrip` on draft toggle. |

**Writeups:**

- **PERF-001 (probe duplication):** 15+ sites call `run_ffprobe` then `probe_duration`, each a separate fork+exec+JSON parse of the same file. Merge.rs already avoids this by parsing duration from its one probe — the pattern is viable. A single `run_ffprobe_once` returning both streams and duration would eliminate one subprocess per operation.
- **PERF-002 (loudnorm):** Fix direction is folding `volumedetect` into the apply pass via `-filter_complex`, cutting 2-pass peak to 1 pass. EBU is inherently 2-pass (the JSON measurement must be ingested before apply), but the extra `probe_duration` is removable.
- **PERF-004 (4000 SVG rects):** The backend default is 500 (`probe/waveform.rs:56`). `Timeline.svelte:692` explicitly opts to 4000. Render cost is O(4000) DOM nodes per selection, O(4000) JSON numbers across IPC, O(4000) reconciliation on every zoom/pan. A `<canvas>` with polyline is O(1) DOM. If SVG is required, buckets should equal timeline pixel width (typically 800-1600).
- **PERF-014 (extract fanout):** Multi-stream extract fires N separate `run_operation { type: 'extract' }` calls; each spawns fresh thread + ffprobe + ffmpeg. A single ffmpeg call with repeated `-map 0:X -c copy output_X` does all extractions in one decode pass.
- **PERF-019 (slider debounce):** Preview sliders fire `preview_image_quality` / `preview_diff` / `chroma_key_preview` per adjustment with no trailing-edge debounce. Each spawns 1-2 subprocesses. Dragging a slider queues overlapping invocations; older results can arrive after newer and overwrite the UI (see CONC-012 for the race flavor).

---

### Concurrency/async lens

| file:line | sev | pattern | description |
|---|---|---|---|
| `lib.rs:720` | HIGH | TOCTOU | `cancel_job` reads `processes[job_id]` but `convert_file` registers flag before Child insert. Early cancel silently fails. |
| `App.svelte:910` | HIGH | unbounded fanout | Batch convert loop fires `invoke('convert_file')` per item without awaiting. 100 items = 100 concurrent ffmpegs. |
| `Timeline.svelte:743` | HIGH | listen-after-invoke | `listen('filmstrip-frame', ...)` returned as Promise; `invoke('get_filmstrip')` called before the await resolves. Early frames lost. Same at `Timeline.svelte:725`, `QueueManager.svelte:197`. |
| `QueueManager.svelte:197` | HIGH | duplicate listener | Two global `filmstrip-frame` listeners (fg + bg) both receive every emission, filter by id in JS. |
| `presets.rs:44,52` | HIGH | RMW race | `save_preset`/`delete_preset` read→push→write with no file lock. Double-click silently loses a preset. |
| `probe/filmstrip.rs:44` | MEDIUM | orphan procs | Filmstrip thread runs 20 sequential ffmpegs with no Child registry, no cancel hook. Item removal orphans the in-flight strip. |
| `lib.rs:687` | MEDIUM | kill-wait race | `cancel_job` kills child then returns; worker `remove_file(output)` can race a subsequent same-path convert. |
| `operations/mod.rs:184` | MEDIUM | fragile stderr drain | Stderr thread joined before `child.wait()`; works today, fragile to additions. |
| `lib.rs:547` | MEDIUM | poison cascade | Single global Mutex on `processes`/`cancellations`; one panic poisons both maps, every subsequent `.expect` panics. |
| `operations/mod.rs:147` | MEDIUM | coarse mutex | Global `Mutex<HashMap<String,Child>>` serialized across all concurrent jobs' insert/remove. Bottleneck at fanout. |
| `probe/spectrogram.rs:9`, analysis ops, preview ops, subtitling, file_info | MEDIUM | sync-on-main | 14 sync `#[command]` handlers each block IPC for seconds-minutes. Session-1 #2 generalized — enumerated list. |
| `ChromaKeyPanel.svelte:65` | MEDIUM | stale result overwrite | Debounce doesn't guard against in-flight invoke finishing after newer one. Missing generation-token pattern. |
| `App.svelte:669` | MEDIUM | post-cancel flicker | `job-progress` listener has no status guard — cancelled item can flip back to 'converting' briefly. |
| `QueueManager.svelte:157` | MEDIUM | preload dedup | `_bgBusy` bool can allow two concurrent preloads for adjacent items. |
| `diagnostics.svelte.js:55` | LOW | unbatched diag | Fire-and-forget `invoke('diag_append')` per error; cascade serializes on sync command thread. |
| `App.svelte:636` | LOW | listener leak | `onMount` registers 3 window listeners; `onDestroy` removes only `keydown`. HMR/multi-window latent bug. |
| `operations/chroma_key.rs:221` | LOW | per-job probe | `probe_duration` spawned by every op's `run()`; N concurrent jobs → N ffprobes. |
| `convert/archive.rs:14` | LOW | repeated bin probe | `seven_zip_bin()` not memoized. |
| project-wide | INFO | no async | 0 async fns, 0 tokio::spawn, 0 .await. Tauri pulls tokio transitively but project doesn't use it. |

**Writeups:**

- **CONC-001 (cancel TOCTOU):** `convert_file` calls `state.cancellations.lock().insert(job_id, false)` then spawns a worker thread. The worker eventually calls `Command::spawn()` inside `run_ffmpeg` / per-module `run()` and only then inserts `Child` into `state.processes`. A `cancel_job` arriving in the gap between flag registration and Child insertion finds an empty slot, returns Ok, and ffmpeg completes fully. The flag is only checked after ffmpeg exits — too late. Fix requires atomic flag+placeholder, or a post-spawn flag re-check before first wait.
- **CONC-002 (batch fanout):** `startConvert` loops `items.forEach(item => invoke('convert_file', ...))` without await. No semaphore, no worker pool. Each backend invocation `std::thread::spawn`s a worker. 100 videos = 100 concurrent ffmpegs + 100 contentions on `state.processes` Mutex per progress line.
- **CONC-004 (filmstrip broadcast):** Tauri's `window.emit` broadcasts to every listener on the window. `QueueManager.svelte:197` registers a persistent listener for `-bg` ids; `Timeline.svelte:725` registers one for the active id. Every frame runs both handlers; each deserializes and filters. Combined with listen-after-invoke race (CONC-003), early frames may be silently dropped. Preload + foreground load for same item doubles every frame.
- **CONC-009 (poison cascade):** `processes` and `cancellations` are both `Mutex<HashMap>`. A panic in any thread holding either (e.g. `Emitter::emit` OOM) poisons it. Every subsequent `.expect("…mutex poisoned")` across 9 call sites panics, including every `cancel_job` invocation — app becomes undead with events still streaming and no way to clear them. Fix: `parking_lot::Mutex` (no poison) or `unwrap_or_else(|p| p.into_inner())` everywhere (HashMap state has no invariant broken by partial mutation).
- **CONC-011 (sync-on-main enumeration):** Beyond session-1 #2's general flag, the concrete list: `get_waveform`, `get_spectrogram`, `analyze_loudness`, `analyze_cut_detect`, `analyze_black_detect`, `analyze_vmaf`, `analyze_framemd5`, `chroma_key_preview`, `preview_diff`, `preview_image_quality`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, `get_file_info`, `get_streams`. Priority by latency: analyze_vmaf > get_waveform > analyze_cut_detect > preview_diff > chroma_key_preview.

---

### Convention/consistency lens

| file:line | sev | pattern | description |
|---|---|---|---|
| `lib.rs:687` | HIGH | string sentinels | `"CANCELLED"` + `"__DONE__"` untyped control flow across two diverged executors. |
| `operations/merge.rs:201` | HIGH | duplicate helper | `run_ffmpeg_merge` copy-pasted; file comment's justification no longer valid. |
| `lib.rs:1025` | HIGH | divergent executors | `convert_file` writes log + handles both sentinels; `run_operation` writes zero logs + handles one sentinel. |
| `lib.rs:1620` | HIGH | dual dispatch | Analysis ops registered individually; mechanical ops via OperationPayload enum. No rule. |
| `fs_commands.rs:15`, theme.rs | HIGH | return shape drift | `scan_dir`→Vec, `file_exists`→bool, `check_tools`→Value, `get_theme`→String, everything else→Result<T,String>. |
| `Cargo.toml:8` | HIGH | unenforced lint | `unwrap_used=warn` set; 176 unwrap/expect sites compile clean, zero suppressions. |
| `operations/subtitling/*.rs`, `operations/analysis/*.rs` | MEDIUM | macro drift | `#[tauri::command]` fully-qualified in newer dirs; `#[command]` shorthand in older — clean split by file age. |
| `operations/video_filters.rs:36` | MEDIUM | entrypoint naming | `run` vs `run_<verb>` — most modules one-fn, filter modules multi-fn, no rule. |
| `operations/subtitling/probe.rs:14` | MEDIUM | prefix naming | `analyze_*`/`probe_*`/`lint_*`/`get_*`/`preview_*` for same category (read-only probe). |
| `lib.rs:421` vs `lib.rs:777` | MEDIUM | log path drift | `format!("{}/.config/librewin/fade.log", HOME)` vs `app.path().app_log_dir()`. Two conventions, broken on Windows. |
| `convert/data.rs:89` | MEDIUM | no logging | No tracing/log; 1 `eprintln!`; 41 `window.emit` with `let _ =` discard. |
| `operations/mod.rs:70` | MEDIUM | error-prefix drift | `"<tool> not found:"` reinvented per file; some sites use `e.to_string()`. |
| `lib.rs:547` | MEDIUM | panic-on-poison | 9 `.expect("…mutex poisoned")` uniformly panic; none use `into_inner()`. |
| `operations/*.rs` (19 sites) | MEDIUM | too-many-args | 19 `#[allow(clippy::too_many_arguments)]` — missing params-struct pattern. |
| `src-tauri/src` | MEDIUM | emit errors dropped | 41 `let _ = window.emit(...)` — IPC failures silently swallowed. |
| `svelte.config.js` | MEDIUM | no TS | 0 `.ts` files, no `tsconfig`, no typeshare/ts-rs. ~100-field `ConvertOptions` hand-wired at boundary. |
| `operations/chroma_key.rs:268` | MEDIUM | module-ownership drift | `chroma_key_preview` in `operations/` while `preview_diff`/`preview_image_quality` in `preview/`. `probe_subtitles` under `operations/subtitling/` while `get_filmstrip` under `probe/`. |
| `lib.rs:318`, `write_fade_log` | MEDIUM | path construction | `format!("{}/{}.{}")` vs `PathBuf::join` in same file. Non-portable. |
| project-wide | LOW | zero TODO | Zero TODO/FIXME/XXX across 93 files. Per CLAUDE.md discipline; worth documenting. |
| `lib.rs:325` | LOW | validator scope | `validate_suffix`/`validate_separator` cover suffix only; run_operation bypasses. |
| `lib.rs:420`, `lib.rs:784` | LOW | log rotation drift | `write_fade_log` = 100-line RMW; `diag_append` = O_APPEND no rotation. |
| `lib.rs:1` | LOW | shadow fn | `librewin_common::media::media_type_for` imported + marked unused; local `classify_ext` duplicates behavior. |
| project-wide | INFO | no async | `async fn` = 0; convention is sync-fn + `std::thread::spawn`. Undocumented. |
| `src-tauri/src/convert` | INFO | convert vs operations | `convert/` = per-media-type pipelines, `operations/` = single-purpose filters. Rule unwritten. Subtitle splits the rule. |

**Writeups:**

- **CONV-001 (string sentinels):** `convert_file` matches `Err(msg) if msg == "CANCELLED"` then `Err(msg) if msg == "__DONE__"` (the latter signals archive-extract natural completion that shouldn't be treated as error). `run_operation` only matches `"CANCELLED"`. A third sentinel would silently fall through to `job-error`. Typed enum (`JobOutcome::Cancelled`/`::DoneManual`) removes the stringly-typed branch.
- **CONV-002 (run_ffmpeg_merge):** merge.rs:198's `"// Identical to the shared run_ffmpeg; duplicated so concat-list cleanup can happen after process exits cleanly"` — but cleanup moved to a `thread::spawn + sleep(2s)` pattern that doesn't need a custom runner. Comment is a fossil.
- **CONV-005 (return shape drift):** A frontend caller of `scan_dir` cannot distinguish "empty directory" from "path invalid" because the command returns plain `Vec<String>`. `check_tools` returns `serde_json::Value` while every other tool-status command returns typed structs.
- **CONV-006 (unenforced clippy):** `Cargo.toml:5-10` declares serious lints including `unwrap_used=warn`, `await_holding_lock=deny`, `needless_collect=warn`. Session 1 counted 176 unwrap/expect sites. No `#[allow(clippy::unwrap_used)]` anywhere. Either CI isn't running clippy with these lints, `warn` isn't gated to failure, or both. Declared-vs-enforced gap.
- **CONV-016 (no TS):** `ConvertOptions` (~100 fields) is hand-built in Svelte components. A Rust field rename only breaks at runtime. Typeshare/ts-rs would emit `.ts` definitions at build time. Single highest-leverage missing convention.

---

### Architecture/boundary lens

| file:line | sev | pattern | description |
|---|---|---|---|
| `lib.rs:838` | HIGH | god-dispatch | `OperationPayload` 29-variant enum hand-destructured in `run_operation` (lib.rs:1044-1521). No trait layer; field rename breaks wire format silently. |
| `operations/merge.rs:198` | HIGH | duplicate runner | `run_ffmpeg_merge` duplicates canonical helper; justification obsolete. |
| `lib.rs:1596` | HIGH | 3 lifecycles | convert+run_operation (job-events), analyze_* (sync block return JSON), probe/preview (mixed). No unified contract. |
| `lib.rs:454` | HIGH | no trust gate | Every command is its own validator. `output_path`/`output_dir`/`watermark_path` etc. receive zero validation in run_operation. |
| `probe/filmstrip.rs:84` | HIGH | broadcast event | `filmstrip-frame` broadcast to all listeners; both QueueManager + Timeline receive+filter. |
| `lib.rs:679` | HIGH | broadcast progress | `job-progress` broadcast; JS filter per emission + O(N) queue.find. |
| `operations/analysis/vmaf.rs:18` | MEDIUM | uncancellable | analyze_vmaf (+ loudness, cut_detect, black_detect, framemd5) block for minutes; no Child registered, no cancel UI possible. |
| `convert/data.rs:224` | MEDIUM | reachable CVE | serde_yml RUSTSEC confirmed reachable: user YAML → `serde_yml::from_str`. |
| `PresetManager.svelte:17` | MEDIUM | type drift | Frontend presets include `normalize_loudness`; Rust `FadePreset` doesn't. Field silently dropped on persist. |
| `lib.rs:68` | MEDIUM | god struct | `ConvertOptions` = 95 Option<T> fields, flat. Every op gets all fields. No narrowing. |
| `lib.rs:278` | MEDIUM | probe reshaping | `FileInfo` defined lib.rs but built in probe/; `StreamInfo` defined operations/mod.rs but re-shaped to `SubStream` in subtitling. Same file probed N times for different callers. |
| `capabilities/default.json:6` | MEDIUM | wide capability | `core:default` + no per-command scoping = every registered command callable. `assetProtocol.scope: ["**"]`. |
| `lib.rs:1574` | MEDIUM | arbitrary URL | `open_url(url: String)` — no scheme allowlist. Passes to `open`/`xdg-open`/`cmd /C start`. |
| `operations/split.rs:40` | MEDIUM | path construction | Each op builds output path ad-hoc with `format!`; no shared validator. |
| `src-tauri/src` | MEDIUM | emit errors swallowed | 43 `let _ = window.emit(...)` — window-destroyed + background thread = leaked state. |
| `lib.rs:364`, `src/lib/utils.js:25` | LOW | dup allowlist | `classify_ext` (Rust) vs `mediaTypeFor` (JS) — `sqlite`, `sqlite3`, `db`, `parquet` in Rust absent from JS → silently unqueueable. |
| `operations/mod.rs:267`, `convert/archive.rs:30` | LOW | dup helpers | `ext_of` + `tool_in_path` duplicated. |
| `probe/spectrogram.rs:8` | LOW | shape asymmetry | `get_spectrogram` blocks+returns; `get_filmstrip` streams events. Same conceptual job, two mental models. |
| `ARCHITECTURE.md:8` | LOW | stale doc | LOC numbers off by 3× for lib.rs, 1.5× for App.svelte; 11 components undocumented. |
| project-wide | INFO | clean dependency graph | No Rust module cycles, no frontend import cycles. |

**Writeups:**

- **ARCH-001 (god-dispatch):** `run_operation` is 478 lines of `match payload { OperationPayload::X { a, b, c, ... } => ops::x::run(a, b, c, ...) }`. Adding an op = 3-edit changeset in lib.rs + a new module. A `trait Operation { fn run(self, ctx) -> Result<...>; }` implemented per-op would collapse the match and move ownership of parameter shapes into the op modules themselves.
- **ARCH-004 (no trust gate):** Every command with a path argument (`convert_file`, `file_exists`, `scan_dir`, `diff_subtitle`, `lint_subtitle`, `probe_subtitles`, every `analyze_*`, every `preview_*`, and all 29 run_operation variants via `output_path`/`output_dir`/`watermark_path`/`a_path`/`b_path`) is its own validator. Most do nothing. A single `fn enter_path(p, kind)` with confinement-to-opened-roots removes the per-command cognitive load and makes the trust boundary auditable.
- **ARCH-008 (serde_yml reachable):** Session 1 flagged `serde_yml` (RUSTSEC-2025-0068 unsound + unmaintained) but couldn't confirm runtime reachability. Confirmed: `convert/data.rs:224` parses user-supplied YAML, `:312` serializes. Live surface every time a `.yaml`/`.yml` enters the queue. Drop-in migration to `serde_yaml_ng` available.
- **ARCH-009 (preset type drift):** `PresetManager.svelte:16-22` built-in presets include `normalize_loudness: true`. `save_preset` (presets.rs:16) has no parameter for it. Built-ins work because they never persist through Rust. User saves silently drop the field. Any further UI preset additions have the same bug.
- **ARCH-012 (wide capability):** `capabilities/default.json` has `core:default` and 3 window/process permissions — no allow/deny for the 30 `#[command]` handlers. Any webview content can invoke `scan_dir`/`file_exists`/`open_url`/`diag_append`. `assetProtocol.scope: ["**"]` means any file path served via `asset://` is readable. Dynamic capability updates (per-session runtime scope based on user-opened roots) would confine this.

---

## Cross-cutting patterns

1. **Subprocess proliferation.** Every user action spawns 2-3 subprocesses where one would do: `run_ffprobe` + `probe_duration` (15+ sites), `seven_zip_bin()` + real command (per-call), `check_tools` (5× `which`), loudnorm (3× ffmpeg), extract-all (N× ffmpeg+ffprobe), preview sliders (debounce-free per-tick spawn). Systemic. Convergent fix would be (a) single ffprobe per op, (b) OnceLock-memoized tool resolver, (c) slider debounce at the frontend call-sites.
2. **Broadcast events + JS-side filtering.** All Tauri events (`filmstrip-frame`, `job-progress`, `job-done/error/cancelled`) broadcast to every listener. Frontend filters by `id` / `job_id` in handlers. Under concurrent jobs + multi-listener components, every event wakes every listener. Fix: scope event names (`job-progress-{id}`) and attach one listener per active item.
3. **Duplicated helpers.** `run_ffmpeg` (×4), `run_ffprobe` pattern (×15 sites probe twice), `ext_of` (×2), `tool_available`/`tool_in_path` (×2), `classify_ext`/`mediaTypeFor` (Rust+JS), path-construction via `format!`. No shared utility crate internally; each module grows its own.
4. **No trust gate at the IPC boundary.** Session 1's `validate_output_name()` missing is one symptom. The systemic issue is that every command owns its validation. `validate_suffix` only runs in `convert_file`. `run_operation`'s 29 variants bypass. `scan_dir`/`file_exists`/`diff_subtitle`/`open_url` all admit arbitrary strings. Capability manifest doesn't scope either.
5. **Unenforced discipline.** Cargo.toml declares `unwrap_used=warn` but 176 sites compile clean. CLAUDE.md declares `validate_output_name()` but it doesn't exist. ARCHITECTURE.md declares "every command emits job lifecycle events" but 9 of 30 don't. Doc-vs-code consistency is the meta-pattern behind many findings.
6. **Silent error swallowing.** 43 `let _ = window.emit(...)` + 41 fire-and-forget `invoke('diag_append')` + zero logging framework. When anything fails at the boundaries, evidence disappears. Contributes to the "spinner that never resolves" end-user failure mode.
7. **Hand-wired cross-language contract.** ~100 fields of `ConvertOptions` + 4 event structs + 29 operation payloads built by hand on both sides. No typeshare/ts-rs, no `.ts` files, no `tsconfig`. Type drift already live (ARCH-009).
8. **Two of everything, no rule.** Two job executors (convert_file vs run_operation), two log writers (write_fade_log vs diag_append), two path idioms (format! vs PathBuf::join), two command-macro styles, two preview modules (preview/ vs operations/chroma_key), two sentinels (CANCELLED, __DONE__), two extension allowlists (Rust + JS). Every one is understandable in isolation; together they're the convention-drift signature.

---

## Recommendations for session 3 (adversarial)

Prioritize probing these surfaces:

1. **Broadcast + broadcast-race + no-size-cap compound (ARCH-005, CONC-003, session-1 #4 #9).** Can a crafted frontend sequence that listens to `filmstrip-frame` + `job-progress` while firing large `scan_dir` + `diff_subtitle`(N GB file) queue enough events to starve the webview or OOM? What's the failure mode when the webview lags behind the emitter?
2. **Cancel-race windows (CONC-001, CONC-007).** Can a rapid cancel→re-convert of same output_path corrupt or delete a legitimate output? Can cancel-before-spawn orphan processes that outlive the app session?
3. **serde_yml reachability + unsoundness (ARCH-008).** Craft a malformed YAML to reach the unsound code path in convert/data.rs. Confirm exploitability beyond DoS.
4. **Capability + open_url + assetProtocol compound (ARCH-012, ARCH-013).** With `assetProtocol.scope: ["**"]` and `open_url` accepting any scheme, what cross-origin content-injection path exists during dev? Is the release webview locked down?
5. **Preset RMW + validation gap (CONC-005, ARCH-009, session-1 #3).** Can a crafted preset file on disk cause a panic during `read_presets`? Does `save_preset` reject names that would collide with existing file paths?
6. **Mutex poison cascade (CONC-009).** Can you force a panic in any convert thread (e.g. via an emit during window destruction) to poison the global mutexes and then observe the undead-app state? Verify recovery path (or lack thereof).
7. **ConvertOptions type-drift exploitation (ARCH-010).** Can an absent-but-required field (e.g. frontend forgets `jpeg_quality`) reach an op that unwraps it? Systematic survey of `.unwrap()` on `Option` fields inside ConvertOptions consumers.

---

## Appendix — raw agent outputs

### Performance agent

```json
{
  "agent": "performance",
  "finding_count": 25,
  "severity_distribution": {"high": 5, "medium": 15, "low": 2, "info": 3},
  "top_themes": [
    "Duplicate ffprobe invocations (run_ffprobe + probe_duration) in 15+ sites",
    "run_ffmpeg helper duplicated 3+ times; per-module re-rolls",
    "Frontend re-probes files already probed on queue-add",
    "Timeline emits 4000 unkeyed SVG rects per waveform",
    "Preview sliders fire fresh subprocess per tick with no debounce",
    "Background filmstrip preload fans out 20 ffmpegs × 20 items",
    "Vec<String> arg construction via .to_string() at every Command::new site",
    "seven_zip_bin() + tool_available shell out per call"
  ],
  "summary": "Backend's biggest systemic performance drag is subprocess proliferation: every user action spawns 2-3 ffprobe/ffmpeg processes that could be 1. Frontend's hottest issue is Timeline rendering 4000 unkeyed SVG rects per waveform (backend default was 500 — frontend overrides). Secondary but systemic: queue re-probes on selection, preview sliders have no debounce."
}
```

### Concurrency agent

```json
{
  "agent": "concurrency",
  "finding_count": 19,
  "severity_distribution": {"high": 5, "medium": 9, "low": 4, "info": 1},
  "top_themes": [
    "cancel_job vs convert_file: TOCTOU between flag-register and Child-insert",
    "Batch convert spawns unbounded concurrent ffmpegs (no worker pool)",
    "listen('filmstrip-frame') subscribed AFTER invoke — early frames lost",
    "filmstrip-frame broadcast: QueueManager + Timeline both receive every frame",
    "save_preset RMW race — concurrent saves lose entries",
    "Filmstrip std::thread has no cancellation plumbing — item delete orphans 20 ffmpegs",
    "AppState mutex poison cascade across all unrelated jobs",
    "14 sync commands block IPC for seconds-minutes each",
    "Zero async fns across src-tauri; Tauri tokio runtime unused"
  ],
  "summary": "Concurrency model is coarse-grained but mostly works for single-job case. Under batch fanout (CONC-002) or rapid UI interaction (CONC-003/4/12) the cracks show: lost events, orphaned processes, cascading poison, racy cancellation. Highest-leverage fixes: bound batch concurrency, wire filmstrip cancellation + dedup broadcast listener, migrate probe/analysis commands to async fn, adopt parking_lot::Mutex."
}
```

### Convention agent

```json
{
  "agent": "convention",
  "finding_count": 25,
  "severity_distribution": {"high": 6, "medium": 14, "low": 3, "info": 2},
  "top_themes": [
    "String sentinels (CANCELLED, __DONE__) for control flow — two executors handle differently",
    "run_ffmpeg duplicated as run_ffmpeg_merge with obsolete justification comment",
    "convert_file vs run_operation diverged on logging + sentinel handling",
    "Return shapes inconsistent: Result<T,String> vs plain T vs serde_json::Value",
    "Cargo.toml unwrap_used=warn declared but 176 sites compile clean (unenforced)",
    "#[command] vs #[tauri::command] splits by directory (newer dirs use qualified)",
    "Two log-path conventions in same file (HOME format! vs app.path().app_log_dir())",
    "No logging framework — tracing=0, log=0, 41 window.emit sites all discard errors",
    "No TypeScript — 0 .ts files, no tsconfig, ~100-field ConvertOptions hand-wired"
  ],
  "summary": "Conventions are consistent on deliberately-decided dimensions (Svelte 5 runes=100%, Result<T,String>=near-100%, no-unsafe, no-TODO, sync #[command]+thread::spawn) but drift on undecided dimensions (module ownership convert/ vs operations/ vs preview/, run-fn naming, command-macro style by file age, job-executor divergence, log-path idioms). Most damaging: run_ffmpeg duplication with obsolete justification, two string sentinels on diverged executors, unenforced clippy lints, missing TypeScript layer."
}
```

### Architecture agent

```json
{
  "agent": "architecture",
  "finding_count": 20,
  "severity_distribution": {"high": 6, "medium": 10, "low": 3, "info": 1},
  "top_themes": [
    "Three disjoint job lifecycles (convert/run_operation vs analyze_* vs probe/preview)",
    "run_operation = 478-line god-dispatch over 29-variant enum; no trait layer",
    "run_ffmpeg duplicated as run_ffmpeg_merge with stale justification",
    "No single trust gate — every command validates (or doesn't) its own paths",
    "Capability manifest wide open: core:default + no per-command scope + assetProtocol=['**']",
    "open_url accepts arbitrary URL schemes",
    "filmstrip-frame + job-progress broadcast to all listeners, JS re-filters",
    "serde_yml (RUSTSEC-unsound) reachable via convert/data.rs user YAML input",
    "ConvertOptions god-struct = 95 Option<T> fields flat",
    "Preset type drift: frontend normalize_loudness absent in Rust FadePreset",
    "Probe data reshaped in 5 places, no shared cache",
    "43 let _ = window.emit sites swallow IPC failures",
    "ARCHITECTURE.md stale (LOC 3× off, 11 components undocumented)"
  ],
  "ipc_surface_map": "30 commands. Unbounded outputs: scan_dir, get_spectrogram, diag_load, diff_subtitle. Unbounded inputs: run_operation::Merge/Split, convert_file::ConvertOptions. Streaming broadcast events: filmstrip-frame, job-progress. Capability manifest grants every command to every origin with no scope.",
  "summary": "Fade is structurally three subsystems (convert, operations, analysis/probe/preview) wearing one coat — each with own lifecycle, cancellation, trust-boundary, error propagation. Root cause of most medium/high findings. Type drift already live. Capability manifest wide open. No module cycles structurally; the issues are contracts + duplication, not entanglement."
}
```

---

*End of session 2. Session 3 (adversarial) is next — it should consume this file + audits/01-static-analysis.md as its full context.*
