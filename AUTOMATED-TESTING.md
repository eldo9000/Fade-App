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

Generates small fixture files on the fly, runs real conversions through the Rust backend, asserts output files exist. ~8 tests, ~5–20 seconds depending on video encoding speed.

**What's covered:**
- Image: PNG → WebP (via ImageMagick)
- Video: MP4 → WebM (via ffmpeg, marked `#[ignore]` to keep default `cargo test` fast)
- Audio: WAV → MP3, AAC, M4A (ALAC 24-bit), Opus, OGG (via ffmpeg)
- Data: CSV → JSON (pure Rust, no external tool)

### Conversion matrix sweep

```bash
# Image + audio + data (fast)
cargo test --manifest-path src-tauri/Cargo.toml --test matrix -- --nocapture

# Plus video (slower, real ffmpeg encodes)
cargo test --manifest-path src-tauri/Cargo.toml --test matrix \
  -- --include-ignored --nocapture
```

Sweeps every live output format (and key settings variants per format) through Fade's real arg builders, writes outputs to `test-results/conversion-matrix/<category>/`, and prints a pass/fail table per category. Soft-fails — collects every failure and reports them all at the end rather than stopping on the first.

**Inspecting results:** open `test-results/conversion-matrix/` after a run. Every successful conversion produces a named output file (`png_to_webp_lossless.webp`, `wav_to_mp3_vbr_q2.mp3`, etc.). A missing file means a failed conversion — investigate manually.

**What's covered:**
- Image (`image_matrix`) — PNG fixture → JPEG (default, q25, q95), PNG, WebP (default, lossless), TIFF, BMP, AVIF
- Audio (`audio_matrix`) — WAV fixture → MP3 (default, CBR 192, VBR q2), WAV, FLAC (default, max compression), OGG VBR q5, AAC 192, Opus VBR 128, M4A (AAC, ALAC 24-bit)
- Data (`data_matrix`) — CSV fixture → JSON, YAML, XML, TSV, CSV
- Video (`video_matrix`, `#[ignore]`) — MP4 fixture → MP4 H.264, MP4 H.265, WebM VP9, MKV FFV1, MOV ProRes, MOV MJPEG, AVI Xvid, GIF

The output folder for each category is wiped at the start of its test so stale files from previous runs cannot mask a new failure. `test-results/` is gitignored.

### Full permutation sweep (diagnostic)

```bash
# Everything (slow — hundreds of cases, mostly video)
cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
  -- --include-ignored --nocapture

# One category at a time
cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
  image_full -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
  audio_full -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
  data_full  -- --ignored --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --test full_sweep \
  video_full -- --ignored --nocapture
```

**Difference from `matrix`:** the matrix is a smoke test — every case must pass. The full sweep is a *diagnostic* — it Cartesian-products every option per format, expects some combinations to be invalid by design, and never fails the test runner. Read the printed table to find which combinations broke. Outputs land in `test-results/full-sweep/<category>/`.

**Approximate case counts:** image ~140, audio ~110, data ~40, video ~400 (H.264 alone is 225 cases — full preset × profile × pix_fmt × tune Cartesian). Continuous ranges (quality 0–100, CRF 0–51, bitrate kbps) are sampled at three points; enums and small integer ranges get full coverage.

Use this before a release to spot-check that no encoder option is silently broken.

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
