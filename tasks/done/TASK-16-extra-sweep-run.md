# TASK-16: Run extra_sweep.rs and document findings

## Goal
Every `#[ignore]` test in `src-tauri/tests/extra_sweep.rs` is executed manually. Failures are appended to `INVESTIGATION-LOG.md` as OPEN entries. The output is a documented findings report — failures are expected and valuable.

## Context
`extra_sweep.rs` covers four domains not included in `full_sweep.rs`:
- `model_sweep` — 3D model conversions (OBJ, STL, PLY, GLTF, GLB, DAE, FBX, 3DS, X3D) via assimp
- `subtitle_sweep` — SRT ↔ SBV round-trips and conversions
- `email_sweep` — EML ↔ MBOX conversions
- `document_text_sweep` — Markdown → TXT, HTML → TXT, HTML → Markdown

Unlike `full_sweep.rs`, these tests do not primarily surface codec encoder-constraints (BC-005). They surface missing-tool dependencies (assimp, pandoc, etc.), format-handling edge cases, and round-trip fidelity issues.

All four tests are `#[ignore]` — manual-only.

## In scope
- `src-tauri/tests/extra_sweep.rs` — the sweep file to run
- `INVESTIGATION-LOG.md` — append new OPEN entries for new findings

## Out of scope
- `src-tauri/tests/full_sweep.rs` — covered in TASK-15
- Fixing any failures found — document only
- Modifying test cases or sweep structure
- Any source file under `src-tauri/src/`

## Steps
1. Read `INVESTIGATION-LOG.md` tail (last 20 lines) to see existing OPEN entries — avoid duplicating them.
2. Run all four categories:
   ```
   cargo test --manifest-path src-tauri/Cargo.toml --test extra_sweep -- --include-ignored 2>&1 | tee /tmp/fade_extra_sweep.txt
   ```
3. Parse output for `FAILED` lines. For each failed test:
   - Read the failure output to identify the root cause (missing binary, format error, fidelity failure, etc.).
   - Group by distinct failure class.
4. For each NEW finding not already in INVESTIGATION-LOG.md, append:
   ```
   2026-04-28 | OPEN | <description> — extra_sweep <test_function>
   ```
5. Report totals: PASSED / FAILED counts and list of new entries written (or "no new findings").

## Success signal
- `cargo test` command completes without hanging (timeout > 10 min is a hang).
- INVESTIGATION-LOG.md has one OPEN entry per new failure class (zero new entries is valid).
- Notes return includes PASSED / FAILED counts.

## Notes
- `model_sweep` requires `assimp` CLI in PATH. If absent, all model cases will fail with "command not found" — that's a single finding (missing dep), not N separate findings. One OPEN entry suffices.
- `subtitle_sweep` is pure-Rust (no ffmpeg), so failures indicate logic bugs in the SRT/SBV converters.
- `email_sweep` and `document_text_sweep` may require external tools (pandoc, etc.) — missing-tool failures follow the same single-entry rule as model_sweep.
