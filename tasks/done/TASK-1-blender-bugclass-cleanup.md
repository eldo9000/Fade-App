# TASK-1: Close resolved KNOWN-BUG-CLASSES entries for BC-003 and BC-004

## Goal
`KNOWN-BUG-CLASSES.md` entries BC-003 and BC-004 are marked as resolved, and BC-001 and BC-002 are audited for accuracy.

## Context
`KNOWN-BUG-CLASSES.md` has four active entries. Two of them are already fixed in the codebase:

**BC-003: Alembic `as_background_job` silent failure** — The fix (always pass `as_background_job=False`) is already present in `scripts/blender_convert.py` at lines 48–50 (`alembic_import`) and 111–113 (`alembic_export`). The entry is stale.

**BC-004: USD import empty-scene silent success** — The fix (check `len(bpy.data.objects) == 0` after import) is already present in `scripts/blender_convert.py` at line 44. The entry is stale.

**BC-001: Inverted toggle-state SVG icons** — Observer Run 3 noted a user-reported resolution, but the entry was never updated. Status unknown — needs verification against current `src/lib/Timeline.svelte`.

**BC-002: Audio analysers black/silent before first playback** — Same situation as BC-001. Needs verification against current `src/lib/Timeline.svelte`.

The format for a resolved entry: append a `**Resolved:**` line with the commit and brief description of what fixed it.

## In scope
- `KNOWN-BUG-CLASSES.md` — update entries
- `scripts/blender_convert.py` — read-only; confirm the fixes are present before marking resolved
- `src/lib/Timeline.svelte` — read-only; check whether BC-001 and BC-002 are fixed

## Out of scope
- Any source code changes — this is documentation only
- `SESSION-STATUS.md`, `OBSERVER-STATE.md` — do not touch
- Any other files

## Steps
1. Read `scripts/blender_convert.py` around the `usd_import`, `alembic_import`, and `alembic_export` calls. Confirm `as_background_job=False` is present on both Alembic calls and `len(bpy.data.objects) == 0` check follows the USD import.
2. If both fixes are confirmed: append `**Resolved:** Fixed in commit \`9b13f41\` (Blender headless backend). \`as_background_job=False\` present on both alembic_import and alembic_export.` to the BC-003 entry. Append equivalent resolution note to BC-004.
3. Read `src/lib/Timeline.svelte`. Search for the SVG chevron logic (BC-001: `vizExpanded` toggled chevron paths) and the `_connectSource`/`_srcConnected` pattern (BC-002).
4. For BC-001: if the fix (correct `{#if}`/`{:else}` branch assignments) is in place, append `**Resolved:**` with the relevant commit if known, or "verified fixed as of current main."
5. For BC-002: if the `$effect` that calls `_connectSource()` when `vizExpanded && mediaReady && !_srcConnected` is in place, mark it resolved similarly. If it is NOT in place, leave the entry as-is and note in your return that BC-002 is still open.
6. Verify the file is valid Markdown.

## Success signal
- `KNOWN-BUG-CLASSES.md` BC-003 and BC-004 entries each have a `**Resolved:**` line.
- BC-001 and BC-002 entries are either marked resolved or confirmed still open.
- No source code files were changed.
