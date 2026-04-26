# TASK-3: Fix analysis-result listener registration race with invoke ordering

## Goal
In `invokeAnalysis` in `AnalysisTools.svelte` and in the `preview_diff` call in `App.svelte`, the `invoke(...)` call runs only after the `listen(...)` promise resolves — guaranteeing the event listener is registered before the backend command can emit a result.

## Context
The async IPC pattern for analysis commands is:
1. Call `listen(`analysis-result:${jobId}`, handler)` — registers a Tauri event listener (async)
2. Call `invoke(command, ...)` — sends the command to the backend which will emit on that channel

The race: `listen()` is called and immediately `invoke()` is called synchronously without waiting for the listen to resolve. If the backend is very fast and emits `analysis-result:${jobId}` before Tauri's JS-side listener is registered (the micro-task gap between starting `listen()` and its `.then()` running), the event is lost and the Promise never resolves or rejects. The `settled` guard added in `.then()` handles the case where the event fires AFTER `listen()` starts but BEFORE `.then()` assigns `unlistenFn` — but it does NOT handle the event firing before the IPC listener is established in Tauri at all.

The fix: move `invoke(...)` inside the `listen(...).then(fn => ...)` callback, called only after `unlistenFn` is assigned. This ensures Tauri has registered the listener before the backend command is dispatched.

The pattern currently looks like:
```javascript
listen(`analysis-result:${jobId}`, handler).then((fn) => {
  if (settled) { fn(); return; }
  unlistenFn = fn;
}).catch((err) => settle(reject, err));

invoke(command, { jobId, ...params }).catch((err) => settle(reject, err));  // ← move this inside .then()
```

The fix:
```javascript
listen(`analysis-result:${jobId}`, handler).then((fn) => {
  if (settled) { fn(); return; }
  unlistenFn = fn;
  invoke(command, { jobId, ...params }).catch((err) => settle(reject, err));  // ← moved here
}).catch((err) => settle(reject, err));
```

This pattern appears in two places:
1. `src/lib/AnalysisTools.svelte` — `invokeAnalysis()` function (line ~97–110)
2. `src/App.svelte` — `preview_diff` call site (line ~485–501)

Relevant files:
- `src/lib/AnalysisTools.svelte` — `invokeAnalysis()` function
- `src/App.svelte` — the `preview_diff` async block

## In scope
- `src/lib/AnalysisTools.svelte` — fix the invoke ordering in `invokeAnalysis()`
- `src/App.svelte` — fix the invoke ordering in the `preview_diff` block

## Out of scope
- Any change to the Rust backend (`src-tauri/`)
- Any change to other Svelte components
- Refactoring the overall Promise structure (only reorder the `invoke` call)
- Adding retry or timeout logic

## Steps
1. Read `src/lib/AnalysisTools.svelte` lines 76–112. Find the `new Promise(...)` block in `invokeAnalysis()`. Locate the `listen(...)` chain and the `invoke(...)` call.
2. Move the `invoke(command, { jobId, ...params }).catch((err) => settle(reject, err));` line from its current position (after the `listen(...).then(...).catch(...)` chain) to inside the `.then((fn) => { ... })` callback, AFTER the `unlistenFn = fn;` assignment and the `if (settled)` guard. The `if (settled)` guard must remain first — if the promise is already settled, call `fn()` and return without invoking.
3. The resulting structure in `invokeAnalysis()` should be:
   ```
   listen(..., handler).then((fn) => {
     if (settled) { fn(); return; }
     unlistenFn = fn;
     invoke(command, params).catch((err) => settle(reject, err));
   }).catch((err) => settle(reject, err));
   // NO invoke() here after the chain
   ```
4. Read `src/App.svelte` lines 472–510. Find the `preview_diff` Promise block with the same listen/invoke pattern.
5. Apply the same fix: move `invoke('preview_diff', {...})` inside the `.then((fn) => { ... })` callback, after `unlistenFn = fn`.
6. Run `npm run check` (Svelte typecheck) if it exists in `package.json`. Must exit 0.

## Success signal
- In `src/lib/AnalysisTools.svelte`: `invoke` appears INSIDE the `.then((fn) => { ... })` block, not after the `listen(...)` chain.
- In `src/App.svelte`: same structure for `preview_diff`.
- `npm run check` exits 0 (if available).
- No other changes to either file beyond the reordering.

## Notes
- The `settled` guard is critical: if the settle function was called during the `listen()` setup (e.g., a cancellation or error), the listener must be cleaned up with `fn()` and `invoke` must NOT be called. The existing guard handles this — ensure it remains in place.
- After the fix, the `invoke` error handler (`.catch((err) => settle(reject, err))`) must still be present — just moved inside `.then()`.
- No Rust changes needed. No test changes needed — this is a structural fix to async ordering.
- The race is not yet observed in production; this is a preventive fix based on structural analysis.
