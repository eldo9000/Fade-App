# Fade-App Observer State
Last updated: 2026-04-22  ·  Run 7

---

## Active development areas

A lightweight hygiene sprint followed the B16 phase 2 work. Two commits landed: SESSION-STATUS was refreshed (`ba2533e`) to reflect the completed sprint, correct the Next action to AudioOffset precision drift, and remove the two resolved Known Risks; and the overlay back-compat shim was migrated and removed (`1b229a9`), replacing the two `overlay.hide()` call sites in `App.svelte` with direct `hideOverlay()` calls and deleting the alias lines from `overlay.svelte.js`.

No feature work is in flight. The project is between sprints, with the AudioOffset micro-patch named as the next concrete target.

## Fragile / high-risk areas

The **Blender backend** fragilities (BC-003, BC-004) are unchanged from Run 6. `blender_convert.py` path resolution at runtime remains unmitigated; USD import empty-scene silent success remains unmitigated. Both items are in KNOWN-BUG-CLASSES but have no fix in progress.

The **`$bindable` chain** — App.svelte → QueueManager → OperationsPanel/ChromaKeyPanel mutating `selectedItem.status`/`.percent`/`.error` in place — persists. No work has touched this pathway in several sprints.

The **analysis-result one-shot listener race** from Run 6 persists. Structurally possible on very fast completions; not observed in practice.

The **overlay back-compat shim**, flagged as a risk in Run 5 and Run 6, is now resolved. The shim was live (two callers in `App.svelte`), not dead — callers were migrated to `hideOverlay()` before the shim was removed. Risk closed.

## Deferred work accumulation

Three items remain from the post-audit formal deferred list:

1. **AudioOffset i64→i32 precision drift** — named as Next action in SESSION-STATUS. `audio_offset.rs` declares `offset_ms` as `i64`; ts-rs generates `bigint`; frontend passes `number`. Correctness issue at the IPC boundary above 2^53ms (impractical values, but a type mismatch). Fix: change struct field to `i32`, regenerate ts-rs output, audit frontend callers.
2. **Windows non-C drive preview** — `assetProtocol.scope` does not cover secondary drives. Affects Windows users only. No work scheduled.
3. **GHA shell injection hardening** — `release.yml` interpolates `${{ inputs.tag }}` in `run:` steps. Not exploitable without repo write access. No work scheduled.

The `librewin_common` superset-vs-authoritative question and the STEP/IGES Blender deferral remain off the formal ledger. The VBR/CBR silent fallback for VP9/AV1 (mode picker has no effect on non-h264/h265 codecs) also remains untracked.

## Pattern watch

The **CI two-step failure pattern** has not recurred since Run 5. Five consecutive CI runs green with no failures, across both the B16 phase 2 sprint and this hygiene sprint. The dispatch workflow's local fmt + clippy enforcement appears to be holding.

The **missed-files pattern** did not surface this sprint — both tasks involved small, targeted changes with no incidental file modifications.

The **overlay shim resolution** is worth noting as a pattern instance: the shim was flagged defensively in Run 5 as "potentially dead," Run 6 confirmed it hadn't been cleaned up, and the audit this sprint found it was live (not dead). The corrective action was right but the framing was off — "dead shim" became "live caller migration." This is a minor example of observer framing diverging from implementation reality.

BC-001 (inverted SVG chevrons) and BC-002 (audio analysers black before first playback) remain listed in KNOWN-BUG-CLASSES as active. Run 3 noted a user-reported resolution for BC-001; no formal update has been made. These entries are stale.

## CI health

`main` is green as of `f64e93a`. Eight consecutive CI runs have completed with `success` across the last two sprints. Run times are consistent at 1–2 minutes. No failures, no flakiness observed.

## Observer notes

**Run 6 flagged items resolution:** SESSION-STATUS stale → resolved. Overlay shim → resolved (with caller migration, not dead-code removal). Both cleanups landed in a single lightweight sprint session. The deferred items and Blender fragility are unchanged.

**Git note protocol remains cold.** Neither `ba2533e` nor `1b229a9` carries structured notes. The last commit with structured notes remains `99144ed` (fmt fix, several sessions ago). The `deferred:` and `fragile:` fields that exist on `9b13f41` (Blender backend) represent the high-water mark for protocol adherence. No recent commit comes close.

**CONDUCTOR-LOG.md in repo root.** Flagged in Run 6 — still present. No move to `docs/` or similar has occurred. Minor cosmetic issue, not a correctness risk.

**AudioOffset is the named next target.** It is small, self-contained, and correctness-relevant. The ts-rs regeneration step requires `cargo build` or `cargo test` to trigger the type export. Frontend callers should be audited before the struct field type is changed, not after — changing from `i64` to `i32` on the Rust side regenerates the TypeScript type from `bigint` to `number`, which is the desired outcome, but any frontend code accidentally passing large values would break silently at runtime rather than at compile time.
