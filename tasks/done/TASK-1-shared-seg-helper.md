# TASK-1: Extract `seg` / `segV` segmented-button helpers into a shared module

## Goal
A single `src/lib/segStyles.js` module exports `seg(active, i, total)` and
`segV(active, i, total)`, both returning Tailwind class strings for connected
segmented buttons. `VideoOptions.svelte` imports from this module instead of
defining the helpers inline. No visual change in the app.

## Context

Fade is a Tauri 2 + Svelte 5 media-conversion app. The options panel on the
right sidebar shows per-format controls (codec, resolution, etc) built out of
"segmented" button groups — rows (`seg`) or vertical stacks (`segV`) of
connected buttons where one is active.

The canonical styling was just finalized on the MP4 / Video panel:
- Inactive buttons: `seg-inactive` CSS class (bevel gradient, bottom-anchored)
- Active buttons: `seg-active` CSS class (50%-darker accent fill, 2px accent
  underline inside the bottom edge with 10px gutters, no 1px outline)
- Horizontal: `px-3 py-[5px] text-center text-[12px] font-medium border
  transition-colors relative`, connected via `rounded-l-md`/`rounded-r-md` +
  `-ml-px` overlap
- Vertical: `w-full px-3 py-[5px] text-left text-[12px] font-medium border
  transition-colors relative`, connected via `rounded-t-md`/`rounded-b-md` +
  `-mt-px` overlap

`VideoOptions.svelte` defines these inline today at `src/lib/VideoOptions.svelte`
(`seg()` and `segV()` near the top of the `<script>`). `AudioOptions.svelte`,
`ImageOptions.svelte`, and `ArchiveOptions.svelte` each have their own older,
slightly different copies that we'll sweep in follow-up tasks. Before we
sweep, the canonical helper needs to live in one place.

The `.seg-active` and `.seg-inactive` CSS classes are already defined in
`src/app.css` — do not touch the CSS in this task, only the JS helper.

## In scope
- Create `src/lib/segStyles.js`
- Edit `src/lib/VideoOptions.svelte` — remove local `seg()` / `segV()` and
  import from the new module

## Out of scope
- `AudioOptions.svelte`, `ImageOptions.svelte`, `ArchiveOptions.svelte`,
  `DataOptions.svelte`, `FormatPicker.svelte` — those get their own tasks
- Any CSS changes
- The unused `devSeg` / `devSegV` helpers in VideoOptions — leave them alone
- Renaming or refactoring unrelated code

## Steps

1. Read `src/lib/VideoOptions.svelte` and copy the exact current bodies of
   `seg(active, i, total)` and `segV(active, i, total)` — they are the source
   of truth. The active branch should return `'seg-active z-10'`; the inactive
   branch should return `'seg-inactive border-[var(--border)] text-[color-mix(in_srgb,var(--text-primary)_70%,transparent)] hover:z-10'`.

2. Create `src/lib/segStyles.js` exporting both functions with those exact
   bodies. Plain ES module, no Svelte runes (these are pure string helpers).

3. In `VideoOptions.svelte`, remove the local `seg` and `segV` function
   declarations and add `import { seg, segV } from './segStyles.js';` at the
   top of the `<script>` block.

4. Run `npx vite build` and confirm it exits 0 with no new warnings about the
   changed file.

5. Manually scan `VideoOptions.svelte` to confirm the template still calls
   `seg(...)` / `segV(...)` and nothing else references the deleted local
   definitions.

## Success signal
- `src/lib/segStyles.js` exists and exports `seg`, `segV`.
- `src/lib/VideoOptions.svelte` no longer contains `function seg(` or
  `function segV(` (only the `devSeg`/`devSegV` remain).
- `npx vite build` succeeds.
- When the app is running, the MP4 output panel renders identically to before
  (visual spot check — same button padding, same active underline, same
  inactive bevel).
