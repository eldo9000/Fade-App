# Fade-App Observer State
Last updated: 2026-04-23  ·  Run 9

---

## Active development areas

The project has reached a clean state milestone. The post-audit hygiene sprint closed all 6 deferred items from the B1–B18 audit cycle in rapid succession: AudioOffset i64→i32 (`3fd9750`), Windows non-C drive assetProtocol (`d705d52`), overlay shim removal (`1b229a9`), KNOWN-BUG-CLASSES documentation (`8bfd744`), `$bindable` chain verification (`1ce6a1b`), and GHA shell injection hardening (`eceedb1`). SESSION-STATUS now reflects the milestone with no open deferred items. No feature work is currently in flight.

## Fragile / high-risk areas

The **Blender backend path resolution** fragility persists. The code bugs (BC-003 `as_background_job`, BC-004 empty-scene check) are confirmed resolved in `blender_convert.py`, but the runtime path resolution for `blender_convert.py` itself — how headless Blender finds the script at runtime in non-dev deployment contexts — remains unaddressed and is noted in SESSION-STATUS Known Risks. The git note on `9b13f41` explicitly flags this as `fragile: blender_convert.py script path resolution at runtime`.

The **analysis-result one-shot listener race** persists. The one-shot event listener introduced in TASKs 4–5 is set up before the invoke call; a very fast completion could fire the event before `unlisten` is registered. Structurally possible, not observed in practice.

## Deferred work accumulation

The formal post-audit deferred list is exhausted. Three informal items remain off the ledger: STEP/IGES format support (deferred in `9b13f41` git note — requires FreeCAD or different toolchain), the `librewin_common` superset-vs-authoritative strategy, and the `createLimiter` slot-leak watchdog (timeout was added; formal watchdog was not). None of these have been targeted or newly referenced in recent commits.

## Pattern watch

All four KNOWN-BUG-CLASSES entries are now marked resolved. BC-001 (inverted chevron SVG) and BC-002 (`_connectSource` on viz expand) were confirmed fixed in `Timeline.svelte` during the TASK-1 audit; both had been listed as active since Run 3. BC-003 and BC-004 were confirmed fixed in `blender_convert.py` and their entries updated.

The `lib.rs` IPC hub pattern — the source of two prior task spec errors (get_streams, AudioOffset) — did not surface in this sprint. The three hygiene tasks (TASK-1, TASK-2, TASK-3) all went green on first dispatch with no spec errors, suggesting the pattern is now understood.

The INVESTIGATION-LOG shows all entries CONFIRMED. No OPEN investigations.

## CI health

CI has been green on every push in this session: 4 consecutive successful runs today (8bfd744, 1ce6a1b, eceedb1, and 71cf26b in progress at observation time). The run for 71cf26b was in-progress when this observation began; the prior 4 were all `success` in 1–2 minutes. Thirteen or more consecutive green runs overall. No flakiness observed.

## Observer notes

**The post-audit deferred list is fully closed.** Run 7 had 2 items remaining; Run 8 had 2 items remaining (Windows drive, GHA injection); Run 9 marks them resolved. The project is in the cleanest documented state since the B1–B18 audit closed.

**SESSION-STATUS drift from Run 8 has been resolved.** The stale AudioOffset entry in Known Risks, the stale Mode line, and the stale deferred list have all been corrected.

**KNOWN-BUG-CLASSES is now fully current.** BC-001–BC-004 all resolved. This file had lagged since Run 3.

**Git note protocol remains cold.** No commit since `99144ed` (the Blender fmt fix) carries a structured note. All recent sprint commits — 8 or more — carry no notes. The protocol gap has persisted through every sprint since the Blender backend. The two commits that do have notes are from earlier arcs.

**$bindable chain topology clarified.** QueueManager does not render OperationsPanel or ChromaKeyPanel as children — they are siblings rendered directly by App.svelte. The chain is App.svelte → QueueManager and App.svelte → OperationsPanel / ChromaKeyPanel independently, with `bind:selectedItem` at all three call sites.
