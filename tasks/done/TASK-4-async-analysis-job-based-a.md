# TASK-4: Full job-based async migration — analysis batch A (cut_detect, black_detect, loudness, framemd5)

## Goal
Four long-running analysis commands are migrated to the full job-based lifecycle pattern: they accept a `job_id`, spawn a thread, register the FFmpeg child for cancellation, emit a typed result event, and return `Ok(())` immediately. Their frontend callers in `AnalysisTools.svelte` are updated to use the new pattern.

## Context
These four commands can run for seconds to minutes on large files. They currently block the IPC thread for their full duration and are uncancellable — killing the FFmpeg process is impossible because the child handle is never registered. This is CONC-011 in the audit ledger.

**The 4 commands:**

| Command | File | Current return type |
|---|---|---|
| `analyze_cut_detect` | `src-tauri/src/operations/analysis/cut_detect.rs` | `Result<Vec<CutPoint>, String>` |
| `analyze_black_detect` | `src-tauri/src/operations/analysis/black_detect.rs` | `Result<Vec<BlackSegment>, String>` |
| `analyze_loudness` | `src-tauri/src/operations/analysis/loudness.rs` | `Result<LoudnessResult, String>` |
| `analyze_framemd5` | `src-tauri/src/operations/analysis/framemd5.rs` | `Result<Vec<FrameMd5>, String>` |

All four currently use `run_ffmpeg_capture(args)` from `operations/analysis/mod.rs`, which calls `Command::output()` and returns captured stderr. This gives no child handle for cancellation.

**Backend migration pattern** (mirrors `convert_file` and `run_operation` in `lib.rs`):

1. Add new params: `window: Window`, `state: State<'_, AppState>`, `job_id: String`.
2. Register an `AtomicBool` cancellation flag in `state.cancellations`.
3. Spawn a `std::thread::spawn` closure.
4. Inside the thread: spawn the FFmpeg child manually (not via `run_ffmpeg_capture`), register the child in `state.processes` under `job_id`, wait for completion, check the cancellation flag, emit the result event.
5. Return `Ok(())` immediately from the command function.

**New helper needed in `operations/analysis/mod.rs`:** Add `run_ffmpeg_capture_registered(args, processes, job_id, cancelled) -> Result<String, String>` that:
- Spawns FFmpeg with `stdout(Stdio::piped())` and `stderr(Stdio::piped())`
- Inserts the child into `processes` map under `job_id`
- Reads stderr while the child runs (via `wait_with_output` or thread-reading)
- After wait: checks `cancelled` flag; if set, returns `Err("CANCELLED")`
- Removes child from processes map before returning
- Returns captured stderr on success

**Event name for results:** Emit on `analysis-result:{job_id}` (e.g., `window.emit(&format!("analysis-result:{}", job_id), payload)`). Payload shape: `AnalysisResult<T> { job_id: String, data: Option<T>, error: Option<String> }`.

Define `AnalysisResult` as a generic struct or use `serde_json::Value` for `data`. A concrete approach: define one payload struct per command (e.g., `CutDetectResult { job_id, cuts: Option<Vec<CutPoint>>, error: Option<String> }`). Each command emits its own named struct.

**Frontend migration — `AnalysisTools.svelte`:**

Current caller pattern (e.g., line 118):
```javascript
const res = await invoke('analyze_cut_detect', { inputPath, algo, threshold, minShotS });
```

New pattern — create a shared helper in `AnalysisTools.svelte`:
```javascript
async function invokeAnalysis(command, params) {
  const jobId = crypto.randomUUID();
  return new Promise((resolve, reject) => {
    let unlisten;
    listen(`analysis-result:${jobId}`, (ev) => {
      unlisten?.then(fn => fn());
      if (ev.payload.error) reject(ev.payload.error);
      else resolve(ev.payload.data ?? ev.payload);
    }).then(fn => { unlisten = Promise.resolve(fn); });
    invoke(command, { jobId, ...params }).catch(reject);
  });
}
```

Replace each analysis invoke call with `await invokeAnalysis('analyze_cut_detect', { inputPath, ... })`.

**Cancellation hook:** When a stale-check fires in AnalysisTools (user switches away mid-analysis), call `invoke('cancel_job', { jobId })`. Track the in-flight `jobId` per analysis type. When a new analysis of the same type starts while one is in flight, cancel the previous one first.

`cancel_job` already exists in `lib.rs:918` — it kills the child via the processes map and sets the cancellation flag.

## In scope
- `src-tauri/src/operations/analysis/mod.rs` — new `run_ffmpeg_capture_registered` helper
- `src-tauri/src/operations/analysis/cut_detect.rs`
- `src-tauri/src/operations/analysis/black_detect.rs`
- `src-tauri/src/operations/analysis/loudness.rs`
- `src-tauri/src/operations/analysis/framemd5.rs`
- `src/lib/AnalysisTools.svelte` — callers for these 4 commands only

## Out of scope
- `analyze_vmaf` — TASK-5 (it has the same pattern but is the heaviest command; batched separately)
- `get_waveform`, `get_spectrogram`, `preview_diff` — TASK-5
- `lib.rs` AppState definition — do not change; `state.processes` and `state.cancellations` already exist
- Any other AnalysisTools functions (loudness normalize runs via `run_operation`, do not touch)
- TASK-3 commands

## Steps
1. Read `src-tauri/src/lib.rs` lines 40–55 (AppState) and lines 663–690 (convert_file thread spawn pattern) to understand the target shape.
2. Read all 4 analysis command files in full.
3. Add `run_ffmpeg_capture_registered` to `operations/analysis/mod.rs`. Use `Command::new("ffmpeg").args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`, register child, wait, check cancelled, return stderr.
4. Update each of the 4 command functions: add `window`, `state`, `job_id` params; register cancellation flag; spawn thread; call `run_ffmpeg_capture_registered`; parse result; emit `analysis-result:{job_id}` event; return `Ok(())`.
5. Define per-command result payload structs (derive `Serialize`, `Clone`).
6. Read `src/lib/AnalysisTools.svelte` lines 60–210 (analysis runner functions).
7. Add `invokeAnalysis` helper to AnalysisTools.
8. Update `runCutDetect`, `runBlackDetect`, `runLoudness`, `runFrameMd5` to use `invokeAnalysis`.
9. Add per-analysis in-flight jobId tracking; cancel on new invocation of same type.
10. Run `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`.
11. Run `npm run test` (vitest) to confirm frontend tests pass.

## Success signal
`cargo test` and vitest both pass. `cargo clippy -D warnings` clean. In AnalysisTools, running a cut detect and immediately switching away (triggering cancel) results in `cancel_job` being called with the in-flight jobId. The four commands no longer appear as synchronous `pub fn` in their files — all are `pub fn ...(window: Window, state: State<...>, job_id: String, ...)`.

## Notes
The `processes` map holds `std::process::Child`. When the thread removes it after completion (whether success or cancel), `cancel_job` may be called after removal — that is safe, it simply finds nothing to kill.

The `cancellations` map is cleaned up inside the thread after the result is emitted. Follow the same pattern as `convert_file` (lib.rs:896–900).

For `analyze_loudness`, the result struct may use floating-point fields. Verify the existing `LoudnessResult` is `Serialize + Clone`; add derives if missing.
