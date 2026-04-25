# TASK-5: Refactor model and model_blender conversion modules + comprehensive 3D sweep

## Goal
`convert::model` and `convert::model_blender` each expose a new `pub fn convert(...)` callable without `&Window`. `run()` becomes a thin wrapper. A new test file `src-tauri/tests/refactored_model_sweep.rs` calls each `convert()` with every supported 3D format pair, skipping cases where the required external tool isn't installed. Existing behavior is unchanged from a Tauri caller's perspective.

## Context
Fifth task in the `&Window` decoupling arc. TASK-1 established `convert::progress`. Earlier tasks refactored 9 modules and proved the pattern. This task handles 3D models, which use two different external tools:

- **model.rs** — uses `assimp` CLI for the common formats (obj, gltf, glb, stl, ply, dae, fbx, 3ds, x3d). The `build_assimp_args` and `assimp_format_id` helpers are already `pub`. The existing `extra_sweep.rs` already calls `build_assimp_args` directly without going through `run()`.
- **model_blender.rs** — uses Blender headless (`blender --background --python ... blender_convert.py`) for `blend`, `usd`, `usdz`, `abc`. Heavy external dep; treat as optional.

The dispatcher in `convert::model::run()` decides which path to take based on the input/output format combo. After the refactor, `convert::model::convert()` keeps that dispatch logic and may call into `convert::model_blender::convert()` for blender-required pairs.

Same wrapper pattern as previous tasks:

```rust
pub fn convert(
    input: &str,
    output: &str,
    opts: &ConvertOptions,
    progress: ProgressFn,
    processes: Arc<Mutex<HashMap<String, Child>>>,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> { /* ... */ }

pub fn run(...) -> ConvertResult {
    let mut emit = /* closure with original payload shape */;
    convert(..., &mut emit, processes, &cancelled)
}
```

Progress payload shape MUST match existing emit calls — frontend depends on it.

Live formats per `App.svelte` FORMAT_GROUPS line 1585:
- assimp path: `obj`, `gltf`, `glb`, `stl`, `ply`, `dae` (COLLADA), `3ds`, `x3d`, `fbx` (FBX ASCII)
- blender path: `usd`, `usdz`, `abc`, `blend`
- todo (skip): `step`, `iges`

The fixture is a minimal cube `.obj` (already used in `extra_sweep.rs::CUBE_OBJ` — copy that constant, do NOT cross-import test files).

Relevant files:
- `src-tauri/src/convert/model.rs` — assimp shell-out + dispatch to blender
- `src-tauri/src/convert/model_blender.rs` — blender shell-out
- `src-tauri/src/args/model.rs` — pure-Rust arg builders, already exposes `build_assimp_args`
- `src-tauri/src/convert/progress.rs` — from TASK-1
- `scripts/blender_convert.py` — the Python helper Blender runs; do NOT modify

## In scope
- `src-tauri/src/convert/model.rs` — extract `convert()`, reduce `run()`
- `src-tauri/src/convert/model_blender.rs` — same
- `src-tauri/tests/refactored_model_sweep.rs` (new file) — comprehensive 3D sweep

## Out of scope
- Any other `convert/*.rs` module
- `convert/archive.rs` — TASK-7
- `convert/image.rs`, `convert/audio.rs`, `convert/video.rs` — TASK-6
- `src-tauri/src/args/model.rs` — already has correct public surface; do not touch
- `scripts/blender_convert.py` — leave alone
- `src-tauri/src/lib.rs`
- The existing `extra_sweep.rs::model_sweep` test — leave it; it tests via `build_assimp_args` directly, complementary to the new `convert()` test

## Steps
1. Read `src-tauri/src/convert/progress.rs`, then one of TASK-2/3/4's refactored modules to refresh the wrapper pattern.
2. Read `src-tauri/src/convert/model.rs` end to end. Note: where is the format-pair dispatch (assimp vs blender)? How does it call into `model_blender`?
3. Read `src-tauri/src/convert/model_blender.rs` end to end. Note: how does it spawn Blender? What `window.emit` calls are inside?
4. Refactor `model_blender.rs` first (it's a leaf — `model.rs` calls into it):
   - Extract `pub fn convert()`, reduce `run()` to a wrapper.
   - Build: `cargo build --manifest-path src-tauri/Cargo.toml`.
5. Refactor `model.rs`:
   - Extract `pub fn convert()` keeping the dispatch logic.
   - Where the old code called `model_blender::run(window, ...)`, change to `model_blender::convert(input, output, opts, progress, processes.clone(), cancelled)`.
   - Reduce `run()` to a wrapper. Build.
6. Run all existing tests: `cargo test --manifest-path src-tauri/Cargo.toml`. All must pass — including `extra_sweep::model_sweep`, which exercises `build_assimp_args` directly.
7. Create `src-tauri/tests/refactored_model_sweep.rs`:
   - Synthesize the cube `.obj` fixture (copy `CUBE_OBJ` constant).
   - For each assimp target format (`obj`, `stl`, `ply`, `gltf`, `glb`, `dae`, `fbx`, `3ds`, `x3d`): call `model::convert()` with `noop_progress()` and an empty `processes` map. Assert output exists and is non-empty.
   - For each blender target format (`usd`, `usdz`, `abc`, `blend`): same, BUT skip if `which blender` returns nothing.
   - Print per-row PASS/FAIL/SKIP. Test exits 0 if no FAIL rows (SKIP rows allowed).
8. Compile-check: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_model_sweep --no-run`.
9. Run: `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_model_sweep -- --nocapture`. On a dev machine with assimp installed, expect 9 PASS + 4 SKIP (or all 13 PASS if Blender is also installed).

## Success signal
- `cargo build --manifest-path src-tauri/Cargo.toml` exits 0.
- `cargo test --manifest-path src-tauri/Cargo.toml` exits 0; `extra_sweep::model_sweep` still passes.
- `cargo test --manifest-path src-tauri/Cargo.toml --test refactored_model_sweep -- --nocapture` exits 0 with ≥ 9 PASS rows (assimp formats).
- Each refactored module has at most one `window.emit` site.

## Notes
- The cube `.obj` fixture is sufficient for assimp formats. For Blender formats it should also work — Blender will just ingest the obj.
- Don't try to write the assimp args inside `convert()` from scratch; call `build_assimp_args(...)` from `args::model`.
- Some assimp output formats may produce a directory (e.g. gltf+bin), not a single file. Check the existing module logic for this and preserve it. The test's "output exists and non-empty" check should look at the path the module reports as the primary output, not at byte count.
- Blender will probably not be in PATH on the test machine. Plan for `usd`/`usdz`/`abc`/`blend` to all SKIP, that's fine.
