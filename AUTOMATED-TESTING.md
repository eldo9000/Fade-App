# Automated Testing — Fade

Two test suites: **Playwright component tests** (UI) and **Rust integration tests** (conversions).

---

## Prerequisites

- Node 22+ and Rust stable already required for dev — nothing new there
- `ffmpeg` and `imagemagick` in PATH (required for Rust conversion tests only)
  - macOS: `brew install ffmpeg imagemagick`

---

## One-time setup

Download the Chromium browser binary Playwright uses to run UI tests:

```bash
npx playwright install chromium
```

Run this once after cloning. ~170 MB, stored in `~/.cache/ms-playwright/`.

---

## Running the tests

### UI component tests (Playwright CT)

```bash
npm run test:e2e
```

Mounts each Svelte options panel in real Chromium and exercises every major control. 55 tests, ~15–20 seconds.

**What's covered:**
- `FormatPicker` — clicking each format button selects it
- `ImageOptions` — quality slider, resize mode (none/percent/pixels), crop presets
- `VideoOptions` — codec dropdown (H.264/H.265/AV1/ProRes), quality slider, preset, resolution, framerate
- `AudioOptions` — bitrate dropdown, sample rate dropdown, trim start/end inputs
- `DataOptions` — CSV delimiter buttons (comma/semicolon/tab/pipe), JSON pretty-print checkbox
- `ArchiveOptions` — compression slider min/max

### Rust conversion tests

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test conversions -- --include-ignored
```

Generates small fixture files on the fly, runs real conversions through the Rust backend, asserts output files exist. 4 tests, ~5–20 seconds depending on video encoding speed.

**What's covered:**
- Image: PNG → WebP (via ImageMagick)
- Video: MP4 → WebM (via ffmpeg, marked `#[ignore]` to keep default `cargo test` fast)
- Audio: WAV → MP3 (via ffmpeg)
- Data: CSV → JSON (pure Rust, no external tool)

### All tests (vitest unit tests + Playwright CT)

```bash
npm test && npm run test:e2e
```

`npm test` runs the existing vitest unit tests (queue, concurrency, operations panels). `npm run test:e2e` runs the Playwright CT suite.

---

## Where everything lives

```
playwright-ct.config.ts          # Playwright CT runner config
playwright/
  index.ts                       # CT entry point (required boilerplate)
  index.html                     # CT Vite entry (required boilerplate)
e2e/
  specs/
    smoke.spec.ts                # Sanity check: DataOptions mounts
    format-picker.spec.ts
    image-options.spec.ts
    video-options.spec.ts
    audio-options.spec.ts
    data-options.spec.ts
    archive-options.spec.ts
  wrappers/
    FormatPickerWrapper.svelte   # Thin $state() wrapper for each component
    ImageOptionsWrapper.svelte   # (needed so Svelte 5 $bindable() mutations
    VideoOptionsWrapper.svelte   #  are visible in the DOM during tests)
    AudioOptionsWrapper.svelte
    DataOptionsWrapper.svelte
    ArchiveOptionsWrapper.svelte
src-tauri/
  tests/
    conversions.rs               # Rust integration tests
```

---

## CI

Both suites run automatically on every push to `main` via `.github/workflows/ci.yml`:

- **E2E component tests** — `npx playwright install --with-deps chromium` then `npm run test:e2e`
- **Rust conversion tests** — `brew install ffmpeg imagemagick` then `cargo test --test conversions -- --include-ignored`

---

## Adding new tests

**New UI test** — add a `*.spec.ts` file to `e2e/specs/`. Import from `@playwright/experimental-ct-svelte`. If the component uses `$bindable()` props, create a wrapper in `e2e/wrappers/` that holds state as `$state()` and passes it with `bind:`. Mount the wrapper, not the component directly.

**New Rust conversion test** — add a `#[test]` function to `src-tauri/tests/conversions.rs`. Use `run_cmd()` to generate a fixture, call the converter, assert output exists. Mark slow tests with `#[ignore]` so they only run when `--include-ignored` is passed.

---

## Known limitations

- **No full-app E2E** — `tauri-driver` does not support macOS in Tauri 2.x (macOS listed as `[Todo]` upstream). If that changes, full WebdriverIO app tests become viable.
- **UI tests don't test conversions** — the option panel tests verify UI state only; no files are actually converted. That's what the Rust tests cover.
- **Operations panel, ChromaKeyPanel, AnalysisTools, Timeline** — not yet covered. These components have Tauri API imports and require mocking `@tauri-apps/api` to test in CT.
