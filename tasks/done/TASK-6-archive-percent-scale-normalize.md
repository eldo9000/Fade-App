# TASK-6: Normalise archive `Percent` to 0.0–1.0 and migrate to emitter helper

## Goal
`convert/archive.rs::convert()` emits `ProgressEvent::Percent(f32)` in the contract-mandated `0.0..=1.0` range, matching every other module. The TODO at `archive.rs:188-191` is removed and the `archive::run` wrapper migrates to `window_progress_emitter` (or `_batched`), eliminating its 25-line inline closure. The contract drift between `convert/progress.rs` (says 0.0–1.0) and the archive emit sites (emit 0–100) is closed.

## Context
The progress contract at `convert/progress.rs:17-29` is explicit:

```rust
/// `Percent` is normalized to `0.0..=1.0` (not `0..=100`); the Tauri wrapper
/// is responsible for scaling it to whatever the frontend expects.
```

Every module follows this except archive. Archive's emit sites at `convert/archive.rs:300, 363` (extract), and others in repack paths, emit values multiplied by `progress_scale` where the underlying value is 0–100 (from `parse_7z_percent` returning `f32` in 0–100). The archive `run()` wrapper at lines 188-216 has a TODO acknowledging this and inlining its own emit closure with `p.clamp(0.0, 100.0)` — masking the divergence rather than fixing it.

The `window_progress_emitter` helper in `convert/mod.rs` assumes 0.0–1.0 and multiplies by 100 internally. Archive can't use it today because passing 0–100 values would clamp to 100 immediately. The TODO comment at `archive.rs:188-191` says exactly this.

The fix is the principled one: normalise archive's emits to 0.0–1.0, then delete the inline closure and call `window_progress_emitter` like every other module. Net change is small but needs care:

1. Every emit site multiplies by `progress_scale`. After normalisation, `progress_scale` stays — it's already a 0.0–1.0 multiplier.
2. The 7z stdout parser returns 0–100. Convert at the source — `parse_7z_percent` returns `f32 / 100.0`, or wrap each emit site with `pct / 100.0`.
3. The hardcoded `25.0 * progress_scale` in the unar branch (line 300) becomes `0.25 * progress_scale`.

Relevant files:
- `src-tauri/src/convert/progress.rs:17-29` — the contract docstring
- `src-tauri/src/convert/archive.rs:188-216` — the inline closure with TODO
- `src-tauri/src/convert/archive.rs:300, 363` — emit sites in `extract_archive`
- `src-tauri/src/convert/archive.rs:816-820` — `parse_7z_percent`
- `src-tauri/src/convert/mod.rs` — `window_progress_emitter` (and `_batched`) definitions
- All other archive emit sites: search for `ProgressEvent::Percent` in `convert/archive.rs`

## In scope
- Normalise every `ProgressEvent::Percent(...)` emit in `convert/archive.rs` to use a 0.0–1.0 value. Change `parse_7z_percent` to divide by 100 at the parse site, or scale at each emit (parse-site is cleaner).
- Replace the inline emit closure at `archive.rs:192-216` with a call to `window_progress_emitter` (or `_batched`).
- Delete the TODO at lines 188-191.
- Add tests (or extend existing) that assert `parse_7z_percent` returns values in `0.0..=1.0`.

## Out of scope
- Refactoring `window_progress_emitter` itself.
- Touching other modules' emit semantics.
- The `Result<(), String>` vs `ConvertResult` split (architecture finding A-3; defer).
- Removing the `&Window` parameter from `archive::run` (it's still needed for `job-done`).

## Steps
1. Read `convert/archive.rs` end-to-end. Catalogue every `ProgressEvent::Percent` emit. Each must transition from 0–100 input to 0–1 input.
2. Read `convert/mod.rs` — confirm the signature of `window_progress_emitter` and `window_progress_emitter_batched`. Confirm the emit-message handling matches archive's needs (initial `Phase` message, terminal `Done`).
3. Modify `parse_7z_percent` (`archive.rs:816-820`) to return `0.0..=1.0`:
   ```rust
   fn parse_7z_percent(line: &str) -> Option<f32> {
       // ... existing parse logic ...
       let raw = trimmed[..pct_end].trim().parse::<f32>().ok()?;
       if !(0.0..=100.0).contains(&raw) {
           return None;
       }
       Some(raw / 100.0)
   }
   ```
4. At every emit site that previously assumed 0–100, replace literals:
   - `archive.rs:300` (unar branch): `progress(ProgressEvent::Percent(25.0 * progress_scale))` → `progress(ProgressEvent::Percent(0.25 * progress_scale))`
   - `archive.rs:363` (7z extract): unchanged — `pct` is already 0.0–1.0 after step 3.
   - Search for any other `ProgressEvent::Percent(` call sites in archive and adjust.
5. Replace the inline closure at `archive.rs:192-216` with:
   ```rust
   let mut emit = window_progress_emitter_batched(window, job_id, &initial_message);
   ```
   (Verify the helper signature accepts `&Window`, `&str`, `&str`. Adjust if it takes `String`.)
6. Delete the TODO at lines 188-191.
7. Update the `Phase` emit at `archive.rs:364` if needed — it currently does `format!("Extracting {}%", pct as u32)` where `pct` was 0–100. After normalisation, `pct` is 0.0–1.0, so update to `format!("Extracting {}%", (pct * 100.0) as u32)`.
8. Run the full integration suite to catch downstream breakage:
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --lib convert::archive
   cargo test --manifest-path src-tauri/Cargo.toml --test conversions
   ```
9. Manual smoke: `tauri dev`, convert a `.zip` → `.tar.gz`, observe the progress bar smoothly fills 0–100% (not stuck or jumping). Repeat with `.cbr` (unar branch) if available.
10. `cargo fmt` + `cargo clippy --all-targets -- -D warnings`.

## Success signal
- `grep "p\\.clamp(0.0, 100.0)" src-tauri/src/convert/archive.rs` returns no matches.
- `grep "TODO(batch-scale)" src-tauri/src/convert/archive.rs` returns no matches.
- `grep "window_progress_emitter" src-tauri/src/convert/archive.rs` returns 1+ match.
- All archive tests pass.
- Manual smoke: archive conversion progress bar still works correctly.
- `cargo clippy --all-targets -- -D warnings` exits 0.

## Notes
- This is the kind of contract erosion that produces silent visual bugs. The `clamp(0.0, 100.0)` masks the type confusion; a future module author copying from archive vs. video will get different behaviour for the same input range. Worth the cleanup.
- The unar branch's `25.0 * progress_scale` is a single placeholder emit (no incremental progress from unar). 25% as a midpoint guess. After normalisation, `0.25 * progress_scale`.
- If `window_progress_emitter_batched` doesn't accept the same arg shape archive needs, accept the small adapter cost — the goal is one canonical helper for all modules. Don't introduce a third variant.
- The integration test suite (`tests/conversions.rs` or similar) is the safety net — local archive tests with sample archives confirm the percent scale change doesn't break the end-to-end flow.
