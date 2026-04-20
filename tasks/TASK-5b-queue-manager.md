# TASK-5b — Extract QueueManager from App.svelte

**Size:** Medium. One session, ~4–5 hours.
**Prerequisites:** **TASK-5a must be complete and merged to main.** This task inherits the `$bindable` pattern established there.
**Dependencies:** Must run **before** TASK-5c. Operation runners in 5c read `selectedItem` and `queue`, which this task restructures.

---

## Project context

**Fade** — desktop media converter built with **Tauri 2 + Svelte 5 + Rust**. Repo at `/Users/eldo/Downloads/Github/Fade-App`.

### How to run
```bash
cd /Users/eldo/Downloads/Github/Fade-App
npm install
npm run tauri dev            # port 1427
npm test                     # vitest
```

### Svelte 5 runes only — no legacy syntax
- `export let foo` → `let { foo } = $props()`
- Two-way prop → `let { foo = $bindable() } = $props()`
- `onMount(...)` → `$effect(() => ...)`
- `let x = 0` (reactive) → `let x = $state(0)`

### Commit protocol — required

Every commit gets a git note:
```bash
git notes add -m "app: fade
state: active | stable | fragile
context: <one sentence>
deferred: <one sentence or 'none'>
fragile: <one sentence or 'none'>
ci: green | red | unknown" HEAD
git push origin main
git push origin refs/notes/commits
```

## Verify prerequisite

Before starting, confirm TASK-5a landed:
```bash
git log --oneline -10
```
You should see commits referencing `UpdateManager`, `PresetManager`, `CropEditor`. If not, stop — this task assumes those extractions are in place.

Check App.svelte size:
```bash
wc -l src/App.svelte
```
Expect it to be in the 5,300–5,500 range (down from 6,014).

## The problem

App.svelte still holds all queue logic: the array itself, selection state, multi-select shift/ctrl logic, drag-and-drop, folder grouping, pause/cancel wiring, and the async preview pipeline that runs when selection changes. This task extracts it.

## Scope — extract ONE component

### `src/lib/QueueManager.svelte`

This is a **controller** component — most of its "output" is state it owns and exposes as `$bindable` props, not visible UI. The existing `src/lib/Queue.svelte` already owns the visual queue list; QueueManager sits between App.svelte and Queue.svelte, holding the state.

**Source lines in App.svelte to move:**

**State (approximate locations, verify by search):**
- `queue = $state([])` and related derived filters (`visibleQueue`)
- `selectedId`, `selectedIds`, `selectAnchorId`, `lastSelectedId`
- `draggingFileId`, `folderDropHover`, folder expand map
- `_loadGen` (generation counter for pipeline cancellation)
- Cached waveform / filmstrip maps

**Functions (approx lines; verify):**
- `addFiles(paths)` — ~1179
- `removeItem(id)` — ~1199
- `handleSelect(e, item, incompat?)` — ~2783–2852 (the big 69-line one)
- Multi-select helpers: `isSelected(id)`, `toggleSelect(id)`, etc.
- Folder ops: `toggleFolderExpanded(id)`, `moveItemToFolder(id, folderId)`, folder create/delete
- Drag handlers: `onRowDragStart`, `onRowDragOver`, `onRowDrop`
- `cancelJob(id)`, `cancelAll()`, `togglePause(id)` (the frontend wrappers; backend stays untouched)
- `runLoadPipeline(item)` — ~465–525 — async waveform/filmstrip preloading
- `_bgPreloadNext()` — background preloader

**Props API (final shape):**
```svelte
<script>
  let {
    // Ops that depend on the current selection read this bindably.
    // Keeping it bindable (rather than derived) lets operation runners
    // in App.svelte continue to mutate selectedItem.status / .percent / .error
    // during job execution without a re-derivation round-trip.
    selectedItem = $bindable(null),

    // Media type is derived from selectedItem but many call sites read it
    // directly; expose it to avoid scattering the lookup.
    selectedMediaType = $bindable(null),

    // The queue itself is exposed so Timeline / preview / ops can read it.
    queue = $bindable([]),

    // Visible queue filter comes out of QueueManager's derivation.
    // Parent reads this as plain prop.
    visibleQueue = null,

    setStatus,                  // (msg, kind?) => void
    outputDir,                  // path, passed to expectedOutputPath inside handleSelect
    outputSeparator,            // separator char from settings

    // Callbacks from parent
    onSelectionChange = null,   // (item) => void — optional; for side effects like resetting diff preview
  } = $props();
</script>
```

The component must expose state in a way App.svelte can read it. Because Svelte 5 runes don't allow arbitrary module-level exports, use `$bindable()` for values the parent needs, plus callback props for events.

**Template:** QueueManager renders `<Queue />` internally with the right bindings. App.svelte just renders `<QueueManager {...} />` where it used to render `<Queue />`.

## Out of scope — do NOT touch

- **Operations runners** (`runCut`, `runRewrap`, `runChromaKey`, `_runOp`, the 30+ operation handlers) → TASK-5c
- **Analysis tools** (loudness, VMAF, cut-detect) → TASK-5c
- **Chroma key preview** → TASK-5c
- **Timeline.svelte, Queue.svelte internals** — Queue.svelte stays intact; QueueManager wraps it
- **Crop, preset, updater** — already extracted in TASK-5a
- **Rust backend** — untouched
- **Window / tab / theme / status bar logic**

## The critical subtlety — async pipeline cancellation

`handleSelect` does more than flip `selectedId`. It also:

1. Increments `_loadGen` (generation counter).
2. Kicks off `runLoadPipeline(selectedItem)` which:
   - Loads file info via `invoke('get_file_info', ...)`
   - Decodes waveform (video/audio) via `invoke('get_waveform', ...)`
   - Generates filmstrip (video) via `invoke('get_filmstrip', ...)`
   - Yields to the browser between stages
   - Bails early if `_loadGen` changed during an await (user clicked another item)
3. Caches results so re-selecting an already-loaded item is instant.

**If you break this, preview will be wrong — waveforms will attach to the wrong file, or a slow decode will continue in the background and overwrite the new selection's data.**

Preserve the generation counter pattern. Do not "simplify" it. Write a vitest test that exercises fast consecutive selections and verifies `invoke('get_waveform', ...)` is called with the correct path after the burst settles.

## Procedure

1. **Spike first, commit second.** Read the following in App.svelte to build a mental model:
   - `$state` declarations around lines 90–300
   - `$derived` / `$derived.by` blocks
   - `handleSelect` (~2783–2852)
   - `runLoadPipeline` (~465–525)
   - `addFiles`, `removeItem`, folder helpers
   - All drag handlers (search for `onRowDrag`)

2. **Create the skeleton** of `src/lib/QueueManager.svelte` with the props API and empty function stubs.

3. **Move functions in one coherent batch:**
   - State + derivations first
   - Simple helpers (`isSelected`, folder toggles)
   - `addFiles`, `removeItem`
   - Drag handlers
   - `handleSelect` + `runLoadPipeline` **together** (they share the generation counter)
   - `cancelJob`, `cancelAll`, `togglePause`

4. **Wire App.svelte:**
   - Import `QueueManager`
   - Replace the `<Queue />` instantiation with `<QueueManager {...} />`
   - Bind `selectedItem`, `queue`, `selectedMediaType`
   - Pass `setStatus`, `outputDir`, `outputSeparator`
   - Remove the moved state + function declarations from App.svelte

5. **Write tests** in `src/tests/queue.test.js`:
   - Existing tests should still pass (don't break them)
   - Add: selecting an item updates `selectedItem`
   - Add: rapid reselection cancels the in-flight pipeline (mock `invoke` and assert only the final `get_waveform` call matters)
   - Add: `removeItem` removes from queue and if it was selected, advances selection

6. **Verify:**
   ```bash
   npm test
   cargo check --manifest-path src-tauri/Cargo.toml   # sanity — no Rust changes expected
   npm run tauri dev                                   # manually verify queue + selection + waveform
   ```

7. **Commit once** (this is one cohesive refactor):
   ```
   refactor(ui): extract QueueManager from App.svelte

   Owns queue state, selection (incl. multiselect and shift/ctrl ranges),
   folder grouping, drag handlers, and the async preview pipeline
   including its generation-counter cancellation.

   App.svelte is now ~N lines smaller. Behavior unchanged.
   ```

8. **Git note + push** (protocol above).

## Verification — definition of done

- [ ] New file `src/lib/QueueManager.svelte` exists and is imported by App.svelte
- [ ] App.svelte no longer declares `queue`, `selectedId`, or `handleSelect`
- [ ] `selectedItem` still reachable in App.svelte (bound from QueueManager) so operation runners can read/mutate it
- [ ] `runLoadPipeline` and `_loadGen` live in QueueManager; cancellation still works
- [ ] Multiselect (shift-click, ctrl-click) still works
- [ ] Folder expand/collapse + drag-to-folder still works
- [ ] `npm test` passes; new tests for selection + cancellation added
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes
- [ ] Dev run launches, files can be added, selected, played, cancelled
- [ ] 1 commit, git note attached, both pushed
- [ ] `gh run list --limit 1` shows green CI

## Gotchas

- **`selectedItem` must remain mutable from outside QueueManager.** Operation runners in App.svelte (TASK-5c will extract these later, but they still live in App.svelte now) mutate `selectedItem.status`, `.percent`, `.error` directly during a job. The `$bindable()` prop allows this. If you make it read-only (plain prop), operations will silently fail to update the UI.

- **Svelte 5 reactivity and objects.** Mutating `selectedItem.status = 'x'` works because `selectedItem` is a `$state` object. Do NOT clone or copy-on-write — that will break the binding chain.

- **`_loadGen` is just a number.** It does NOT need to be `$state`. It's written by `handleSelect` and read by the running `runLoadPipeline` closure. Plain `let _loadGen = 0` inside the component is fine.

- **The drag handlers use native drag-drop.** Tauri also has its own drag-drop for file imports (`@tauri-apps/api/dnd`). These are two different systems — the Tauri one fires when files come in from outside; the native one is for reordering inside the queue. Do not confuse them.

- **`visibleQueue` is a derivation** that collapses folders into a flat list when open, and shows only folder rows when closed. It must recompute when folder expand state changes. Use `$derived` inside QueueManager; expose the result as a plain prop (not `$bindable`).

- **`invoke()` calls in `runLoadPipeline` can reject.** The current code catches silently for waveform/filmstrip (those are best-effort caches) and surfaces to the status bar for `get_file_info`. Preserve the distinction.

- **Generation counter early-exit pattern:**
  ```js
  const gen = ++_loadGen;
  await somethingAsync();
  if (gen !== _loadGen) return;   // user clicked another item; bail
  ```
  This pattern is used multiple times inside `runLoadPipeline`. Every `await` must be followed by a gen check before mutating state. Keep all of them.

- **Do NOT run a release.** No version bumps, no tags.

## If you get stuck

- If you find `selectedItem` is being read from too many places in App.svelte and you can't cleanly extract without rewriting callers, halt and report. The remaining callers will be cleaned up in TASK-5c — your job is to move ownership to QueueManager while keeping the bindings compatible.

- If tests start hanging (likely: async pipeline never resolves because of a broken mock), check that `invoke` mocks in `src/tests/setup.js` cover `get_file_info`, `get_waveform`, `get_filmstrip`. Add if missing.

- If the multiselect logic in `handleSelect` seems unnecessarily complex, **do not simplify it**. The shift/ctrl range logic across expanded/collapsed folders is load-bearing UX. Move it as-is.

## Reporting

When done, report:
- Line count of App.svelte before and after this task
- Line count of QueueManager.svelte
- Number of new tests added
- CI run URL
- Any props API deviations (and why)
- Any functions you considered in scope but left in App.svelte (and why)
