# TASK-28: Close stale Blender Known Risks entry

## Goal
The `## Known Risks` section in `SESSION-STATUS.md` no longer carries the stale "Blender backend path fragility" entry. The entry is replaced with an accurate summary of what was hardened and what remains (no version check, no unit tests — covered in TASK-29).

## Context
The Known Risks entry reads:
> **Blender backend: `blender_convert.py` path resolution at runtime is fragile.** Binary discovery and script path construction are not hardened for all deployment contexts.

This is stale. A thorough code audit (2026-04-29) confirms the hardening is complete:
- `locate_script()` in `src-tauri/src/convert/model_blender.rs` (lines 13–58) tries 4 candidates: CWD-relative, exe-adjacent, macOS app bundle (`Contents/MacOS/../Resources/`), Linux FHS (`bin/../share/fade/`). Diagnostic error lists all paths tried.
- `find_blender()` in `src-tauri/src/args/model_blender.rs` (lines 4–67) does 5-stage fallback: PATH lookup → macOS hardcoded `/Applications/Blender.app/` + `~/Applications/` → Windows `Program Files` scan → Linux `/usr/bin/` + `/usr/local/bin/`.
- Both binaries and scripts emit actionable errors when not found.

The two remaining gaps — no unit tests and no version check — are tracked separately (TASK-29) and do not warrant a "fragile" risk label.

## In scope
- `SESSION-STATUS.md` — update the Blender Known Risks entry to "CLOSED" with a one-line summary of what's hardened and a forward pointer to TASK-29 for unit tests

## Out of scope
- Any Rust source file
- Any test file
- The INVESTIGATION-LOG.md

## Steps
1. Read `SESSION-STATUS.md` `## Known Risks` section.
2. Replace the stale Blender entry with:
   ```
   - **Blender backend path resolution — HARDENED.** `locate_script()` checks 4 candidate paths (CWD, exe-adjacent, macOS bundle `../Resources/`, Linux FHS `../share/fade/`). `find_blender()` does 5-stage fallback (PATH → macOS hardcoded → Windows Program Files scan → Linux hardcoded). Both emit diagnostic errors listing paths tried. Remaining gaps: no unit tests for path logic (TASK-29), no Blender version check (deferred).
   ```
3. Do NOT commit (dispatcher commits).

## Success signal
- The "fragile" label is removed from the Blender entry in SESSION-STATUS.md.
- The new entry accurately reflects the hardened state.
