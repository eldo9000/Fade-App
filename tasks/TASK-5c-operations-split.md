# TASK-5c — Extract OperationsPanel, AnalysisTools, ChromaKeyPanel from App.svelte

**Size:** Large. One session, ~5 hours.
**Prerequisites:** **TASK-5a AND TASK-5b must be complete and merged to main.** This task relies on `QueueManager` owning `selectedItem` as a bindable, and on the `$bindable` patterns from TASK-5a.
**Dependencies:** Final task in the App.svelte split sprint.

---

## Project context

**Fade** — desktop media converter, **Tauri 2 + Svelte 5 + Rust**. Repo at `/Users/eldo/Downloads/Github/Fade-App`.

### How to run
```bash
cd /Users/eldo/Downloads/Github/Fade-App
npm install
npm run tauri dev            # port 1427
npm test                     # vitest
```

### Svelte 5 runes only
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

## Verify prerequisites

Before starting, confirm TASK-5a and TASK-5b landed:
```bash
git log --oneline -15
wc -l src/App.svelte
ls src/lib/
```
You should see:
- Commits for `UpdateManager`, `PresetManager`, `CropEditor`, `QueueManager`
- `src/lib/QueueManager.svelte` exists
- App.svelte is substantially smaller than the original 6,014 (expect 3,500–4,500 range)

If any of the above is missing, stop. This task is not runnable standalone.

## The problem

After 5a/5b, App.svelte still holds the largest chunk of logic: **30+ operation runners, 9 analysis tools, and the chroma key preview system.** They all share a pattern (`_runOp` wrapper → `invoke('run_operation', ...)` → status bar update) but differ enough that splitting them into cohesive groups is nontrivial.

## Scope — extract 3 components

### 1. `src/lib/OperationsPanel.svelte`

Bulk transforms: operations that produce a new output file.

**Functions to move (all live between ~1340 and ~2090 in the current App.svelte; line numbers will have shifted after 5a/5b — find by name):**

- `_runOp(...)` (the generic wrapper around `invoke('run_operation', ...)`) — ~1720 originally
- `runCut(...)`
- `runRewrap(...)`
- `runConform(...)`
- `runMerge(...)`
- `runRotate(...)`
- `runSpeed(...)`
- `runFade(...)`
- `runDenoise(...)`
- `runLoop(...)`
- `runSilenceRemove(...)`
- `runSilencePad(...)`
- `runExtract(...)`
- `runRemoveAudio(...)`
- `runGifify(...)`
- `runTranscode(...)` / smart transcoder entry
- All other `run*` functions EXCEPT `runDiffPreview`, the chroma preview generator, and analysis-tool runners (see below)

**State to move:**
- Rewrap options, conform options, cut params, silence params, speed param, fade params, merge params — any state consumed only by the op runners
- `selectedOperation` — which op is currently focused in the right panel
- `expectedOutputPath` helper if only called from ops (check; if used by TASK-5a-extracted preview, leave in App.svelte)

**Props API:**
```svelte
<script>
  let {
    selectedItem = $bindable(null),   // ops mutate .status / .percent / .error
    queue = $bindable([]),            // batch convert reads/writes
    videoOptions = $bindable(),
    audioOptions = $bindable(),
    imageOptions = $bindable(),
    dataOptions = $bindable(),
    globalOutputFormat,
    outputDir,
    outputSeparator,
    setStatus,
  } = $props();
</script>
```

**Template:** the right-side operations panel that switches markup based on `selectedOperation`. Extract the whole `<div class="operations-panel">` (or equivalent — read App.svelte).

### 2. `src/lib/AnalysisTools.svelte`

Analysis operations produce result objects, not output files. They render their results inline in the panel.

**Functions to move:**
- `runLoudness()` / `loudnessTarget` state + `loudnessResult` display
- `runAudioNorm()` / related state
- `runCutDetect()` / `cutDetectAlgo`, `cutDetectResults`
- `runBlackDetect()` / `blackDetectResults`
- `runVmaf()` / `vmafModel`, `vmafResult`
- `runFrameMd5()` / `framemd5Result`
- `runSubLint()` / `subLintResults`
- `runSubDiff()` / `subDiffResults`
- `runSubProbe()` / `subProbeResult`

**State to move:** all per-analyzer config + results pairs listed above.

**Props API:**
```svelte
<script>
  let {
    selectedItem,                     // read-only — analysis doesn't mutate input item beyond .status
    setStatus,
  } = $props();

  // Each analyzer owns its own config and result state internally.
  // Results are not exposed back to parent — they're rendered in this component.
</script>
```

If any analysis result is read by markup outside this component (e.g., loudness result rendered on the Timeline), pass it back as an event/callback. Verify by searching the template for `loudnessResult`, `vmafResult`, etc. before you start extracting.

**Template:** the analysis section of the right panel (or wherever the results are rendered).

### 3. `src/lib/ChromaKeyPanel.svelte`

Chroma / color keying has its own preview-generation loop — debounced, hash-keyed cache, PNG preview overlay.

**Functions to move:**
- `runChromaKey()`
- `generateChromaPreview()`
- `_chromaPreviewKeyOf()`
- `_chromaOutputMeta()`
- Debounce timer management (`_chromaPreviewTimer`)

**State to move:**
- `chromaAlgo`, `chromaColor`, `chromaSimilarity`, `chromaBlend`, `chromaDespill`
- `chromaPreviewUrl`, `chromaPreviewLoading`, `chromaPreviewKey`, `_chromaPreviewCache`

**Props API:**
```svelte
<script>
  let {
    selectedItem = $bindable(null),
    videoOptions,          // reads trim_start/end
    outputDir,
    outputSeparator,
    setStatus,
  } = $props();
</script>
```

The chroma preview overlay (`<img>` or `<video>` with the preview URL) goes inside this component.

## Out of scope — do NOT touch

- `runDiffPreview()` — stays in App.svelte (coupled to the main preview panel)
- Background preloader `_bgPreloadNext()` — already moved to QueueManager in TASK-5b
- Queue management — done in 5b
- Crop, preset, updater — done in 5a
- Timeline.svelte internals — leave alone
- Rust backend — untouched
- `startConvert()` (batch convert entry point) — **leave in App.svelte** unless you can extract it cleanly as part of OperationsPanel. It's the "Convert All" entry and has compatibility-filtering logic. If it's >50 lines and tangled with queue + output-format state, leave it.

## The critical subtlety — `selectedItem` mutation

Operation runners mutate `selectedItem` **during** a job:

```js
selectedItem.status = 'converting';
selectedItem.percent = 0;
selectedItem.error = null;
try {
  await invoke('run_operation', { ... });
} catch (err) {
  selectedItem.status = 'error';
  selectedItem.error = String(err);
}
```

Tauri also fires `job-progress` and `job-complete` events from Rust. The listener (in App.svelte or already in QueueManager — check) updates `.status` / `.percent` / `.error` on the matching queue item.

**For this to keep working across the component boundary:** `selectedItem` must be `$bindable()` in OperationsPanel and ChromaKeyPanel. The parent's `selectedItem` is itself bound from QueueManager. Bindings chain transparently — mutation in the deepest child propagates up.

If you make `selectedItem` read-only, status updates silently stop. Visual regression: jobs appear stuck at "converting" forever.

## Procedure

1. **Map the territory.** Read App.svelte (post-5a/5b state) end-to-end. Note:
   - Every `run*` function name and line range
   - Every `$state` declaration and which functions read/write it
   - Which state is used only by one component vs. shared
   - The Tauri event listeners (search for `listen('job-`)

2. **Extract in order: ChromaKeyPanel → AnalysisTools → OperationsPanel.**

   Rationale: ChromaKeyPanel is the most self-contained (single feature + preview). AnalysisTools is next (many small features, no output files). OperationsPanel is the biggest and most coupled — do it last when the patterns are settled.

3. **For each component:**

   a. Create the file with the props API.
   b. Move state + functions.
   c. Move template markup.
   d. Replace the relevant region in App.svelte with `<Component {...} />`.
   e. Add import.
   f. Run `npm test` — must pass.
   g. Run `cargo check --manifest-path src-tauri/Cargo.toml` (sanity).
   h. Smoke-test manually in `npm run tauri dev` — fire one op per extracted component and confirm status + progress updates.
   i. Commit + git note + push.

4. **Three commits total**, one per component. Each:
   ```
   refactor(ui): extract <Name> from App.svelte

   Moves <N> functions and <M> state declarations.
   Behavior unchanged; selectedItem remains bindable throughout.
   ```

## Verification — definition of done

- [ ] Three new files: `src/lib/OperationsPanel.svelte`, `src/lib/AnalysisTools.svelte`, `src/lib/ChromaKeyPanel.svelte`
- [ ] App.svelte no longer defines `_runOp` or any `run*` function moved into these components
- [ ] App.svelte imports all three
- [ ] At least one test per new component in `src/tests/` that:
  - mounts it with mocked `invoke`
  - fires the primary action
  - asserts `invoke` was called with the expected `run_operation` payload shape
- [ ] `npm test` passes
- [ ] `cargo check --manifest-path src-tauri/Cargo.toml` passes
- [ ] Dev run: select a video, run one op from each category, verify status updates and output files produced (or results rendered for analysis tools)
- [ ] App.svelte is now **under 2,500 lines** (sanity target; report the exact number)
- [ ] 3 commits, 3 git notes, all pushed
- [ ] `gh run list --limit 1` shows green CI

## Gotchas

- **`selectedItem` mutation MUST keep working.** Test explicitly: start a long-running op (e.g., `runTranscode` on a large file), verify progress bar updates. If it doesn't update, your binding chain is broken.

- **Tauri event listeners.** The `listen('job-progress', ...)` and `listen('job-complete', ...)` subscriptions may currently live in App.svelte. They mutate `queue[i].status` etc. by looking up by `job_id`. Leave them where they are unless QueueManager already owns them (check after 5b). Do not duplicate listeners — that causes double updates.

- **Chroma debounce timer cleanup.** The chroma preview debounce uses a `setTimeout` handle. On component unmount or when `selectedItem` changes, the pending timer must be cleared. Use `$effect(() => { return () => clearTimeout(_chromaPreviewTimer); })` or the return-function pattern. Do NOT leak timers.

- **`_chromaPreviewCache` is keyed by a hash of options + timecode.** Preserve the exact key-computation logic (`_chromaPreviewKeyOf`). Breaking cache keys means every param change triggers a full reencode.

- **Analysis results may be rendered in multiple places.** Loudness and VMAF results sometimes show on the Timeline as annotations. Grep for each result state name in `.svelte` files outside your extraction scope. If found, expose via callback prop or a store.

- **`_runOp` is shared.** Both OperationsPanel and ChromaKeyPanel's `runChromaKey` may call it. Put `_runOp` in OperationsPanel and have ChromaKeyPanel take it as a callback prop — OR duplicate the small helper. **Do not** put `_runOp` in a new `utils.js` with runes (runes don't work outside `.svelte` files). If you duplicate it, each copy must stay in sync — the function is small enough that duplication is acceptable.

- **Format compatibility checks.** `visibleQueue`, `compatibleOutputCats`, `compatibleTypes` — these derivations depend on `selectedItem.mediaType` and `globalOutputFormat`. They may live in App.svelte or in QueueManager after 5b. Check before you start; if they're in App.svelte, leave them — OperationsPanel can receive the derived values as plain props.

- **Do NOT run a release.** No version bumps, no tags.

- **If you run out of time on the third component:** commit what you have (two components extracted), report, and leave a `deferred:` note in the git commit note pointing to the remaining work. Do not leave a half-extracted component in the tree.

## If you get stuck

- If a function you're moving turns out to reference state that's scattered across many unrelated features, halt. Report which state is the tangle. The user will decide whether to rewrite the function or accept a larger props API.

- If Tauri event listeners cause double updates after your extraction, you've created a duplicate. Check all `listen('job-*', ...)` call sites — there must be exactly one subscription per event type.

- If tests pass but manual smoke shows the progress bar stuck, the `$bindable` chain is broken. Re-verify each layer: `App → OperationsPanel.selectedItem`, `App → QueueManager.selectedItem`, and that both point at the same object.

## Reporting

When done, report:
- App.svelte line count before and after (and cumulative since original 6,014)
- Line counts of the 3 new components
- Test count added
- CI run URL
- Any operation runners NOT moved (and why)
- Any Tauri event listener changes
- Any deviations from the proposed props APIs
