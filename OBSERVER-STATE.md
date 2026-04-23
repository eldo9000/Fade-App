# Fade-App Observer State
Last updated: 2026-04-23  ·  Run 8

---

## Active development areas

The AudioOffset precision drift micro-patch landed in `3fd9750`: `offset_ms` changed from `i64` to `i32` in both the `OperationPayload::AudioOffset` enum variant in `lib.rs` and the `run()` function signature in `audio_offset.rs`. The ts-rs binding in `OperationPayload.ts` regenerated automatically, changing the TypeScript type from `bigint` to `number`. All three checks (cargo test 271 passed, clippy -D warnings clean, fmt --check clean) passed. CI green.

The task spec had a factual error: it described the struct as living in `audio_offset.rs`, but the ts-rs-annotated type lives in the `OperationPayload` enum in `lib.rs`. The worker correctly halted on the first attempt, the task was corrected, and the second dispatch succeeded. This is a known architectural pattern in this codebase: IPC boundary types are defined inline in `lib.rs` as enum variants, not in per-module struct files.

No feature work is currently in flight. The project is between sprints.

## Fragile / high-risk areas

The **Blender backend** fragilities (BC-003, BC-004) are unchanged. `blender_convert.py` path resolution at runtime and USD import empty-scene silent success both remain unmitigated.

The **`$bindable` chain** persists without change across all recent sprints.

The **analysis-result one-shot listener race** persists. Structurally possible; not observed.

SESSION-STATUS `## Known Risks` still lists "AudioOffset i64→i32 precision drift" as an open lower-urgency gap, and `## Mode` still reads "Next: AudioOffset precision drift micro-patch." Both are stale — AudioOffset is fixed. The Known Risks section also still lists "Windows non-C drive preview" and "GHA shell injection" as deferred items, which is accurate, but the AudioOffset entry should be removed. Minor drift.

## Deferred work accumulation

Two items remain from the post-audit formal deferred list:

1. **Windows non-C drive preview** — named as `## Next action` in SESSION-STATUS. `assetProtocol.scope` in `tauri.conf.json` covers only the default drive; secondary drives are blocked on Windows. No fix has been attempted. The scope of the fix is narrow: `tauri.conf.json` asset protocol configuration.
2. **GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` in `run:` steps. Low priority; not exploitable without write access.

The `librewin_common`, STEP/IGES, and VBR/CBR VP9/AV1 fallback items remain off the formal ledger.

The `## Audit outcome summary` in SESSION-STATUS still lists AudioOffset as a deferred item that has not been addressed. This is now incorrect — it was addressed in `3fd9750`.

## Pattern watch

The **task spec error pattern** surfaced again. The AudioOffset task incorrectly identified `audio_offset.rs` as containing the ts-rs-annotated struct; the actual type lives in `lib.rs`. This is the second time a task spec has misdescribed file locations (the first was the TASK-3 `get_streams` wrapper being in `lib.rs` rather than `extract.rs`). Both cases involve the same architectural pattern: commands and IPC types that appear to belong in a module file are actually defined in `lib.rs`. Task specs for future `lib.rs` changes should be written with this in mind.

The **CI two-step failure pattern** has not recurred in any of the last 10 CI runs. All green on first push since Run 5.

BC-001 and BC-002 remain listed as active in KNOWN-BUG-CLASSES with no updates.

## CI health

`main` is green as of `d59594c`. Ten consecutive successful CI runs. No failures, no flakiness. Run times 1–2 minutes consistently.

## Observer notes

**Run 7 flagged AudioOffset as the named next target — it landed cleanly.** The deferred list from the post-audit close now has two items remaining (Windows non-C drive, GHA injection) out of the original six. Resolution rate: 4/6.

**SESSION-STATUS partially stale.** The `## Mode` line and `## Known Risks` AudioOffset entry need cleanup. Not urgent — they're cosmetic inaccuracies, not misleading about project direction.

**Git note protocol remains cold.** No recent commit carries structured notes. The protocol gap persists across every sprint since the Blender backend commit.

**`lib.rs` as the IPC boundary hub.** The pattern of IPC types and command wrappers living in `lib.rs` rather than module files has now caused task spec errors twice. This is a structural property of the codebase worth noting: `lib.rs` is load-bearing for ts-rs codegen, command registration, and dispatch — it's not just a thin entry point.
