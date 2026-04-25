# TASK-4: Refactor data and tracker conversion modules

## Goal
`convert::data` and `convert::tracker` each expose a new `pub fn convert(...)` callable without `&Window`. `run()` becomes a thin wrapper. Existing tests for `data` continue to pass. A new test `src-tauri/tests/refactored_data_tracker_sweep.rs` calls each `convert()` directly. Existing behavior is unchanged from a Tauri caller's perspective.

## Context
Fourth task in the `&Window` decoupling arc. TASK-1 established `convert::progress`. TASKs 2 and 3 refactored 7 modules and proved the pattern. This task handles two unusual ones:

- **data.rs** is the largest non-archive module. It already exposes pure helpers (`parse_input`, `write_output`) but its `run()` is a dispatcher with many sub-format paths (csv↔json↔yaml↔xml↔tsv, plus sqlite reader). The refactor preserves the dispatcher inside `convert()` and only lifts out `window.emit` calls.
- **tracker.rs** is a niche module (audio tracker formats). Read it first to confirm its actual surface — it may be small and trivial, or it may have its own quirks.

Same wrapper pattern as TASK-2 and TASK-3:

```rust
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> { /* ... */ }

pub fn run(window: &Window, ..., processes, cancelled) -> ConvertResult {
    let mut emit = /* closure that re-emits original payload shape */;
    convert(input, output, opts, &mut emit, processes, &cancelled)
}
```

Progress payload shape MUST match what the modules currently emit — frontend reads specific keys.

Relevant files:
- `src-tauri/src/convert/data.rs` — multi-format dispatcher; `parse_input` and `write_output` are already `pub`; the existing test in `src-tauri/tests/conversions.rs` (data_csv_to_json) already calls those helpers, not `run()`.
- `src-tauri/src/convert/tracker.rs` — niche; read it before deciding scope.
- `src-tauri/src/convert/progress.rs` — from TASK-1.

## In scope
- `src-tauri/src/convert/data.rs` — extract `convert()`, reduce `run()`
- `src-tauri/src/convert/tracker.rs` — same
- `src-tauri/tests/refactored_data_tracker_sweep.rs` (new file) — at least 3 cases for data (csv→json, csv→yaml, csv→xml), at least 1 case for tracker (or SKIP if tracker requires external tool that's unavailable)

## Out of scope
- Any other `convert/*.rs` module — not yet refactored modules will be done in later tasks
- `convert/archive.rs` — TASK-7
- `convert/model.rs`, `convert/model_blender.rs` — TASK-5
- `convert/image.rs`, `convert/audio.rs`, `convert/video.rs` — TASK-6
- The existing `src-tauri/tests/conversions.rs` — leave it alone, it already works
- `src-tauri/src/lib.rs`
- Any progress payload shape change

## Steps
1. Read `src-tauri/src/convert/progress.rs` and one of TASK-2/TASK-3's refactored modules to refresh the wrapper pattern.
2. Read `src-tauri/src/convert/tracker.rs` end to end. Record: what external tool (if any), what input/output formats, what `window.emit` calls exist. If it's trivial (≤ 50 lines, no shell-out), the refactor is mostly identical to email's. If it shells out, follow the TASK-3 pattern.
3. Refactor `data.rs`:
   - Find every `window.emit("job-progress", ...)` call inside `run()` and the helpers `run()` calls. Map each to a `ProgressEvent` variant.
   - Move the dispatch logic (the match on input/output extension) into `pub fn convert()`. Keep all the existing sub-format paths (csv, json, yaml, xml, tsv, sqlite) intact.
   - Reduce `run()` to a wrapper. Build: `cargo build --manifest-path src-tauri/Cargo.toml`.
4. Refactor `tracker.rs` using the same pattern. Build.
5. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. All must pass — especially `conversions.rs::data_csv_to_json`, which exercises `parse_input`/`write_output` directly and would surface accidental damage.
6. Create `src-tauri/tests/refactored_data_tracker_sweep.rs`:
   - Import `fade_lib::convert::{data, tracker, noop_progress}` and shared types.
   - For data: at least 3 cases. Synthesize a CSV fixture and call `data::convert()` with output formats `json`, `yaml`, `xml`. Assert each output exists and is non-empty.
   - For tracker: at least 1 case. If tracker requires an unavailable external tool, the test prints `[SKIP] tracker — <tool> not in PATH` and counts it as not-failed.
7. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_data_tracker_sweep --no-run`.
8. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_data_tracker_sweep -- --nocapture`.

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0; `data_csv_to_json` in `conversions.rs` still passes.
- The new sweep test exits 0 with at least 4 rows (3 data PASS + 1 tracker PASS or SKIP).
- `data.rs::run()` is ≤ 25 lines after refactor.
- `tracker.rs::run()` is ≤ 25 lines after refactor.

## Notes
- Data has the most existing public surface (`parse_input`, `write_output`) of any module. Don't duplicate logic — `convert()` should call those helpers, not re-implement parsing/writing.
- If tracker turns out to be empty/stub/unimplemented, the refactor for it is one minute of work; document that in the commit message.
- The dispatcher in `data.rs::run()` is probably the most non-trivial logic of any module besides archive. Be careful when lifting it: the match arms must keep their exact ordering and fall-through behavior.
