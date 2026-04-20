# TASK-5a — Extract UpdateManager, PresetManager, CropEditor from App.svelte

**Size:** Medium. One session, ~4 hours.
**Prerequisites:** None beyond familiarity with Svelte 5 runes.
**Dependencies:** Should run **before** TASK-5b and TASK-5c. Those tasks depend on the `$bindable` patterns established here.

---

## Project context

**Fade** — desktop media converter (images, video, audio, data formats). **Tauri 2 + Svelte 5 + Rust** at `/Users/eldo/Downloads/Github/Fade-App`.

### How to run
```bash
cd /Users/eldo/Downloads/Github/Fade-App
npm install                  # if fresh checkout
npm run tauri dev            # native dev (port 1427)
npm test                     # vitest (frontend tests)
```

### Svelte 5 runes — this file uses ONLY runes, no legacy syntax

| Legacy | Svelte 5 rune |
|--------|---------------|
| `export let foo` | `let { foo } = $props()` |
| `export let foo = 0` | `let { foo = 0 } = $props()` |
| `$: derived = foo * 2` | `const derived = $derived(foo * 2)` |
| `let foo = 0` (reactive) | `let foo = $state(0)` |
| `onMount(() => ...)` | `$effect(() => ...)` |
| Two-way bind prop | `let { foo = $bindable() } = $props()` |

Testing infrastructure: `src/tests/` uses `vitest` with `mount`/`unmount` from `svelte`. Tauri APIs are pre-mocked in `src/tests/setup.js` via `vi.mock`.

### Commit protocol (non-negotiable)

Every commit in this repo needs a git note:
```bash
git notes add -m "app: fade
state: active | stable | fragile
context: <what you did in one sentence>
deferred: <what you left incomplete, or 'none'>
fragile: <what's nearby that could break, or 'none'>
ci: green | red | unknown" HEAD
git push origin main
git push origin refs/notes/commits
```

## The problem

`src/App.svelte` is **6,014 lines**. It mixes queue management, operations, analysis tools, crop, preset management, and auto-updater into one file. This task extracts three low-risk, self-contained slices to establish the `$bindable` pattern for subsequent tasks.

## Scope — extract exactly 3 components

All 3 live under `src/lib/`. Create:

### 1. `src/lib/UpdateManager.svelte`

**Source lines in App.svelte:** approx **lines 280–390, 316–349, 386–440, and the template region ~4500–4570** (the "update available / downloading / ready to restart" UI block in the header).

**State moved in:**
- `updateState` (`'idle' | 'available' | 'downloading' | 'ready'`)
- `updateVersion`
- `updateProgress`
- `_pendingUpdate`
- `appVersion`
- `diagnosticsExpanded`

**Functions moved in:**
- `openReleasesPage()`, `_uploadBeforeUpdate()` (lines 292–314)
- `checkForUpdate()`, `maybeCheckForUpdate()` (lines 326–349)
- `installUpdate()` (lines 351–384)
- `restartNow()` (lines 386–395)

**Props API (final shape):**
```svelte
<script>
  let {
    settings,                     // store from settings.svelte.js — read/write
    setStatus,                    // callback: (msg, kind?) => void
    onUploadBeforeUpdate = null,  // optional callback; if present, awaited
  } = $props();
</script>
```

The component owns its own state; App.svelte no longer holds `updateState`, `updateVersion`, etc.

**Template to extract:** the update banner / progress UI. Read App.svelte to find the `<div>` block that switches on `updateState`. Move verbatim.

### 2. `src/lib/PresetManager.svelte`

**Source lines in App.svelte:** approx **2200–2366** (the `BUILTIN_PRESETS` constant + the four functions below).

**State moved in:**
- `presets` (user-saved presets loaded from backend)
- `headerPresetName`, `headerPresetId`, `headerAdding`, `_hpSuppressReset`
- The `BUILTIN_PRESETS` constant and `ALL_BUILTINS` derivation

**Functions moved in:**
- `loadPresets()` (2307)
- `applyPreset(id)` (2311)
- `saveHeaderPreset()` (2334)
- `deletePreset(id)` (2357)

**Props API:**
```svelte
<script>
  let {
    imageOptions = $bindable(),
    videoOptions = $bindable(),
    audioOptions = $bindable(),
    globalOutputFormat = $bindable(),
    activeOutputCategory,
    setStatus,
  } = $props();
</script>
```

Apply/save preset mutates the bound options objects; parent sees the change reactively.

**Template to extract:** the preset dropdown / save row in the header. Read App.svelte for a block containing `headerPresetName` / `saveHeaderPreset` bindings.

### 3. `src/lib/CropEditor.svelte`

**Source lines in App.svelte:** approx **682–755** (script functions) plus the corresponding crop overlay markup in the image preview region (~4300–4400 range — read to find).

**State moved in:**
- `cropActive`, `cropRect`, `cropDrag`, `cropAspect`
- `imgNaturalW`, `imgNaturalH` (if used only for cropping — check; if used elsewhere, leave in parent and pass as props)

**Functions moved in:**
- `initCropRect(aspect)` (682)
- `activateCrop(aspect)` (697)
- `startCropDrag(e, type)`, `onCropDragMove(e)`, `onCropDragEnd()` (705–736)
- `applyCrop()`, `cancelCrop()`, `clearCropValues()` (738–754)
- `getImgBounds()` if only used by crop — check usages first
- The `CROP_MIN` constant

**Props API:**
```svelte
<script>
  let {
    imageOptions = $bindable(),   // crop apply writes crop_x/y/width/height here
    imgEl,                        // ref to the <img> element for bounds math
    imgNaturalW,
    imgNaturalH,
  } = $props();
</script>
```

`applyCrop()` writes to `imageOptions.crop_*`. Parent binds its `imageOptions` and sees updates.

**Template to extract:** the crop overlay (the `<div>` block with resize handles that renders over the preview image when `cropActive === true`).

## Out of scope — do NOT touch

- **Queue management** — `queue`, `selectedId`, `selectedItem`, selection handlers, `addFiles`, `removeItem`, `handleSelect` → TASK-5b
- **Any operation runner** (anything named `run*` other than `runDiffPreview`) → TASK-5c
- **Analysis tools** (loudness, VMAF, cut-detect, black-detect, etc.) → TASK-5c
- **Chroma key panel** → TASK-5c
- **Timeline.svelte / Queue.svelte** and other existing `lib/*.svelte` files
- **Rust backend** (`src-tauri/`)

## Procedure

1. **Read the existing lib components** to understand the style conventions:
   ```
   src/lib/ImageOptions.svelte
   src/lib/VideoOptions.svelte
   src/lib/Queue.svelte
   src/lib/AudioOptions.svelte
   ```
   Confirm: they use `$props()`, `$bindable()`, and `$state()`. Match that style.

2. **For each of the 3 components, in this order: UpdateManager → PresetManager → CropEditor:**

   a. Read the relevant script + template lines in `App.svelte`.
   b. Create the new component file under `src/lib/<Name>.svelte`.
   c. Move the script logic in, adapting to `$props()` + `$bindable()` pattern.
   d. Move the template markup in.
   e. In `App.svelte`, delete the moved script lines and replace the template block with `<Name {...props} />`.
   f. Add the import at the top of App.svelte.
   g. Run `npm test` — must pass.
   h. Run `npm run tauri dev` briefly to visually confirm the feature still works (settings → check for updates; header preset dropdown; image crop). You may skip this visual check IF you wrote tests covering the behavior (preferred).

3. **One commit per component extraction.** Three commits total. Each commit message:
   ```
   refactor(ui): extract <Name> from App.svelte

   Moves <N> state declarations and <M> functions into a self-contained
   component. Behavior unchanged.
   ```

4. **Git notes after each commit** (see protocol above).

5. **Push after each commit** so CI catches breakage incrementally, not at the end.

## Verification — definition of done

- [ ] Three new files: `src/lib/UpdateManager.svelte`, `src/lib/PresetManager.svelte`, `src/lib/CropEditor.svelte`
- [ ] App.svelte line count dropped meaningfully (expect ~500–700 lines removed total)
- [ ] App.svelte imports each of the 3 new components
- [ ] `npm test` passes
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes (sanity — no Rust changes expected)
- [ ] Dev run launches successfully and the three features work: updater banner in settings, preset dropdown in header, image crop overlay
- [ ] 3 commits, 3 git notes, all pushed
- [ ] `gh run list --limit 1` shows green CI

## Gotchas

- **DOM refs across component boundaries.** `CropEditor` needs a ref to the `<img>` element to compute bounds. Pass it as a prop (`imgEl`), don't try to query the DOM from inside the child. If the parent holds the `<img>`, pass the ref. If the child owns the `<img>`, move it into the child.

- **`$bindable()` is two-way.** Any prop marked `$bindable()` lets the child write back to the parent's value. Use this for `imageOptions`, `videoOptions`, etc. — do NOT copy-clone and try to emit events.

- **Runes do not work in `.js` files** — only in `.svelte` files. If you find logic you want to extract into a helper module, use plain JS without runes; move the rune-dependent code into the `.svelte` file.

- **Do not regress the prop-drilling depth.** If a value only exists for one component, move it in. If it's shared (like `imageOptions`), bind it.

- **Svelte 5 does not allow `$state` in module scope inside a `.svelte` file.** `$state` declarations must be inside `<script>` at component body level.

- **Preset selection syncs `globalOutputFormat`.** Keep that two-way binding intact — apply preset writes `globalOutputFormat = p.output_format;`, which the parent's format picker reads. Mark it `$bindable()`.

- **`_hpSuppressReset` uses `queueMicrotask`** to defer re-enabling after a reactive write. Preserve this pattern — it prevents a render loop.

- **Tests are lightweight** — the existing suite just mounts the App component. If your extraction breaks it, check that all Tauri commands mentioned in your new components are mocked in `src/tests/setup.js`. Add mocks if needed.

- **Do NOT run a release.** Do not bump `src-tauri/tauri.conf.json`, do not tag. Commit and push to main only.

## If you get stuck

- If a function you're moving turns out to reference state from a region you're not extracting, **do not widen scope**. Pass that state in as a prop, or leave the function in App.svelte for TASK-5b / TASK-5c to handle.

- If a template block you're moving has markup interleaved with the queue or operation panel that you can't cleanly separate, halt and report. Do not produce a half-extracted component.

- If tests start failing with errors unrelated to your changes (e.g., pre-existing breakage), halt and report. Do not fix unrelated bugs in this session.

## Reporting

When done, report:
- Line count of App.svelte before and after
- Line counts of the three new components
- CI run URL
- Any deviations from the proposed props API (and why)
