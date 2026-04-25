# TASK-3: Refactor the single-tool shell-out modules (notebook, timeline, font, ebook)

## Goal
`convert::notebook`, `convert::timeline`, `convert::font`, and `convert::ebook` each expose a new `pub fn convert(...)` callable without `&Window`. `run()` becomes a thin wrapper. A new test file `src-tauri/tests/refactored_shellout_sweep.rs` calls each `convert()` with at least one case, skipping cases where the required external tool isn't in PATH (rather than failing). Existing behavior is unchanged from a Tauri caller's perspective.

## Context
This is the third task in the `&Window` decoupling arc. TASK-1 added `convert::progress::{ProgressEvent, ProgressFn, noop_progress}`. TASK-2 refactored the pure-Rust modules and established the wrapper pattern. This task applies the same pattern to four modules that each shell out to one external tool.

The pattern (identical to TASK-2):

```rust
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    progress(ProgressEvent::Started);
    // ... existing logic, with every window.emit replaced by progress(...) ...
    progress(ProgressEvent::Done);
    Ok(())
}

pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult {
    let job_id = job_id.to_string();
    let win = window.clone();
    let mut emit = move |ev: ProgressEvent| {
        let payload = serde_json::json!({ "jobId": job_id, /* shape from before */ });
        let _ = win.emit("job-progress", payload);
    };
    convert(input, output, opts, &mut emit, processes, &cancelled)
}
```

Pass `processes` as an arg to `convert()` exactly as TASK-2 established. The progress payload emitted by the wrapper closure must match the JSON shape these modules already emit — frontend reads those keys. Don't invent new fields.

External tools per module:
- **notebook** — `jupyter nbconvert` (pip-installed, may not be in PATH)
- **timeline** — `otioconvert` from opentimelineio (pip-installed, may not be in PATH)
- **font** — `fonttools` / `pyftsubset` (pip-installed, may not be in PATH)
- **ebook** — `ebook-convert` from Calibre (may not be in PATH)

Tests must skip cases where the tool is missing rather than failing. Pattern:

```rust
fn tool_available(name: &str) -> bool {
    Command::new(name).arg("--version").output()
        .map(|o| o.status.success() || !o.stderr.is_empty())
        .unwrap_or(false)
}

if !tool_available("jupyter") {
    println!("  [SKIP] notebook_ipynb_to_md — jupyter not in PATH");
    continue;
}
```

Relevant files:
- `src-tauri/src/convert/notebook.rs`
- `src-tauri/src/convert/timeline.rs`
- `src-tauri/src/convert/font.rs`
- `src-tauri/src/convert/ebook.rs`
- `src-tauri/src/convert/progress.rs` — from TASK-1; import from here.

## In scope
- `src-tauri/src/convert/notebook.rs` — extract `convert()`, reduce `run()` to wrapper
- `src-tauri/src/convert/timeline.rs` — same
- `src-tauri/src/convert/font.rs` — same
- `src-tauri/src/convert/ebook.rs` — same
- `src-tauri/tests/refactored_shellout_sweep.rs` (new file) — at least one case per module, with PATH-availability skipping

## Out of scope
- `convert/archive.rs` — much more complex, TASK-7
- `convert/data.rs`, `convert/tracker.rs` — TASK-4
- `convert/model.rs`, `convert/model_blender.rs` — TASK-5
- `convert/image.rs`, `convert/audio.rs`, `convert/video.rs` — TASK-6
- `convert/email.rs`, `convert/subtitle.rs`, `convert/document.rs` — already done in TASK-2
- `src-tauri/src/lib.rs`
- Any existing test file
- Any progress payload shape change

## Steps
1. Read `src-tauri/src/convert/progress.rs` to refresh the contract. Read TASK-2's refactored modules (pick one, e.g. `email.rs`) to see the established wrapper pattern in action.
2. For `notebook.rs`: identify the body of `run()`, extract everything except `window.emit` calls into `pub fn convert()`. Replace each emit with `progress(ProgressEvent::...)`. Reduce `run()` to a wrapper. Build: `cargo build --manifest-path src-tauri/Cargo.toml`.
3. Repeat step 2 for `timeline.rs`.
4. Repeat step 2 for `font.rs`.
5. Repeat step 2 for `ebook.rs`.
6. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. All must pass.
7. Create `src-tauri/tests/refactored_shellout_sweep.rs`. For each module, synthesize one minimal fixture (a valid ipynb stub for notebook, a minimal otio JSON for timeline, a tiny TTF or pre-existing font fixture for font, a minimal EPUB for ebook). For inputs you cannot trivially synthesize (real EPUB, real font), put an `if !std::path::Path::new("path/to/sample").exists() { skip }` guard. Call each `convert()` with `noop_progress()` and an empty `processes` map. Assert output exists OR skip if tool/fixture missing. Print `[SKIP]` rows in the report so the user can see what didn't run.
8. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_shellout_sweep --no-run`. Must succeed.
9. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_shellout_sweep -- --nocapture`. Should exit 0; cases either PASS or SKIP, none should FAIL on the dev machine that has the tools installed.

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0.
- The new sweep test exits 0 with at least 4 rows in the report (one per module), each PASS or SKIP — no FAIL.
- Each of the 4 refactored modules has at most one `window.emit` site.
- `wc -l` on `run()` in each refactored module shows ≤ 25 lines.

## Notes
- For minimal fixtures: a valid ipynb is just `{"cells": [], "metadata": {}, "nbformat": 4, "nbformat_minor": 5}`. A minimal otio is similar JSON. Font and ebook are harder to synthesize; SKIP them if no fixture available.
- If a module emits multiple progress shapes (e.g. different `phase` values during multi-step conversion), preserve every one of them in the `run()` wrapper closure. Drop nothing.
- Calibre's `ebook-convert` is the slowest external tool here; the test should not require it to actually run successfully — fixture+tool availability skip is fine.
- DO NOT remove `processes` from the `run()` signature. The dispatcher in `lib.rs` passes it; signature must stay stable.
