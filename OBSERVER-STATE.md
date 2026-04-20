# Fade-App Observer State
Last updated: 2026-04-20  ·  Run 1

---

## Active development areas

Fade is in pre-1.0 polish. The last ten commits are weighted toward UX refinement and release hygiene rather than feature addition: sidebar search, segmented-button styling, settings-panel click-through fix, dev-feature gating, macOS Gatekeeper ad-hoc signing. One larger feature window closed recently — the v0.6.0 cycle bundled chromakey/colorkey/HSV keying, professional codec presets, the smart transcoder, SQLite/parquet/ipynb conversion, tracker module rendering (fluidsynth/openmpt123), and Premiere XML timeline routing via otioconvert. That was the last feature-heavy window; everything since has been stabilization.

## Fragile / high-risk areas

The release pipeline is the one live fragility. Tag `v0.6.1` failed the Release workflow because `src-tauri/tauri.conf.json` still reads `0.6.0`; the workflow has a strict tag-vs-config version check and aborts before building. This has not been repaired — either a re-tag at 0.6.1 after bumping the config, or a fresh 0.6.2, is required. Until this is done no binaries are shipping.

INVESTIGATION-LOG shows one recent arc (2026-04-17) that resolved cleanly — viz chevrons and `_connectSource` on expand, both CONFIRMED green via dispatch. No OPEN findings. The log is current.

## Deferred work accumulation

Cannot measure yet. Git notes protocol was added to this repo on 2026-04-20 and no commits have been made since, so no commit carries a structured `deferred:` field. This section will become meaningful after ~10 commits written under the new protocol.

## Pattern watch

Two bug classes catalogued in `KNOWN-BUG-CLASSES.md`: BC-001 (inverted toggle-state SVG icons) and BC-002 (audio analysers black/silent before first playback). Neither has surfaced in recent commits. BC-001 overlaps conceptually with the 2026-04-17 viz-chevrons inversion finding in INVESTIGATION-LOG — worth watching whether chevron-direction and toggle-state icon bugs share a root cause that hasn't been named yet.

## CI health

Main branch CI is green (run 24648547365, 2026-04-20 04:32, 49s). Release workflow is red on v0.6.1 (run 24648281064, 2026-04-20 04:21) — the tag-vs-config version check failed. The main CI and the release CI are separate pipelines; main has been healthy. A background concern: GitHub deprecation notice flagged `actions/checkout@v4` for Node 20, with Node 24 becoming the default on 2026-06-02. Not blocking.

## Observer notes

**Run 1 — loop just instantiated.** Git notes protocol was added to Fade's CLAUDE.md today; no commits yet carry notes. The `deferred:` and `fragile:` fields in commit metadata are the richest signal the observer reads, so this state file will be thin until the protocol runs for ~10 commits. Re-read in context.

The release failure is the highest-signal observation available right now. It is a concrete, mechanically-diagnosable blocker with a clear fix path, and it is holding up distribution of recently-shipped features. SESSION-STATUS's Next action correctly names it.

Fade is at the stage where the Observer Loop is at its most useful: feature surface is complete, the remaining work is polish and release hygiene, and the observer's job is to watch for regressions, accumulating deferrals, and release-pipeline brittleness as the project approaches 1.0. This is the test case the loop was designed for.
