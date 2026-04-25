# TASK-3: E2E UI sweep — format picker and all options panels

## Goal
WebdriverIO specs cover every format in the format picker and every major interactive control in each options panel (ImageOptions, VideoOptions, AudioOptions, DataOptions, ArchiveOptions); all tests pass without triggering any actual conversions.

## Context
Fade is a Tauri 2 + Svelte 5 desktop app. The E2E harness (WebdriverIO + tauri-driver, smoke test) was set up in a prior task. This task adds systematic UI tests that click every format and exercise every major control in the options panels.

**No conversions here.** These tests drive the UI only — select a format, interact with controls, assert the resulting state. No files need to be in the queue; the options panels render independently of the queue state.

**How the app's format/options system works:**
- The app has a single-page layout with a `FormatPicker` at the top and conditional options panels below.
- Selecting a format (e.g., "webp") shows `ImageOptions`. Selecting "mp4" shows `VideoOptions`. The currently rendered panel is gated on the selected format's media category.
- The format picker renders a set of segmented-button groups. The CSS class `seg-active` marks the active format button.
- All option panels use consistent CSS patterns: `seg-active` for active segments, `fade-check` for checkboxes, `fade-range` for sliders.

**Selector strategy:** WebdriverIO selectors must target stable DOM attributes. Where elements lack `data-testid` attributes, you are allowed to add them to the relevant Svelte components as part of this task. Limit additions to `data-testid` attributes only — no functional changes to any `.svelte` files.

**Files to add data-testid to (if needed after reading them):**
- `src/lib/FormatPicker.svelte` — the format picker buttons
- `src/App.svelte` — the options panel container (or the panels themselves)
- `src/lib/ImageOptions.svelte`
- `src/lib/VideoOptions.svelte`
- `src/lib/AudioOptions.svelte`
- `src/lib/DataOptions.svelte`
- `src/lib/ArchiveOptions.svelte`

**Format inventory (from the app source):**
The app supports formats across these categories: image (jpg, png, webp, tiff, avif, gif), video (mp4, mkv, webm, mov, avi), audio (mp3, wav, flac, ogg, aac, opus, m4a), data (csv, json, xml, yaml, tsv), archive (zip, tar.gz, 7z), and others (ebook, subtitle, model, document — not in scope for this task).

**Key controls per panel:**
- **ImageOptions:** quality slider, resize mode dropdown (none/percent/pixels), crop preset buttons
- **VideoOptions:** codec dropdown (H.264/H.265/AV1/VP9/ProRes), quality slider, preset dropdown, resolution dropdown, frame rate dropdown
- **AudioOptions:** bitrate dropdown, sample rate dropdown, trim start/end fields
- **DataOptions:** JSON pretty-print checkbox, CSV delimiter buttons (comma/semicolon/tab/pipe)
- **ArchiveOptions:** compression level slider

## In scope
- `e2e/specs/format-picker.spec.ts` — new
- `e2e/specs/image-options.spec.ts` — new
- `e2e/specs/video-options.spec.ts` — new
- `e2e/specs/audio-options.spec.ts` — new
- `e2e/specs/data-options.spec.ts` — new
- `e2e/specs/archive-options.spec.ts` — new
- `src/lib/FormatPicker.svelte` — allowed to add `data-testid` attributes only
- `src/lib/ImageOptions.svelte` — allowed to add `data-testid` attributes only
- `src/lib/VideoOptions.svelte` — allowed to add `data-testid` attributes only
- `src/lib/AudioOptions.svelte` — allowed to add `data-testid` attributes only
- `src/lib/DataOptions.svelte` — allowed to add `data-testid` attributes only
- `src/lib/ArchiveOptions.svelte` — allowed to add `data-testid` attributes only
- `src/App.svelte` — allowed to add `data-testid` attributes only

## Out of scope
- Any functional changes to `.svelte` files (logic, props, bindings, styles)
- Any changes to `src-tauri/` Rust source
- Any changes to `e2e/wdio.conf.ts` or `e2e/specs/smoke.spec.ts`
- Conversion smoke tests (those are in a separate task)
- Operations panel, ChromaKeyPanel, AnalysisTools, Timeline — advanced, out of scope
- Ebook, subtitle, model, document format options — out of scope

## Steps

1. **Read the Svelte source files** for FormatPicker, ImageOptions, VideoOptions, AudioOptions, DataOptions, ArchiveOptions, and enough of App.svelte to understand how the options panels are conditionally rendered. Identify: (a) how to click a format button, (b) what selector identifies each options panel when rendered, (c) what selectors target each key control within each panel.

2. **Add `data-testid` attributes** to each component wherever selectors are needed. At minimum:
   - Format picker: each format button gets `data-testid="format-{name}"` (e.g., `data-testid="format-webp"`)
   - Options panels: each panel root element gets `data-testid="panel-image"`, `data-testid="panel-video"`, etc.
   - Key controls within each panel: `data-testid="image-quality"`, `data-testid="image-resize-mode"`, `data-testid="video-codec"`, `data-testid="video-quality"`, `data-testid="audio-bitrate"`, `data-testid="audio-samplerate"`, `data-testid="data-json-pretty"`, `data-testid="data-csv-delimiter-comma"`, `data-testid="archive-compression"`, etc.
   - Only add attributes that are needed for the tests. Do not add them to every element.

3. **Write `e2e/specs/format-picker.spec.ts`:**
   - For each format category (image: webp, video: mp4, audio: mp3, data: csv, archive: zip), click the format button and assert: (a) the button has the `seg-active` class or is marked selected, (b) the corresponding options panel is displayed.
   - At minimum cover: one image format, one video format, one audio format, one data format, one archive format.

4. **Write `e2e/specs/image-options.spec.ts`:**
   - Setup: click a format that shows ImageOptions (e.g., webp or jpg).
   - Test quality slider: move to min value, assert displayed value changes; move to max, assert again.
   - Test resize mode: click "percent" option, assert percent fields appear; click "pixels", assert width/height fields appear; click "none", assert resize fields hidden.
   - Test crop preset: click one crop preset button, assert it becomes active.

5. **Write `e2e/specs/video-options.spec.ts`:**
   - Setup: click a video format (mp4).
   - Test codec dropdown: open it, select H.265, assert selection reflected.
   - Test quality slider: drag to min, assert value shown; drag to max.
   - Test preset dropdown: open, select "slow", assert selection.
   - Test resolution dropdown: open, select a non-default option, assert selection.
   - Test frame rate dropdown: open, select a non-default option, assert selection.

6. **Write `e2e/specs/audio-options.spec.ts`:**
   - Setup: click an audio format (mp3).
   - Test bitrate dropdown: open, select 192 kbps, assert selection.
   - Test sample rate dropdown: open, select 48000 Hz, assert selection.
   - Test trim start: type "00:00:05" into trim start field, assert value.
   - Test trim end: type "00:00:30" into trim end field, assert value.

7. **Write `e2e/specs/data-options.spec.ts`:**
   - Setup: click a data format (csv).
   - Test CSV delimiter buttons: click "semicolon" button, assert it is active/selected; click "tab"; click "comma" to restore.
   - Switch to json format. Test pretty-print checkbox: assert initial state, click to toggle, assert state changed.

8. **Write `e2e/specs/archive-options.spec.ts`:**
   - Setup: click an archive format (zip).
   - Test compression slider: set to 0 (minimum), assert value; set to 9 (maximum), assert value.

9. **Run `npm run test:e2e` locally** to verify all new specs pass. Fix any selector mismatches.

## Success signal
- All six spec files exist in `e2e/specs/`.
- `npm run test:e2e` exits 0 with all tests passing.
- No `.svelte` file has any change other than added `data-testid` attributes.
- The format picker tests cover at least one format per category (image, video, audio, data, archive).
- Each options panel test exercises at least three distinct controls.

## Notes
- WebdriverIO element interaction: use `$('[data-testid="..."]')` for selection, `.click()` to click, `.setValue()` for inputs, `.selectByVisibleText()` or `.click()` for custom dropdowns (Fade uses custom segmented-button dropdowns, not native `<select>`).
- Svelte 5 with Tailwind: the active state on segmented buttons is toggled via CSS class (`seg-active`). Use `browser.execute()` to read className if `.getAttribute('class')` is insufficient.
- If a dropdown is a custom Svelte component (not a native `<select>`), click the trigger to open it, then click the option inside the opened list.
- Sliders may use HTML range inputs (`<input type="range">`) — use `.setValue('0')` directly on the element.
- Do not test that conversions actually happen — just that UI state changes correctly.
