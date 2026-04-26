# TASK-1: Remove stale analysis-result race entry from SESSION-STATUS Known Risks

## Goal
SESSION-STATUS.md Known Risks section no longer mentions the "analysis-result one-shot listener race" — that entry was made stale by commit `ce5b37e` and has persisted uncorrected for two observer runs.

## Context
The SESSION-STATUS.md Known Risks section contains this entry:

> **analysis-result one-shot listener race.** The one-shot event listener introduced in async IPC migration is set up before the invoke call; if the event fires before `unlisten` is registered on a very fast completion, the result may be missed. Structurally possible, not yet observed.

Commit `ce5b37e` (`fix(ipc): move invoke inside listen().then() to prevent analysis-result race`) moved the `invoke` call inside `listen().then()`, eliminating the timing window that made the race possible. The race is no longer "structurally possible" — it is structurally impossible. The SESSION-STATUS "Next action" block already acknowledges the fix ("analysis-result invoke moved inside listen().then() to close listener race"), but the Known Risks prose was not updated.

This is a documentation gap only. No code changes are needed.

## In scope
- `SESSION-STATUS.md` — Known Risks section only

## Out of scope
- Any Rust source file
- Any Svelte file
- INVESTIGATION-LOG.md, OBSERVER-STATE.md, CLAUDE.md, ARCHITECTURE.md
- The Blender runtime fragility entry in Known Risks (that is still valid; do not touch it)

## Steps
1. Read `SESSION-STATUS.md` in full.
2. Locate the `## Known Risks` section.
3. Remove the "analysis-result one-shot listener race" bullet entirely.
4. Verify the remaining Known Risks entries are unchanged: `$bindable` chain verified correct, and Blender backend path fragility.
5. Commit the change.

## Success signal
`SESSION-STATUS.md` Known Risks section contains exactly two entries: the `$bindable` chain note and the Blender path fragility note. The analysis-result race entry is gone. `git diff SESSION-STATUS.md` shows only the removed lines.

## Notes
Do not rewrite or summarize the remaining entries — change only what must change. The Blender fragility note should remain word-for-word. The commit should be:
`docs(status): remove stale analysis-result race from Known Risks`
