# Known Bug Classes — Fade App

## BC-001: Inverted toggle-state SVG icons
**First observed:** 2026-04-17
**File:** `src/lib/Timeline.svelte`
**Description:** SVG chevron paths assigned to the wrong `{#if}` / `{:else}` branch, causing the icon to show the opposite direction from what the current state requires. When `vizExpanded=true` the chevrons pointed down (suggesting the panel would expand below) instead of up (suggesting it can be collapsed). Swap the `<path d="...">` values between the two branches to fix.
**Pattern:** Any boolean-toggled SVG where paths are authored in `{#if open}` / `{:else}` blocks — verify each branch's visual meaning matches the state label.

---

## BC-003: librewin-common pinned past latest Libre-Apps tag
**First observed:** 2026-04-20
**File:** `src-tauri/Cargo.toml`
**Description:** `librewin-common` is pinned to git rev `d55e89df`, which is newer than the latest tag `v0.1.2` in eldo9000/Libre-Apps. Until a new tag is cut in Libre-Apps, this dep cannot move to a semver pin and will stay opaque. No runtime impact; affects maintainability.
**Fix:** Cut a new tag in Libre-Apps at or past `d55e89df`, then switch the Cargo.toml dep to `tag = "vX.Y.Z"`.

---

## BC-002: Audio analysers black / silent before first playback
**First observed:** 2026-04-17
**File:** `src/lib/Timeline.svelte`
**Description:** `_connectSource()` is only called during `togglePlay()` and the media-load effect. When the visualiser panel is expanded before the user has started playback, `_srcConnected` remains `false`, all three draw functions (`_drawOscilloscope`, `_drawSpectrum`, `_drawLissajous`) bail early on the `!_srcConnected` guard, and canvases show only black. Fix: add a reactive `$effect` that calls `_connectSource()` whenever `vizExpanded && mediaReady && audioEl && _audioCtx && !_srcConnected`.
**Pattern:** Any Web Audio graph that guards draw calls on `_srcConnected` (or equivalent) — ensure the source is connected reactively whenever the relevant panel becomes visible, not only on playback start.
