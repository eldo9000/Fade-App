# TASK-2: Audit and document the $bindable mutation chain

## Goal
The `selectedItem` mutation chain from `App.svelte` → `QueueManager` → `OperationsPanel` / `ChromaKeyPanel` is verified to use `$bindable()` + `bind:` correctly at every link, and `SESSION-STATUS.md` no longer lists the chain as an open risk.

## Context
Fade passes `selectedItem` (a `$state(null)` reactive object in `App.svelte`) down to child components that mutate its `.status`, `.percent`, and `.error` properties during job execution. An OBSERVER note flagged this as a "fragile `$bindable` chain" risk: if any link in the chain accidentally dropped the `bind:` or removed `$bindable()`, progress updates would silently stop propagating.

A pre-task audit has already confirmed the following:
- `OperationsPanel.svelte`: declares `selectedItem = $bindable(null)` in `$props()` ✓
- `ChromaKeyPanel.svelte`: declares `selectedItem = $bindable(null)` in `$props()` ✓
- `QueueManager.svelte`: declares `selectedItem = $bindable(null)` in `$props()` ✓
- `App.svelte`: passes `bind:selectedItem` to `QueueManager` (line ~1737) ✓
- `App.svelte`: passes `bind:selectedItem` to `OperationsPanel` and `ChromaKeyPanel` (line ~2366) ✓

The chain is correctly implemented. This task is to verify that assessment end-to-end and close the risk entry in `SESSION-STATUS.md`.

**Key thing to verify:** `QueueManager.svelte` is the parent of `OperationsPanel` and `ChromaKeyPanel` OR they are siblings under `App.svelte`. If QueueManager renders them as children, confirm it also passes `bind:selectedItem` to them. If they are rendered directly by `App.svelte`, the chain is already complete as audited.

## In scope
- `App.svelte` — read-only audit of `bind:selectedItem` call sites
- `src/lib/QueueManager.svelte` — read-only; does it render OperationsPanel/ChromaKeyPanel?
- `src/lib/OperationsPanel.svelte` — read-only; confirm `$bindable()` on selectedItem
- `src/lib/ChromaKeyPanel.svelte` — read-only; confirm `$bindable()` on selectedItem
- `SESSION-STATUS.md` — remove the `$bindable` chain risk entry if audit is clean

## Out of scope
- Any source code changes to `.svelte` files — audit only, no edits
- `OBSERVER-STATE.md` — do not touch
- Any other files

## Steps
1. Read `src/lib/QueueManager.svelte` — check whether it renders `OperationsPanel` or `ChromaKeyPanel` as children. If it does, verify it passes `bind:selectedItem` to them.
2. Read `src/lib/OperationsPanel.svelte` lines 1–30 — confirm `selectedItem = $bindable(null)` is in `$props()`.
3. Read `src/lib/ChromaKeyPanel.svelte` lines 1–20 — confirm `selectedItem = $bindable(null)` is in `$props()`.
4. Search `src/App.svelte` for all `bind:selectedItem` occurrences — confirm each component that mutates `selectedItem` is called with `bind:`.
5. If the chain is complete and correct: edit `SESSION-STATUS.md` to remove the `$bindable chain` entry from `## Known Risks`. Replace with a one-line note: "**`$bindable` chain verified correct** — all mutation paths use `$bindable()` + `bind:` explicitly."
6. If any link is missing `bind:` or `$bindable()`: do NOT edit SESSION-STATUS; return red with the specific gap found.

## Success signal
- Audit confirms `$bindable()` declared in all three child components, `bind:selectedItem` present at every call site.
- `SESSION-STATUS.md` `## Known Risks` no longer lists the `$bindable` chain as an open risk.
- No `.svelte` files were changed.
