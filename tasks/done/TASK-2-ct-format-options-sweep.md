# TASK-2: Playwright CT — format picker and options panel sweep

## Goal
Playwright CT specs cover `FormatPicker`, `ImageOptions`, `VideoOptions`, `AudioOptions`, `DataOptions`, and `ArchiveOptions` — every major interactive control is exercised and state assertions pass.

## Context
Fade is a Tauri 2 + Svelte 5 desktop app. The Playwright CT harness (smoke test green, CI configured) was set up in a prior task. This task adds the systematic UI sweep.

**All six components are pure presentational — no Tauri API imports, no global stores.** Each accepts an `options = $bindable()` prop (the `ConvertOptions` object) and callback props for events. They can be mounted in isolation with a plain `options` object.

**ConvertOptions shape:** Read `src/lib/types/generated/ConvertOptions.ts` before writing tests — it is the authoritative list of every field and its TypeScript type. Pass only the fields relevant to each panel; use `null` for everything else (or use `as any` on the object to avoid TypeScript errors on incomplete objects).

**Component-specific props beyond `options`:**
- `FormatPicker`: also receives `formats` (array of format strings), `label`, `upperCase`, `ariaLabel`
- `ImageOptions`: also receives `onqualitystart`, `onqualityinput`, `onqualityend`, `oncropstart`, `oncropclear`, `cropActive`, `cropAspect` callbacks
- `VideoOptions`, `AudioOptions`: also receive `errors = {}`

**Selector strategy:** Read each component's Svelte source to find the actual element structure (CSS classes, element types, text content) and use Playwright locators that match. You are allowed to add `data-testid` attributes to the Svelte components if stable selectors are not otherwise available — but only as a last resort after trying CSS class and role-based selectors. The existing CSS patterns are `seg-active` (active segmented button), `fade-check` (checkboxes), `fade-range` (sliders).

**What to test per component:**
- `FormatPicker`: given formats `['jpg', 'png', 'webp']`, clicking each button selects it (active class applied, `options.output_format` updated)
- `ImageOptions`: quality slider min/max; resize mode cycle (none → percent → pixels); crop preset selection
- `VideoOptions`: codec dropdown (select H.265, AV1, ProRes); quality slider; preset dropdown; resolution dropdown; framerate dropdown
- `AudioOptions`: bitrate dropdown (select 192k); sample rate dropdown (select 48000); trim start/end inputs
- `DataOptions`: CSV delimiter buttons (comma → semicolon → tab → pipe and back); JSON pretty-print checkbox toggle
- `ArchiveOptions`: compression slider min/max

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
- `src/lib/types/generated/ConvertOptions.ts` — read-only; source of truth for options shape

## Out of scope
- Any functional changes to `.svelte` files (logic, bindings, styles, non-testid attributes)
- Any changes to `src-tauri/` Rust source files
- Any changes to existing vitest tests
- Any changes to `playwright-ct.config.ts` or `e2e/specs/smoke.spec.ts`
- Conversion smoke tests (separate task)
- Operations panel, ChromaKeyPanel, AnalysisTools, Timeline

## Steps

1. **Read `src/lib/types/generated/ConvertOptions.ts`** in full. Note every field relevant to the six components. Build a mental model of the minimum valid `options` object for each panel.

2. **Read each Svelte component** (`FormatPicker.svelte`, `ImageOptions.svelte`, `VideoOptions.svelte`, `AudioOptions.svelte`, `DataOptions.svelte`, `ArchiveOptions.svelte`) to understand the DOM structure and identify selectors for each interactive element.

3. **Add `data-testid` attributes** to any elements that lack stable CSS-class or role-based selectors. Do this surgically — one attribute per element that needs it. Prefer existing patterns before adding testids.

4. **Write `e2e/specs/format-picker.spec.ts`:**
   - Import from `@playwright/experimental-ct-svelte`
   - Import `FormatPicker` from `../../src/lib/FormatPicker.svelte`
   - Mount with `formats: ['jpg', 'png', 'webp']` and `options: { output_format: 'jpg' } as any`
   - Test: clicking each format button makes that format active (assert class or aria-pressed)
   - Test: after clicking 'webp', assert `options.output_format` changed — use a Svelte binding update callback or check the active visual state

5. **Write `e2e/specs/image-options.spec.ts`:**
   - Mount `ImageOptions` with initial `options` (include `quality: 80, resize_mode: 'none'` and other required fields as null)
   - Test quality slider: locate the slider, set value to 0, assert displayed; set to 100
   - Test resize mode: click 'percent', assert percent input appears; click 'pixels', assert width/height inputs appear; click 'none', assert they disappear
   - Test crop preset: click a crop preset button, assert it becomes active

6. **Write `e2e/specs/video-options.spec.ts`:**
   - Mount `VideoOptions` with initial `options` (include `video_codec: 'h264'`, `crf: 23`, and other fields as null)
   - Test codec dropdown: open it, select H.265, assert selection reflected in UI
   - Test preset dropdown: open, select 'slow', assert reflected
   - Test quality slider: move to 0 and 51, assert value displays
   - Test resolution dropdown: open, select a non-default option
   - Test framerate dropdown: open, select a non-default option

7. **Write `e2e/specs/audio-options.spec.ts`:**
   - Mount `AudioOptions` with initial `options` (include `audio_bitrate: '128k'`, `audio_sample_rate: 44100`, and other fields as null)
   - Test bitrate: open dropdown, select '192k', assert reflected
   - Test sample rate: open dropdown, select 48000, assert reflected
   - Test trim start: type '00:00:05', assert input value
   - Test trim end: type '00:00:30', assert input value

8. **Write `e2e/specs/data-options.spec.ts`:**
   - Mount `DataOptions` with `options: { output_format: 'csv', csv_delimiter: ',', json_pretty: false } as any`
   - Test delimiter buttons: click semicolon, assert it's active; click tab; click comma to restore
   - Switch output format to 'json' (update the prop): assert JSON pretty-print control is visible
   - Test pretty-print checkbox: click it, assert it's checked; click again, assert unchecked

9. **Write `e2e/specs/archive-options.spec.ts`:**
   - Mount `ArchiveOptions` with `options: { output_format: 'zip', compression_level: 5 } as any`
   - Test compression slider: set to 0, assert value; set to 9, assert value

10. **Run `npm run test:e2e` locally.** Fix any selector mismatches, prop shape errors, or component rendering failures before returning.

## Success signal
- All six spec files exist in `e2e/specs/`.
- `npm run test:e2e` exits 0 with all tests passing across all six specs.
- No `.svelte` file has any change other than `data-testid` attribute additions.
- Each spec file has at least 3 passing tests.

## Notes
- **Svelte 5 bindable props in CT:** When the component mutates `options` internally (via `options.field = newValue`), the change is visible in the DOM but not directly readable as a JS value from Playwright. Test by asserting the DOM state (selected class, displayed text, input value) rather than reading the prop value.
- **Custom dropdowns:** Fade uses custom segmented-button dropdowns, not native `<select>`. To interact: click the dropdown trigger to open it, then click the option element in the revealed list.
- **`as any` on options:** TypeScript will complain about incomplete `ConvertOptions` objects. Use `as any` or `as Partial<ConvertOptions>` for test props — this is intentional and acceptable in tests.
- **Updating props after mount:** In Playwright CT, call `component.update({ props: { options: { ...newOptions } } })` to re-render with new props. Use this for the DataOptions json/csv format switch test.
- If any component imports a sub-component that has Tauri API imports, the test will fail with a module resolution error. In that case, return a halt with the specific sub-component identified.
