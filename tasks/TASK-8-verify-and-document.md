# TASK-8: Verify acceptance criteria and document the conversion contract

## Goal
The full Fade test suite passes, every conversion module satisfies the refactor acceptance criteria, and the new `convert()` / `run()` contract is documented in `ARCHITECTURE.md` so future contributors know which to add code to.

## Context
Final task in the `&Window` decoupling arc. TASKs 1–7 have refactored every conversion module: `email`, `subtitle`, `document`, `notebook`, `timeline`, `font`, `ebook`, `data`, `tracker`, `model`, `model_blender`, `image`, `audio`, `video`, `archive`. Each now exposes `pub fn convert(...)` (pure, no `&Window`) and `pub fn run(...)` (thin Tauri wrapper). Multiple new test files have landed.

This task does no further refactoring. It verifies the work and writes the documentation.

The contract to document, plus the files/conventions to verify:

```rust
// Pure conversion — callable from tests, CLIs, anything. No Tauri runtime needed.
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String>;

// Thin Tauri wrapper — translates progress events into window.emit calls.
pub fn run(
    window: &Window,
    job_id: &str,
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: Arc<AtomicBool>,
) -> ConvertResult;
```

Relevant files:
- `ARCHITECTURE.md` at repo root — add new section
- `src-tauri/src/convert/*.rs` — verify each module satisfies acceptance criteria
- `src-tauri/tests/*.rs` — verify the suite is green

## In scope
- `ARCHITECTURE.md` — add a new section "Conversion pipeline contract"
- `AUTOMATED-TESTING.md` — append references to the new sweep tests
- Verification of acceptance criteria across all 15 conversion modules
- Cleanup of any TODO comments left by earlier refactor tasks

## Out of scope
- Any further refactoring
- Adding new test cases beyond what TASKs 2–7 already produced
- Changes to `lib.rs` (deferred to a future arc — see Notes)
- Changes to the `convert()` or `run()` signatures

## Steps
1. Run the full test suite: `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored`. Every test must pass. Record total test count.
2. Verify acceptance criteria per module. For each of the 15 conversion modules:
   - **a.** `wc -l` on the `pub fn run(...)` body — must be ≤ 30 lines (≤ 25 for non-AV modules, ≤ 30 for AV modules per TASK-6).
   - **b.** `pub fn convert(...)` exists with the signature shown in Context.
   - **c.** No body of `convert()` references `&Window`, `tauri::Emitter`, `tauri::Manager`, or any other `tauri::*` type.
   - **d.** The wrapper `run()` has at most one `window.emit(...)` site (inside the closure that becomes the `ProgressFn`).
   Use `grep -c "window.emit" src-tauri/src/convert/<file>.rs` to verify (d) — each file should show at most 1.
3. If any module fails (a)–(d), open it, fix the gap, and re-run step 1. Do not accept the task done with red criteria.
4. Document the contract in `ARCHITECTURE.md`. Add a new section near the top titled "Conversion pipeline contract". Cover:
   - Why the split exists (the `&Window` coupling story; the testability + portability gain).
   - The two functions per module, with their signatures.
   - Where to add new code (almost always `convert()`).
   - How `ProgressFn` works and what `noop_progress()` is for.
   - One-paragraph note on the future direction: a single generic `run()` wrapper in `convert::mod` could replace all 15 per-module wrappers, since they're now identical boilerplate. Flagged as a future arc, not in scope here.
5. Update `AUTOMATED-TESTING.md`: add a "Refactored conversion sweeps" subsection listing the new test files and what each covers. Cross-link from the existing matrix/full-sweep sections.
6. Search for refactor-leftover TODO comments: `grep -rn "TODO\|FIXME\|XXX" src-tauri/src/convert/`. Any TODOs added during TASKs 2–7 either get resolved now or get a tracking comment with a brief rationale.
7. Final sanity: run the full test suite once more, end-to-end. Capture the output and include the test count + duration in the commit message.

## Success signal
- `cargo test --manifest-path src-tauri/Cargo.toml -- --include-ignored` exits 0; total test count is greater than what existed before TASK-1 (new sweep tests should add to the total).
- `for f in src-tauri/src/convert/*.rs; do echo $f $(grep -c "window.emit" $f); done` shows every conversion module has at most 1 `window.emit` site (modules with no progress reporting will show 0).
- `ARCHITECTURE.md` has a new section "Conversion pipeline contract" covering the split, the signatures, and where to add code.
- `AUTOMATED-TESTING.md` lists every new sweep test file (`refactored_pure_sweep`, `refactored_shellout_sweep`, `refactored_data_tracker_sweep`, `refactored_model_sweep`, `refactored_av_sweep`, `refactored_archive_sweep`) and one-line summaries.
- No TODO comments remain in `src-tauri/src/convert/` from the refactor tasks (or any remaining ones have explicit "tracked elsewhere" rationale).

## Notes
- The `lib.rs` dispatcher (around line 853) still calls `run_<category>_convert` from `convert::mod`. That's fine for now — the wrappers still work. A future arc can migrate the dispatcher to call `convert()` directly and centralize progress emission, eliminating per-module wrappers entirely. Note this in the ARCHITECTURE.md "future direction" paragraph.
- If the verification in step 2 turns up a module that doesn't quite satisfy criteria (e.g. `run()` ended up at 32 lines instead of 30), make a judgment call: either trim or document the over-shoot. Don't burn time on cosmetic fixes.
- This task is verification + docs only. If you find a real refactor bug — e.g. a module where the wrapper closure's progress payload doesn't match what frontend expects — STOP, do not patch it under this task. Open it as a bug, surface to user, decide whether to fix here or schedule a TASK-9.
