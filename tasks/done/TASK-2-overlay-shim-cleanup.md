# TASK-2: Audit and remove dead overlay back-compat shim

## Goal
`src/lib/stores/overlay.svelte.js` has its back-compat shim (`overlay.show`/`overlay.hide` aliases) removed, and the two live callers in `src/App.svelte` (lines 3257 and 3272) are updated to call `hideOverlay()` directly.

## Context
During the VideoOptions overhaul (commit `4bd1839`), a portal-style overlay store was added to `src/lib/stores/overlay.svelte.js`. The store exposes `showOverlay`/`hideOverlay` as its canonical API. At some point the API was renamed mid-work, and a back-compat shim was added: `overlay.show` and `overlay.hide` aliased to the canonical functions.

The shim comment says "in case callers still use the old shape" — meaning it was added defensively, not because any caller was known to require it at write time. OBSERVER-STATE.md Run 5 and Run 6 flagged this shim as potentially dead weight: if no callers reference `.show`/`.hide`, the shim adds confusion about the canonical API shape and should be removed.

**What the shim looks like (approximate):**
Inside `overlay.svelte.js`, something like:
```js
overlay.show = showOverlay;   // back-compat alias
overlay.hide = hideOverlay;   // back-compat alias
```
or an exported object with `show`/`hide` properties pointing to the canonical functions.

**How to check for live callers:**
Search the entire `src/` tree for `.show(` and `.hide(` on the overlay store object, and for any import of `overlay` that then accesses `.show` or `.hide`. The store is imported as `overlay` (from `$lib/stores/overlay.svelte.js`).

A prior audit run confirmed the only live callers are `overlay.hide()` at `src/App.svelte:3257` and `src/App.svelte:3272`. There are no `overlay.show()` callers anywhere in `src/`.

## In scope
- `src/lib/stores/overlay.svelte.js` — remove the shim lines
- `src/App.svelte` — update lines 3257 and 3272 from `overlay.hide()` to `hideOverlay()`

## Out of scope
- `src-tauri/` — Rust files are irrelevant
- Any other store or frontend files not listed above

## Steps
1. Read `src/lib/stores/overlay.svelte.js` in full to identify the exact shim lines.
2. Read `src/App.svelte` around lines 3250–3280 to see the exact call sites and confirm `hideOverlay` is already imported or available in scope.
3. In `src/App.svelte`: replace `overlay.hide()` with `hideOverlay()` at both lines 3257 and 3272. If `hideOverlay` is not yet imported in `App.svelte`, add it to the import from `$lib/stores/overlay.svelte.js`.
4. In `src/lib/stores/overlay.svelte.js`: remove the shim lines (the `overlay.show`/`overlay.hide` alias assignments). Do not change any other logic.
5. Verify `overlay.svelte.js` still exports `showOverlay` and `hideOverlay`.
6. Search `src/` to confirm no remaining `overlay.show` or `overlay.hide` references exist.

## Success signal
- `overlay.svelte.js` contains no `.show` or `.hide` alias assignments; `showOverlay` and `hideOverlay` are still exported.
- `src/App.svelte` at the former lines 3257 and 3272 calls `hideOverlay()`, not `overlay.hide()`.
- A grep for `overlay\.hide` and `overlay\.show` across `src/` returns no matches.

## Notes
The shim may be implemented as property assignment on an exported object, or as named re-exports, or as extra fields on a returned store object. Read the file first — don't assume the shape.
