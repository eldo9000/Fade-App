# TASK-2: Surface H.264 profile auto-promotion in VideoOptions UI

## Goal
When the user has H.264 selected and picks `pix_fmt = yuv422p` or `yuv444p`, the H.264 profile dropdown either grays out the unreachable options (`baseline`, `main`, `high`) or shows an inline hint explaining the auto-promotion (`"Will use High 4:2:2 profile for yuv422p"` etc.). The UI no longer presents combinations that silently get rewritten by the backend without telling the user.

## Context
TASK-1 made the Rust arg builder auto-promote the H.264 profile to `high422` or `high444` when `pix_fmt` is high-bit-depth chroma. That fix closes the broken-output bug (660 failing combinations from `full_sweep.rs::video_full`). But the UI still presents `baseline` / `main` / `high` as if they were valid choices for those pix_fmts — the user picks one, and the backend silently overrides it. This task closes the UI half of the gap.

The "what you set is what you get" principle is a Fade-wide stance. Allowing the backend to silently rewrite a user-visible choice violates it. Two acceptable shapes:

**(a) Disable + tooltip.** When `pix_fmt` is yuv422p, gray out the `baseline` and `main` profile buttons; leave `high` enabled but display a small `(→ High 4:2:2)` suffix or tooltip. When `pix_fmt` is yuv444p, similar with `(→ High 4:4:4)`.

**(b) Inline hint.** Keep all profile buttons enabled but show an inline note below the dropdown: `"yuv422p chroma → encoded as High 4:2:2 profile"` / similar.

Pick (a) — disable + suffix. Reasoning: it's the strongest "what you set is what you get" experience. The backend will still auto-promote (TASK-1's fix is the load-bearing safety net), but a user who's looking at the UI will not see misleading options.

Relevant files:
- `src/lib/VideoOptions.svelte` — has the H.264 profile picker (likely a button group or dropdown bound to `options.h264_profile`) and the pix_fmt picker.
- `src/lib/segStyles.js` — shared helpers for segmented controls; if existing buttons use `seg-active`, the disabled state needs a similar canonical class. Check what's available before inventing new styles.
- `e2e/specs/video-options.spec.ts` — Playwright CT test for VideoOptions. Adding a new behavior should add at least one spec covering it.
- `e2e/wrappers/VideoOptionsWrapper.svelte` — wrapper for the CT mount. May need to expose pix_fmt as a controllable prop if it isn't already.

## In scope
- `src/lib/VideoOptions.svelte` — disable baseline/main when pix_fmt is yuv422p/yuv444p; suffix the high button when auto-promotion is in effect
- `e2e/specs/video-options.spec.ts` — add a test asserting the disabled state and the suffix text
- `e2e/wrappers/VideoOptionsWrapper.svelte` — only if pix_fmt isn't already a `$bindable` or controllable prop; add it minimally if needed

## Out of scope
- Any change to `src-tauri/src/args/video.rs` or `ConvertOptions` — that's TASK-1, already done
- Any change to other format pickers (only H.264 has this profile/pix_fmt issue exposed in the UI)
- Restyling the segmented-control component itself
- Refactoring how VideoOptions reads `options.codec` or `options.pix_fmt`
- New UI options or new fields

## Steps
1. Read `src/lib/VideoOptions.svelte` end to end. Find the H.264 profile picker — likely a section gated by `options.codec === 'h264'`. Note how the buttons are constructed and how they receive their selected/disabled states. Find the pix_fmt picker too.
2. Read `src/lib/segStyles.js` (if it exists) for the canonical disabled-state class. If there's no disabled style, use Tailwind's `opacity-50 cursor-not-allowed` plus `pointer-events-none` on the button.
3. Add a `$derived` (Svelte 5 runes) — call it `h264ProfileLocked` — that evaluates to `'yuv422p'`, `'yuv444p'`, or `null` based on `options.pix_fmt`. Use it to:
   - Disable the `baseline` and `main` profile buttons when `h264ProfileLocked` is non-null.
   - Suffix the `high` button label with ` (→ High 4:2:2)` when locked='yuv422p' and ` (→ High 4:4:4)` when locked='yuv444p'. The suffix can be plain text inside the same `<button>` or a small adjacent `<span class="text-xs opacity-60">`.
   - When the user selects yuv422p or yuv444p while currently on baseline or main, force `options.h264_profile = 'high'` so the UI state is consistent with what gets sent.
4. Read `e2e/specs/video-options.spec.ts` to see the existing test pattern. Find a test that exercises the codec dropdown.
5. If the existing `VideoOptionsWrapper.svelte` doesn't already expose `pix_fmt` as bindable, add it — match the pattern the wrapper uses for other bindable fields. Do not refactor unrelated wrapper structure.
6. Add a Playwright CT test: mount VideoOptions with codec=h264, switch pix_fmt to yuv422p, assert the baseline button is disabled (`page.getByRole('button', { name: 'Baseline' }).isDisabled()` or matches the disabled class). Assert the high button text contains `4:2:2`. Repeat for yuv444p with 4:4:4.
7. Run `npm run test:e2e` locally if Playwright is installed. CI is the truth either way; if local Playwright is unavailable on this machine, push and let CI verify.
8. No Rust changes — no `cargo fmt` / `cargo clippy` needed. Do run `npm run check` (Svelte typecheck) if it exists in `package.json`.

## Success signal
- Visual check: with H.264 selected, switching pix_fmt to yuv422p disables the baseline + main profile buttons and the high button now reads `High (→ High 4:2:2)` (exact wording optional, the `4:2:2` substring matters).
- New CT spec passes: `npm run test:e2e -- video-options` exits 0 with the new test row green.
- `grep "h264ProfileLocked\|→ High 4:2:" src/lib/VideoOptions.svelte` returns at least 2 lines (the derived definition + the suffix usage).
- No regression in the existing 55 CT tests — full `npm run test:e2e` exits 0.

## Notes
- Svelte 5 runes only. `$derived` not `$:`.
- The `force options.h264_profile = 'high'` write in step 3's third sub-bullet should happen via `$effect`, not directly in `$derived` (derived must be pure). Run the effect on `pix_fmt` change.
- If the pix_fmt selector in the UI is itself disabled or hidden in some configurations (e.g. when codec isn't H.264), the locked-state derivation still works — `h264ProfileLocked` reads `options.pix_fmt` directly, no codec check needed because the H.264 profile section is itself gated by `codec === 'h264'`.
- Watch for the `$bindable(null)` cascade pattern (see CLAUDE.md Known Patterns once TASK-3 lands) — do not add new `$bindable(default_value)` defaults. If a field is conditionally rendered, initialize the parent's state to a concrete value at the top.
