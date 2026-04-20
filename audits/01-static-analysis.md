# Static Analysis — Session 1 of 3

**Date:** 2026-04-20
**Branch:** main
**Commit:** 2253162
**Repo:** Fade (Tauri 2 + Svelte 5 + Rust media converter, v0.6.2)
**Scope scanned:** `src-tauri/src/**` (Rust, ~12k LOC, 62 files) + `src/**` (Svelte/TS, ~11k LOC, 31 files)
**Excluded:** `node_modules`, `target`, `dist`, `gen`, `src-tauri/target`
**Tools run:** Semgrep 1.157.0 (p/rust, p/security-audit, p/owasp-top-ten, p/secrets, p/typescript — 134 rules on 88 files), cargo-audit 0.x, ripgrep pattern scans
**Tools unavailable:** CodeQL (not installed)

This is session 1 of a 3-pass audit. Sessions 2 (concern-based) and 3 (adversarial) will consume this file as input.

---

## Summary — counts by severity

| Severity | Count | Notes |
|---|---|---|
| CRITICAL | 0 | No RCE / auth bypass / hardcoded secret / memory corruption found. |
| HIGH | 4 | Path-traversal surface in fs_commands; all `#[tauri::command]` handlers synchronous; unbounded frontend inputs reaching subprocess spawns; stale CLAUDE.md invariant (`validate_output_name` does not exist). |
| MEDIUM | 9 | `.expect()` in mutex paths; progress-event cadence uncapped; repeated subprocess spawns where batching possible; unbounded stdout decode into Vec<u8>; world-readable `/tmp` used for non-secret artifacts; `write_fade_log` race; sync disk IO inside command thread; large base64 payloads via IPC events; dependency advisories (unsound/unmaintained transitive crates). |
| LOW / INFO | 22 | 15 Semgrep `temp-dir` INFO hits, 7 other stylistic findings. |
| CLEAN | — | ffmpeg arg construction via `Vec<String>` (no shell); codec selection is allowlist; hex/numeric chroma inputs are typed + clamped; no hardcoded secrets found; frontend has no `innerHTML`/`eval`/`new Function` outside tests. |

---

## Findings table

| file:line | severity | rule / category | one-line description |
|---|---|---|---|
| src-tauri/src/fs_commands.rs:7 | HIGH | path-traversal / missing-allowlist | `file_exists(path)` accepts any path from frontend — no confinement; probes filesystem. |
| src-tauri/src/fs_commands.rs:15 | HIGH | path-traversal / unbounded-output | `scan_dir(path, recursive)` walks arbitrary dirs recursively; no depth cap, no allowlist, no file-count cap. Can enumerate `/` or `$HOME`. |
| src-tauri/src/lib.rs:454 | HIGH | async-discipline | `convert_file` (and every `#[tauri::command]` in the project) is synchronous `fn`, not `async fn`. Tauri default dispatches sync commands on the main thread, blocking IPC. |
| src-tauri/CLAUDE.md + src-tauri/src/lib.rs:325 | HIGH | doc/code drift | CLAUDE.md QC rule mandates `validate_output_name()` before any CLI arg interpolation. That function does not exist — only `validate_suffix` + `validate_separator`. Invariant is unenforced; new commits can silently violate it. |
| src-tauri/src/lib.rs:547, 666, 733 | MEDIUM | unwrap/expect in hot path | `.expect("cancellations mutex poisoned")` / `"processes mutex poisoned"` — panics propagate across threads; mutex poisoning is reachable if any held-lock section panics. |
| src-tauri/src/operations/mod.rs:172 | MEDIUM | event-spam / IPC cadence | `run_ffmpeg` emits `job-progress` for every `out_time_ms` line from ffmpeg stdout (~2 Hz, higher with some builds). No rate-limit, no change-threshold gate. |
| src-tauri/src/probe/filmstrip.rs:47 | MEDIUM | repeated-subprocess | `get_filmstrip` spawns N separate `ffmpeg` processes in a loop (one per frame). `count` is unvalidated from frontend. Could batch with a single `select` filter call. |
| src-tauri/src/probe/filmstrip.rs:84 | MEDIUM | oversized IPC payload | Emits `filmstrip-frame` events containing full base64-encoded JPEG per frame. 20 events per call; payload size uncapped (decided by user-supplied `scale` + `q:v`). |
| src-tauri/src/probe/waveform.rs:38 | MEDIUM | unbounded-allocation / sync-in-command | `get_waveform` calls `Command::output()` collecting entire decoded PCM stream into `Vec<u8>` before bucketing. 1-hour file @ 8 kHz ≈ 115 MB. Also sync command — blocks IPC. |
| src-tauri/src/lib.rs:420 | MEDIUM | race / sync IO in command path | `write_fade_log` reads existing file, appends, rewrites — no file lock. Concurrent job completions race; under contention last-writer-wins can drop entries. |
| src-tauri/src/preview/image_quality.rs:17 | MEDIUM | sync-in-command | Sync `#[command]` spawning two `magick` subprocesses back-to-back on the command thread. Large images block IPC. |
| src-tauri/src/operations/subtitling/diff.rs:33 | MEDIUM | unbounded-read | `fs::read_to_string(a_path)` / `(b_path)` with no size cap. Frontend-supplied paths — a multi-GB file freezes the command thread and exhausts RAM. |
| src-tauri/Cargo.lock (serde_yml 0.0.12) | MEDIUM | dep-advisory | RUSTSEC-2025-0068 — `serde_yml` is unsound + unmaintained. Direct dependency. |
| src-tauri/Cargo.lock (libyml 0.x) | MEDIUM | dep-advisory | RUSTSEC-2025-0067 — `libyml::string::yaml_string_extend` unsound. Pulled transitively via serde_yml. |
| src-tauri/Cargo.lock (glib 0.x) | LOW | dep-advisory | RUSTSEC-2024-0429 — unsound `VariantStrIter` iter impl (Linux/GTK only; macOS build unaffected). |
| src-tauri/Cargo.lock (rand) | LOW | dep-advisory | RUSTSEC-2026-0097 — unsound with custom logger + `rand::rng()`. |
| src-tauri/Cargo.lock (gtk/atk/gdk/gdkx11 family) | LOW | dep-advisory | RUSTSEC-2024-041x — GTK3 bindings unmaintained (Linux-only impact). |
| src-tauri/Cargo.lock (fxhash, proc-macro-error, unic-*) | INFO | dep-advisory | Unmaintained crates pulled transitively; no known vulns. |
| src-tauri/src/convert/subtitle.rs:244,259 | INFO | semgrep temp-dir | `std::env::temp_dir()` used for scratch output — acceptable (non-secret artifacts) but flagged by p/rust. |
| src-tauri/src/fs_commands.rs:55,83,127 | INFO | semgrep temp-dir | Same — inside `#[cfg(test)]` blocks. |
| src-tauri/src/operations/analysis/vmaf.rs:32 | INFO | semgrep temp-dir | VMAF log file in temp_dir — filename has uuid, low risk. |
| src-tauri/src/operations/chroma_key.rs:294 | INFO | semgrep temp-dir | Preview PNG in temp_dir — filename has uuid. |
| src-tauri/src/operations/mod.rs:213 | INFO | semgrep temp-dir | `write_temp_concat_list` — uuid-namespaced; content is user-controlled paths wrapped in `'...'` with backslash escaping (safe for ffmpeg concat demuxer). |
| src-tauri/src/preview/image_quality.rs:35,102,120,167 | INFO | semgrep temp-dir | Temp files for encoded/diff images — uuid-namespaced. |
| src-tauri/src/preview/video_diff.rs:44,162 | INFO | semgrep temp-dir | Temp video artifacts — uuid-namespaced. |
| src-tauri/src/convert/tracker.rs:182 | INFO | semgrep temp-dir | Tracker temp files. |
| src-tauri/src/lib.rs:307-311 | LOW | `.unwrap_or_default()` on Path stem | `Path::file_stem().unwrap_or_default()` in `build_output_path` — OK semantically but silent empty stem could create `./.ext`. |
| src-tauri/src/lib.rs:421 | LOW | shell-env path join | `format!("{}/.config/librewin/fade.log", home)` — manual join; Path-style preferred. |
| src-tauri/src/lib.rs:411-417 | LOW | platform-portability | `tool_available` shells out to `which` — Windows has `where`. Current project ships Linux/macOS so acceptable. |
| src-tauri/src/operations/split.rs:40 | LOW | format-string path | `format!("{}/{}_part%03d.{}")` — output_dir + stem both from caller; `%03d` is ffmpeg pattern but no validation of `stem`. |
| src-tauri/src/presets.rs:33 | LOW | preset input validation | `codec`, `output_format`, `media_type` strings stored into persisted presets without validation. Safe today because downstream uses allowlists, but the persisted file becomes a trust boundary. |

Total Semgrep findings: 15 (all INFO-level `temp-dir` — see raw output at bottom).

---

## Top 10 — detailed

### 1. HIGH — `scan_dir` has no path confinement and no result caps

**File:** `src-tauri/src/fs_commands.rs:15-47`

```rust
#[command]
pub fn scan_dir(path: String, recursive: Option<bool>) -> Vec<String> {
    let recurse = recursive.unwrap_or(false);
    let mut files: Vec<String> = Vec::new();
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() { return files; }
    let mut stack: Vec<std::path::PathBuf> = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue; };
        for entry in entries.flatten() {
            // ... pushes every file path it finds ...
            if recurse && p.is_dir() { stack.push(p); }
        }
    }
    files.sort();
    files
}
```

**Why it matters:** Frontend-callable command that accepts arbitrary path, recurses with no depth or file-count cap, and returns every path it finds as a `Vec<String>`. Pointing it at `/` or `$HOME` returns a multi-hundred-megabyte JSON payload across the Tauri IPC boundary and blocks the command thread for the duration. Tauri's capability allowlist is the only gate; if the dev capability is permissive, this doubles as a filesystem enumeration oracle. This is also a DoS vector: one call can pin the command thread for minutes.

**Fix direction:** Confine to an allowlisted set of roots (the user-chosen project dir), enforce a `max_depth` and `max_entries` ceiling, and make the command async so it doesn't monopolize the IPC thread. Consider streaming results as events instead of returning one giant Vec.

---

### 2. HIGH — All `#[tauri::command]` handlers are synchronous `fn`

**File:** project-wide — 16 command-bearing files. `grep -rn "async fn" src-tauri/src` returns 0 hits.

```rust
#[command]
pub fn get_waveform(path: String, ...) -> Result<WaveformData, String> {
    let output = Command::new("ffmpeg").args([...]).output()
        .map_err(|e| format!("ffmpeg not found: {e}"))?;
    // ... seconds of work on the command thread ...
}
```

**Why it matters:** Tauri dispatches sync `#[command]` invocations on a single main thread by default. Every long-running command (ffmpeg probe, magick encode, ffprobe, filesystem walk) monopolizes that thread for its full duration, blocking all other IPC — including `cancel_job`. In practice this explains why cancels feel laggy. The project works around it by `std::thread::spawn`-ing long jobs from `convert_file`, but the command itself still returns from the main thread; probe/preview commands don't even do that.

**Fix direction:** Convert probe / preview / analysis commands to `async fn` so Tauri uses the async runtime and spawns them off the main thread. For CPU-bound work, use `tokio::task::spawn_blocking` or an explicit worker pool. Separate concern from finding #6 below (progress cadence).

---

### 3. HIGH — `validate_output_name()` referenced by project invariant does not exist

**File:** `CLAUDE.md` ("Known Patterns & Gotchas") vs. `src-tauri/src/lib.rs`.

CLAUDE.md states:
> `validate_output_name()` before any CLI arg interpolation. Any Tauri command that interpolates a user-supplied name into a CLI arg string must call `validate_output_name()` first.

Reality:
```
$ grep -rn "fn validate_" src-tauri/src
src-tauri/src/lib.rs:325:  fn validate_suffix(suffix: &str)
src-tauri/src/lib.rs:340:  fn validate_separator(sep: &str)
```

**Why it matters:** A documented, load-bearing safety invariant points to a function that doesn't exist. Either the function was renamed (→ `validate_suffix` covers only the *suffix* segment of the output path, not the full name) and the doc was never updated, or the invariant was never implemented. Either way it's silently unenforced — any new Tauri command that interpolates a user string into a subprocess arg has no checklist hit. Related: `output_path` itself is built from `output_dir` + stem (passed through `Path::file_stem` on the input) + validated suffix + validated separator. So the full *name* is partially validated, but frontends passing `output_dir` directly are not.

**Fix direction:** Reconcile — either add `validate_output_name()` as an umbrella validator and call it from every command that interpolates, or rewrite the CLAUDE.md rule to match what `validate_suffix` + `validate_separator` actually guarantee and document the `output_dir` assumption. Session 2 should treat every frontend-supplied `output_path` as an un-validated trust boundary until this is resolved.

---

### 4. HIGH — Frontend-supplied `count` drives unbounded ffmpeg subprocess spawn

**File:** `src-tauri/src/probe/filmstrip.rs:25-94`

```rust
#[command]
pub fn get_filmstrip(window: Window, path: String, id: String,
                     count: usize, duration: f64, draft: bool) -> Result<(), String> {
    // no upper bound on `count`
    std::thread::spawn(move || {
        for i in 0..count {
            let output = Command::new("nice").args(["-n","19","ffmpeg",...]).output();
            // emits base64 JPEG per frame
            let _ = window.emit("filmstrip-frame", ...);
        }
    });
    Ok(())
}
```

**Why it matters:** `count` comes from the frontend with no ceiling. A bug or misuse can spawn thousands of ffmpeg processes sequentially, each doing a seek+decode, and emit thousands of base64-encoded JPEG payloads through the IPC channel. Caller-driven fan-out of subprocess work is the kind of thing session 2's perf lens will want to fix even without a malicious frontend.

**Fix direction:** Cap `count` server-side (e.g. `count.min(128)`). Better: replace the per-frame loop with a single ffmpeg call using the `select` filter and `image2pipe` to emit N evenly-spaced JPEGs, decoding the file once instead of seeking N times.

---

### 5. MEDIUM — `.expect()` on mutex poisoning in command handlers

**File:** `src-tauri/src/lib.rs:547, 666, 733`

```rust
let mut map = state.cancellations.lock().expect("cancellations mutex poisoned");
```

```rust
let mut map = state.processes.lock().expect("processes mutex poisoned");
```

**Why it matters:** `Mutex::lock()` returns `Err` when a prior holder panicked while holding the lock. That's reachable — any panic inside a command path that holds `processes` or `cancellations` (for instance an `.unwrap()` on `Child::kill()` that went wrong) poisons the mutex. Subsequent commands then `.expect()` on it and propagate the panic upward; in a long-running desktop app this kills the Tauri runtime and drops all in-flight jobs. High unwrap/expect density across command files (69 `.unwrap()` + 107 `.expect()/tauri::command/...` mixed hits) means the first panic cascades.

**Fix direction:** Recover from poison: `.lock().unwrap_or_else(|p| p.into_inner())` — state is just a HashMap of cancellation flags and child processes; none of it has invariants that break if a writer panicked mid-mutation. Alternatively route through `parking_lot::Mutex` which doesn't poison.

---

### 6. MEDIUM — `run_ffmpeg` emits an IPC event per ffmpeg progress line

**File:** `src-tauri/src/operations/mod.rs:163-182`

```rust
if let Some(stdout) = stdout {
    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(elapsed) = parse_out_time_ms(&line) {
            let percent = ((elapsed / dur) * 100.0).min(99.0) as f32;
            let _ = window.emit("job-progress", JobProgress { job_id, percent, message: ... });
        }
    }
}
```

**Why it matters:** ffmpeg's `-progress pipe:1` emits an `out_time_ms=...` line roughly every 500 ms at default cadence, but some builds/filters push it faster. There's no rate-limit and no percent-change threshold — a long video with many progress updates means hundreds of IPC events, each serialized through Tauri's message passing. The frontend has to rerender on each. Event spam compounds when multiple jobs run concurrently.

**Fix direction:** Gate emissions on either a minimum interval (e.g. `Instant::now().duration_since(last) > 100ms`) or a minimum percent delta (e.g. `(percent - last_percent).abs() > 0.5`). Also: the `message: format!("{:.0}s elapsed", elapsed)` allocation happens per line regardless — cheap but wasteful.

---

### 7. MEDIUM — `get_waveform` collects entire decoded PCM into a `Vec<u8>`

**File:** `src-tauri/src/probe/waveform.rs:38-54`

```rust
let output = Command::new("ffmpeg")
    .args(["-i", &path, "-ac", "1", "-ar", ar, "-f", "f32le", "-"])
    .output()
    .map_err(|e| format!("ffmpeg not found: {e}"))?;
// output.stdout is the entire decoded f32le stream
let samples: Vec<f32> = output.stdout
    .chunks_exact(4)
    .filter_map(|c| c.try_into().ok().map(f32::from_le_bytes))
    .collect();
```

**Why it matters:** `.output()` blocks until ffmpeg exits, collecting the full decoded stream. A 1-hour file at 8000 Hz mono f32le = ~115 MB RAM. At 2000 Hz draft (for heavy files) it's still ~29 MB — and a 10-hour podcast crosses a gig. Also, `samples: Vec<f32>` doubles the memory briefly while decoding. Combined with finding #2, the command thread is blocked for the entire decode duration.

**Fix direction:** Pipe ffmpeg through `Stdio::piped()` and do streaming RMS/ZCR accumulation with a small ring buffer. You only need `n` buckets of running RMS — no need to retain every sample. Also bounds-check decoded duration up front (via `-t` or ffprobe) to cap worst-case.

---

### 8. MEDIUM — `write_fade_log` read-modify-write without locking

**File:** `src-tauri/src/lib.rs:420-431`

```rust
fn write_fade_log(entry: &str) {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let log_path = format!("{}/.config/librewin/fade.log", home);
    let existing = std::fs::read_to_string(&log_path).unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    lines.push(entry.to_string());
    if lines.len() > 100 { let start = lines.len() - 100; lines.drain(0..start); }
    let _ = std::fs::write(&log_path, lines.join("\n") + "\n");
}
```

**Why it matters:** Called from the convert-thread finalizer (`done`/`cancelled`/`error` paths at lib.rs:673-703). Multiple concurrent jobs finish around the same time in production (batch queue) — each does its own read-modify-write, and the whole-file rewrite means the last writer silently drops the others' entries. On crash mid-write the file can be truncated to zero. Also: sync disk IO on a thread that just finished a job is fine, but `write_fade_log` is also called for every `error` in the progress-event path.

**Fix direction:** Switch to O_APPEND + periodic rotation (at file open, check size; if > N, rotate). Write via `OpenOptions::new().append(true).open(...)`. One syscall, atomic at the 4 KB pipe-buffer level. Same pattern already exists correctly in `diag_append` at `lib.rs:784` — `write_fade_log` just needs to adopt it.

---

### 9. MEDIUM — `diff_subtitle` and siblings read entire user file into memory with no size cap

**File:** `src-tauri/src/operations/subtitling/diff.rs:33`

```rust
#[tauri::command]
pub fn diff_subtitle(a_path: String, b_path: String) -> Result<Vec<SubDiffLine>, String> {
    let a_body = std::fs::read_to_string(&a_path).map_err(|e| format!("read a: {e}"))?;
    let b_body = std::fs::read_to_string(&b_path).map_err(|e| format!("read b: {e}"))?;
    // ...
}
```

**Why it matters:** Subtitle files are *usually* small, but the path comes from the frontend — no assertion that the file is a subtitle, no size cap. A pointed-at multi-GB log file reads into RAM and blocks the sync command thread. Same pattern likely exists in other subtitling helpers (see the 37 `std::fs::read/write/…` hits). Worth a session-2 sweep.

**Fix direction:** Check file size before read; cap at a sane bound (e.g. 32 MB — bigger than any real subtitle). Or `File::metadata()` then `File::take(cap).read_to_string(&mut buf)`.

---

### 10. MEDIUM — `serde_yml` + `libyml` pulled in, both flagged unsound

**File:** `src-tauri/Cargo.lock` (direct dep is `serde_yml 0.0.12` in `src-tauri/Cargo.toml`).

```
Crate:   serde_yml    Title: serde_yml crate is unsound and unmaintained   ID: RUSTSEC-2025-0068
Crate:   libyml       Title: `libyml::string::yaml_string_extend` unsound  ID: RUSTSEC-2025-0067
```

**Why it matters:** `serde_yml` is a fork of `serde_yaml` whose maintainer abandoned it; it's now flagged unsound. If this crate parses any user-supplied YAML (presets? config files read from disk?), the unsoundness is reachable.

**Fix direction:** Migrate to `serde_yaml_ng` (actively maintained fork) or drop YAML entirely in favor of TOML (which the project already uses — `Cargo.toml`, `tauri.conf.json`). Session 2 should trace what YAML is actually parsed at runtime.

---

## Clean / checked

- **ffmpeg / magick / ffprobe arg construction:** Every subprocess spawn uses `Command::new(bin).args([...])` with `Vec<String>` arg vectors. No `sh -c`, no shell interpolation of user strings. Command injection via arg separation is structurally impossible. Codec selection (`ffmpeg_video_codec_args`, `args/video.rs:180`) is an exhaustive allowlist with `copy` fallback for unknown codecs. Chroma-key filter graph (`operations/chroma_key.rs:89-142`) uses typed numeric values with explicit `.clamp(...)` and hex-parsed `(u8,u8,u8)` interpolated with `{:02X}` — no raw user string lands in the filter chain. HSL hue / sat / val formatted via `{:.2}` / `{:.4}` — fixed numeric formatter.
- **Concat demuxer file paths:** `operations/mod.rs:212-224` writes a temp concat-list file. Single-quote escaping via `p.replace('\'', "'\\''")` is correct for ffmpeg's concat demuxer quoting.
- **No hardcoded secrets:** grep-based scan across `*.rs`, `*.ts`, `*.js`, `*.svelte`, `*.toml`, `*.yaml` for `password=`, `api_key=`, `secret=`, `token=`, AKIA-style keys, `sk-...` tokens — zero production hits.
- **Frontend XSS surface:** `grep` for `innerHTML`, `eval(`, `new Function` across `src/**` — zero hits outside Tauri `invoke(...)` calls in tests. Svelte 5 runes auto-escape.
- **No `unsafe` blocks in project code:** `grep -rn "unsafe" src-tauri/src` — zero matches.
- **Dependency CVEs (only):** cargo-audit reports 22 advisories, *all* of them "unmaintained" or "unsound" warnings — no `cvss:`-rated vulnerabilities.

## Not scanned / limitations

- **CodeQL:** not installed on this machine. Semgrep p/rust coverage is thinner than CodeQL for dataflow into subprocess args; session 2 could install CodeQL for a second pass on the ffmpeg arg plumbing.
- **Capability manifest:** `src-tauri/capabilities/*.json` / `tauri.conf.json` were not analyzed for which commands/paths the frontend is allowed to invoke. High-value for sessions 2/3 — the actual blast radius of finding #1 depends on the capability allowlist.
- **`src/lib/stores/zoom.svelte.js`:** modified file per git status; not re-analyzed against the committed version.
- **Windows build:** `tool_available` shells out to `which`; Windows-specific concerns not scanned (project currently ships macOS/Linux).

---

## Appendix A — Raw Semgrep output

All 15 findings, JSON-derived (`severity | check_id | path:line`):

```
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/convert/subtitle.rs:244
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/convert/subtitle.rs:259
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/convert/tracker.rs:182
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/fs_commands.rs:55
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/fs_commands.rs:83
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/fs_commands.rs:127
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/operations/analysis/vmaf.rs:32
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/operations/chroma_key.rs:294
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/operations/mod.rs:213
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/image_quality.rs:35
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/image_quality.rs:102
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/image_quality.rs:120
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/image_quality.rs:167
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/video_diff.rs:44
INFO | rust.lang.security.temp-dir.temp-dir | src-tauri/src/preview/video_diff.rs:162
```

**Semgrep run summary:**

```
Rules run: 134 (p/rust, p/security-audit, p/owasp-top-ten, p/secrets, p/typescript)
Targets scanned: 88 (62 Rust, 11 Svelte (treated as JS), 7 JS, 8 misc)
Parsed: ~100%
Findings: 15 (all INFO)
Blocking: 15 (severity threshold default)
```

All 15 are the same check (`temp-dir`) flagging uses of `std::env::temp_dir()`. Each instance uses `uuid::Uuid::new_v4()` in the filename; contents are non-secret scratch artifacts (intermediate encodes, filter logs, concat lists). Semgrep's generic advice does not apply — triaged to INFO.

## Appendix B — cargo-audit output (summary)

22 advisories: 0 CVE-rated vulnerabilities, 18 "unmaintained" warnings, 4 "unsound" warnings. Full IDs:

```
unsound:
  RUSTSEC-2025-0067  libyml
  RUSTSEC-2025-0068  serde_yml
  RUSTSEC-2024-0429  glib            (Linux-only)
  RUSTSEC-2026-0097  rand            (custom-logger edge case)

unmaintained:
  RUSTSEC-2024-0411  gdkwayland-sys
  RUSTSEC-2024-0412  gdk
  RUSTSEC-2024-0413  atk
  RUSTSEC-2024-0414  gdkx11-sys
  RUSTSEC-2024-0415  gtk
  RUSTSEC-2024-0416  atk-sys
  RUSTSEC-2024-0417  gdkx11
  RUSTSEC-2024-0418  gdk-sys
  RUSTSEC-2024-0419  gtk3-macros
  RUSTSEC-2024-0420  gtk-sys
  RUSTSEC-2024-0370  proc-macro-error
  RUSTSEC-2025-0057  fxhash
  RUSTSEC-2025-0075  unic-char-range
  RUSTSEC-2025-0080  unic-common
  RUSTSEC-2025-0081  unic-char-property
  RUSTSEC-2025-0098  unic-ucd-version
  RUSTSEC-2025-0100  unic-ucd-ident
```

All gtk/gdk/atk hits are Linux-GUI-stack transitive; macOS builds don't link them. `fxhash` / `proc-macro-error` / `unic-*` are build-time or low-risk transitive. `serde_yml` + `libyml` are the live ones (see finding #10).

## Appendix C — Pattern scan counts (for session 2 reference)

```
Tauri commands (#[tauri::command]):      16 files
async fn in src-tauri/src:                0     (!! finding #2)
.unwrap() in src-tauri/src:              69  across 18 files
.expect() / mixed hits:                 107  across 29 files (includes command decls)
std::fs::{read,write,create,remove,copy,rename} calls: 37
Command::new / .arg / .spawn / .output  82 occurrences across 24 files
window.emit(...) call sites:             41 across 18 files
unsafe blocks:                            0
```

High unwrap/expect density per file to prioritize for session 2:

```
src-tauri/src/fs_commands.rs     14 unwrap (all in #[cfg(test)] — OK)
src-tauri/src/lib.rs             10 unwrap + 36 expect/command hits
src-tauri/src/convert/archive.rs 10 unwrap
src-tauri/src/operations/chroma_key.rs  7 unwrap
```

---

## Recommendations for sessions 2 and 3

- **Session 2 (concern-based) should verify:** the Tauri capability manifest allowlist (limits blast radius of finding #1); whether `serde_yml` is actually reached at runtime (determines severity of finding #10); whether any other `#[command]` reads user-supplied paths without size caps (finding #9 generalized); a sweep of `.expect()` sites in command handlers (finding #5 generalized).
- **Session 3 (adversarial) should probe:** whether the `output_dir` / `output_path` parameters on convert/operations can escape the user-chosen directory (finding #3 fallout); whether a crafted preset JSON file on disk can cause a panic in preset-load path (presets validated at write, not at read — `presets.rs:11`); whether the `filmstrip-frame` event stream can be abused from frontend to drive the main thread to OOM (finding #4 + finding #6 compounded).
- Finding #3 (missing `validate_output_name`) is the single highest-leverage item: resolving it (either adding the function or rewriting the invariant) gives sessions 2/3 a clear trust-boundary to reason against.
