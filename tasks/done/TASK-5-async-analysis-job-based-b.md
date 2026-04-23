# TASK-5: Full job-based async migration — analysis batch B (vmaf, waveform, spectrogram, preview_diff)

## Goal
Four remaining long-running analysis/preview commands are migrated to the full job-based lifecycle: they accept a `job_id`, spawn a thread, register the FFmpeg child for cancellation, emit result events, and return `Ok(())` immediately. Their frontend callers are updated to use the new pattern and cancel in-flight jobs when the user switches files.

## Context
This task is a direct continuation of TASK-4. The same backend and frontend migration pattern applies. Read TASK-4 in full before executing this task — the `run_ffmpeg_capture_registered` helper and `invokeAnalysis` frontend helper created in TASK-4 are prerequisites.

**The 4 commands:**

| Command | File | Duration | Frontend caller |
|---|---|---|---|
| `analyze_vmaf` | `src-tauri/src/operations/analysis/vmaf.rs` | Minutes on long files | `AnalysisTools.svelte:161` |
| `get_waveform` | `src-tauri/src/probe/waveform.rs` | Seconds–minutes | `QueueManager.svelte:174`, `Timeline.svelte:692` |
| `get_spectrogram` | `src-tauri/src/probe/spectrogram.rs` | Seconds | `Timeline.svelte:703` |
| `preview_diff` | `src-tauri/src/preview/video_diff.rs` | Seconds–minutes | `App.svelte:456` |

**Backend pattern:** Same as TASK-4 — add `window: Window, state: State<'_, AppState>, job_id: String`; spawn thread; use `run_ffmpeg_capture_registered` (or spawn manually for `get_waveform` which uses streaming, see Notes); register in `state.processes` and `state.cancellations`; emit `analysis-result:{job_id}`.

**`get_waveform` special case:** The current implementation in `probe/waveform.rs` uses a custom streaming reader (reads PCM chunks, computes RMS and ZCR in a bucket accumulator). It does NOT use `run_ffmpeg_capture` — it spawns FFmpeg with `stdout(Stdio::piped())` and reads stdout directly. This means it already has a child handle. The migration just needs to: register the child in `state.processes`, check the `cancelled` flag periodically in the streaming loop (every N buckets or every N reads), and break + emit cancellation event if set.

**`get_spectrogram` pattern:** Likely similar to the analysis commands (runs FFmpeg, captures output). Read the file to confirm.

**`analyze_vmaf` note:** VMAF analysis is the heaviest command — it can run 10+ minutes on long content. The cancellation support here is the most valuable of all 14 commands. Ensure the cancellation check fires within a reasonable time after `cancel_job` is called.

**Frontend callers:**

- `AnalysisTools.svelte:161` — uses `invokeAnalysis` helper from TASK-4 (same file, same helper). Reuse.
- `QueueManager.svelte:174` — calls `get_waveform` in `_bgPreloadNext`. This is a background preload; if the queue item changes, the preload should be cancelled. Track the in-flight `jobId` in `_bgBusy` state; cancel before starting a new preload for a different item.
- `Timeline.svelte:692–703` — calls both `get_waveform` and `get_spectrogram` inside `$effect` blocks. The `_capturedId` / `id` pattern is already used for stale-check; when `_capturedId !== id`, `cancel_job` should be called with the in-flight waveform/spectrogram jobId.
- `App.svelte:456` — calls `preview_diff` in a function. Track the in-flight jobId; cancel on new invocation or component destroy.

**Event name:** Same pattern as TASK-4: `analysis-result:{job_id}`. Each frontend caller sets up a one-shot listener before calling invoke.

## In scope
- `src-tauri/src/operations/analysis/vmaf.rs`
- `src-tauri/src/probe/waveform.rs`
- `src-tauri/src/probe/spectrogram.rs`
- `src-tauri/src/preview/video_diff.rs`
- `src/lib/AnalysisTools.svelte` — vmaf caller only
- `src/lib/QueueManager.svelte` — waveform caller in `_bgPreloadNext`
- `src/lib/Timeline.svelte` — waveform and spectrogram callers
- `src/App.svelte` — preview_diff caller

## Out of scope
- `run_ffmpeg_capture_registered` — already created in TASK-4, do not recreate
- `invokeAnalysis` helper in AnalysisTools — already created in TASK-4, reuse it
- TASK-3 and TASK-4 commands — do not touch
- Any Timeline logic other than the waveform/spectrogram `$effect` blocks

## Steps
1. Confirm TASK-4 is complete: `run_ffmpeg_capture_registered` exists in `operations/analysis/mod.rs` and `invokeAnalysis` exists in `AnalysisTools.svelte`.
2. Read `probe/waveform.rs` in full to understand the streaming architecture.
3. Read `probe/spectrogram.rs`, `operations/analysis/vmaf.rs`, and `preview/video_diff.rs` in full.
4. Migrate `analyze_vmaf` and `preview_diff` using `run_ffmpeg_capture_registered` (same pattern as TASK-4).
5. Migrate `get_waveform`: add `window`, `state`, `job_id` params; register child and cancellation flag; add a cancellation check inside the streaming loop (check `cancelled.load()` every ~100 buckets or every read iteration); emit `analysis-result:{job_id}` on completion or cancellation.
6. Migrate `get_spectrogram` similarly.
7. Define result payload structs for all 4 commands.
8. Update `AnalysisTools.svelte` vmaf caller to use `invokeAnalysis`.
9. Update `QueueManager.svelte`: give `_bgPreloadNext` a `let _waveformJobId = null` tracker; cancel before each new preload.
10. Update `Timeline.svelte`: track waveform and spectrogram `jobId` per `$effect`; when `_capturedId !== id` (stale), call `invoke('cancel_job', { jobId })` with the tracked id before clearing it.
11. Update `App.svelte` preview_diff caller to use job_id pattern.
12. Run `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `npm run test`.

## Success signal
`cargo test` and vitest pass. `cargo clippy -D warnings` clean. In Timeline, loading a file while `get_waveform` is in flight results in a `cancel_job` call for the previous waveform job. `grep -n "^pub fn get_waveform\|^pub fn get_spectrogram\|^pub fn analyze_vmaf\|^pub fn preview_diff" src-tauri/src/**/*.rs` returns no hits — all are migrated.

## Notes
For `get_waveform`, the cancellation check in the streaming loop must handle the case where the child has already been killed by `cancel_job`. After `kill()` is called externally, the next read on the child's stdout pipe will return EOF or an error — the loop will exit naturally. The cancellation flag check is belt-and-suspenders to catch the case where the kill signal arrives between reads.

The `waveformData = null` reset in Timeline's `$effect` (before the invoke) serves as the existing stale-check. After this task, also call `invoke('cancel_job', { jobId })` at that same point if a jobId is in flight.
