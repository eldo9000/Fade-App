# Fade — Session Status

Last updated: 2026-04-25 (updated post-sprint)

---

## Current Focus

Two full sessions of substantial work landed 2026-04-25.

**Session A — test sweep infrastructure (`46c37db`):** Three new test files added to `src-tauri/tests/`: `matrix.rs` (33-case smoke matrix; pre-release sanity gate), `full_sweep.rs` (~700-case Cartesian diagnostic; surfaces broken combos), `extra_sweep.rs` (cheap-to-test categories: 3D models, subtitle pure-Rust, email, document text). Seven helper functions made `pub` across `email.rs`, `subtitle.rs`, `document.rs` to support direct test calls. All sweep tests subsequently marked `#[ignore]` (manual-only; CI runs `--lib` and `--include-ignored` only on the conversions integration suite).

**Session B — `&Window` decoupling refactor arc (8 tasks, all CI-green):** All 15 conversion modules (`email`, `subtitle`, `document`, `notebook`, `timeline`, `font`, `ebook`, `data`, `tracker`, `model`, `model_blender`, `image`, `audio`, `video`, `archive`) split into a pure `pub fn convert(...)` + thin `pub fn run(...)` wrapper. Established `convert::progress::{ProgressEvent, ProgressFn, noop_progress}` contract for Window-free invocation. Added 6 new `refactored_*_sweep.rs` test files (one per module group), each calling `convert()` directly with `noop_progress()`. All tests `#[ignore]`. 309 tests passing (`--include-ignored`). Conversion pipeline contract documented in `ARCHITECTURE.md`.

CI green on `main`. Arc closed 2026-04-25.

## Next action

Second arc complete (3 tasks, all CI-green, 2026-04-25). No specific arc in flight — ready for new feature work or another diagnostic sweep.

## Audit outcome summary

**33 findings closed across 18 batches (B1–B18).** Key structural wins:

- `JobOutcome` typed enum replaced string sentinels `"CANCELLED"` / `"__DONE__"` (B11)
- ts-rs codegen: 12 TypeScript types generated at build time from Rust structs (B17)
- Streaming waveform RMS: O(file) → O(n) memory for `get_waveform` (B12)
- `run_ffmpeg` consolidated from 3 diverged copies to 1 canonical with rate-limiter (B8)
- `createLimiter` batch concurrency semaphore: 100 unbounded ffmpegs → clamped to `hardwareConcurrency` (B10)
- validate_output_name umbrella covering all 29 `OperationPayload` variants (B15)
- parking_lot::Mutex across 32 files, return-shape drift normalized (B18)

## Known Risks

- **`$bindable` chain verified correct** — all mutation paths use `$bindable()` + `bind:` explicitly.
- **Blender backend: `blender_convert.py` path resolution at runtime is fragile.** Binary discovery and script path construction are not hardened for all deployment contexts. (BC-003/BC-004 code bugs resolved; runtime path fragility is a separate concern.)
- **analysis-result one-shot listener race.** The one-shot event listener introduced in async IPC migration is set up before the invoke call; if the event fires before `unlisten` is registered on a very fast completion, the result may be missed. Structurally possible, not yet observed.

**full_sweep.rs diagnostic findings (2026-04-25):**
- **H.264 profile/pix_fmt impossible combos — CLOSED (TASK-1 + TASK-2 this arc).** `full_sweep` surfaced 660 failing H.264 combos; fixed by arg-builder auto-promotion (`723cbff`) + UI disable of unreachable profile buttons (`50c89cb`).
- **Missing `1px.jpg` fixture — CLOSED (TASK-4 this arc).** Pre-existing ignored test in `lib.rs` referenced a non-existent fixture; fixture restored (`8b61613`).
- **AVIF speed cap — CLOSED (`457d22c`).** Clamped to 9 in arg builder + UI slider capped at 9.
- **DNxHR minimum resolution — CLOSED (`0d1c045`).** Guard in `convert/video.rs` returns clear error for sub-1280×720 resolution.
- **7zz tar.gz/tar.xz repack — CLOSED (`8e8298e`).** `repack_tar_compressed` two-step function added to `convert/archive.rs`.

## Mode

Active development. Two diagnostic-driven cleanup arcs closed 2026-04-25 (8 tasks total). No arc in flight.
