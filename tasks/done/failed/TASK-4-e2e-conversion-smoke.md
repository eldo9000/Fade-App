# TASK-4: E2E conversion smoke tests — one per media category

## Goal
`e2e/specs/conversions.spec.ts` contains one conversion test per media category (image, video, audio, data, archive); each test invokes the Tauri `convert_file` command directly via IPC, waits for the `job-done` event, and asserts the output file exists on disk.

## Context
Fade is a Tauri 2 + Svelte 5 desktop app. The E2E harness (WebdriverIO + tauri-driver) and test fixtures were set up in prior tasks. This task adds the conversion smoke layer — verifying that the Rust backend actually converts files end-to-end.

**Why IPC-direct instead of UI-driven conversions:**
Tauri handles file drops at the OS/Rust level, not via HTML drag-and-drop events. Simulating a file drop via WebdriverIO is not reliable. Instead, tests call `window.__TAURI_INTERNALS__.invoke('convert_file', {...})` from within the webview context — this bypasses the queue UI entirely and drives the same Rust backend path that real conversions use.

**IPC call pattern:**
```ts
// Inside browser.executeAsync() or browser.execute() with a callback
const result = await browser.executeAsync((fixturePath, done) => {
  const { invoke, listen } = window.__TAURI_INTERNALS__
  const jobId = crypto.randomUUID()

  listen(`job-done`, (event) => {
    if (event.payload.job_id === jobId) {
      done({ success: true, outputPath: event.payload.output_path })
    }
  })

  invoke('convert_file', {
    jobId,
    inputPath: fixturePath,
    options: { /* ConvertOptions fields */ }
  }).catch((err) => done({ success: false, error: String(err) }))
}, fixturePath)
```

**ConvertOptions structure** (from `src/lib/types/generated/ConvertOptions.ts` — read this file to see all fields). The minimum required fields for each conversion:
- Image: `{ output_format: 'webp', quality: 80 }` (plus required nulls for video/audio fields)
- Video: `{ output_format: 'mp4', video_codec: 'h264', crf: 23 }` (plus required nulls)
- Audio: `{ output_format: 'mp3', audio_bitrate: '192k' }` (plus required nulls)
- Data: `{ output_format: 'json' }` (plus required nulls)
- Archive: `{ output_format: 'tar_gz' }` (plus required nulls)

**Read `src/lib/types/generated/ConvertOptions.ts` before writing tests** — it is the authoritative source for every field name and its TypeScript type. All fields not relevant to a conversion should be set to `null`.

**Output path convention:** Fade derives the output path from the input path by changing the extension and appending to the same directory. The test must know the expected output path to assert the file exists. Strategy: after the `job-done` event fires, use `event.payload.output_path` as the path to verify — call `window.__TAURI_INTERNALS__.invoke('file_exists', { path })` to check it.

**Cleanup:** Each test must delete its output file after asserting it exists. Use `browser.execute(() => window.__TAURI_INTERNALS__.invoke('scan_dir', ...))` or the `fs` module (not available in browser context) — alternatively, add the cleanup to the Node.js layer via `browser.execute(() => fetch('...'))` — actually the cleanest approach: use `browser.execute(() => window.__TAURI_INTERNALS__.invoke('file_exists', { path }))` to verify, and clean up in `afterEach` via a Node.js `fs.unlinkSync` call using the path returned from `job-done`.

**Fixtures path:** The fixture files are at `e2e/fixtures/test.{png,mp4,wav,csv,zip}`. Use absolute paths when invoking — `FIXTURES.png` etc. from `e2e/helpers/fixtures.ts` gives the absolute path. Pass these to `browser.executeAsync()` as parameters (they are serialized by WebdriverIO automatically).

**CI tools check:** ffmpeg and imagemagick must be in PATH on the CI runner. The macOS-14 GitHub Actions runner does not have ffmpeg by default — add `brew install ffmpeg imagemagick` to `.github/workflows/ci.yml` before the `Generate E2E fixtures` step. Check the current ci.yml to see if this step already exists; if not, add it.

## In scope
- `e2e/specs/conversions.spec.ts` — new
- `.github/workflows/ci.yml` — add `brew install ffmpeg imagemagick` step if not already present (before the fixtures generation step)

## Out of scope
- Any changes to `src/` Svelte source files
- Any changes to `src-tauri/src/` Rust source files
- Any changes to existing vitest tests
- Any changes to other e2e spec files
- More than one conversion test per media category
- Operations, chroma key, analysis, timeline conversions — out of scope

## Steps

1. **Read `src/lib/types/generated/ConvertOptions.ts`** in full. Note every field name, its TypeScript type, and whether it's required or nullable. This is the ground truth for what to pass to `convert_file`.

2. **Read `src-tauri/src/lib.rs`** and locate the `convert_file` command signature to confirm parameter names (they are snake_case at the Rust level and serialized from camelCase at the JS level via serde). Note whether the command takes `job_id` or `jobId` on the JS side.

3. **Check `.github/workflows/ci.yml`** for an existing `brew install ffmpeg` step. If absent, add it before the `Generate E2E fixtures` step:
   ```yaml
   - name: Install ffmpeg and imagemagick
     run: brew install ffmpeg imagemagick
   ```

4. **Write `e2e/specs/conversions.spec.ts`** with these five tests inside a single `describe('Conversions', ...)` block. Each test follows this pattern:
   - Call `browser.executeAsync()` passing the fixture path as a parameter
   - Inside the async call: register a `job-done` listener (filtered by job_id), then call `invoke('convert_file', { jobId, inputPath, options: { output_format, ...all other fields as null } })`
   - Wait for `done()` to be called with the result
   - Assert `result.success === true`
   - Assert `await browser.execute((path) => window.__TAURI_INTERNALS__.invoke('file_exists', { path }), result.outputPath)` returns `true`
   - In `afterEach` or the test body: delete the output file via Node.js `fs.unlinkSync(outputPath)` — the output path is known once the test receives it from `job-done`.

   **Test 1: Image — PNG to WebP**
   - inputPath: `FIXTURES.png`
   - options: `{ output_format: 'webp', quality: 80, ...all other fields null }`

   **Test 2: Video — MP4 re-encode to MP4 (H.264)**
   - inputPath: `FIXTURES.mp4`
   - options: `{ output_format: 'mp4', video_codec: 'h264', crf: 23, ...all other fields null }`

   **Test 3: Audio — WAV to MP3**
   - inputPath: `FIXTURES.wav`
   - options: `{ output_format: 'mp3', audio_bitrate: '192k', ...all other fields null }`

   **Test 4: Data — CSV to JSON**
   - inputPath: `FIXTURES.csv`
   - options: `{ output_format: 'json', ...all other fields null }`

   **Test 5: Archive — ZIP to TAR.GZ**
   - inputPath: `FIXTURES.zip`
   - options: `{ output_format: 'tar_gz', ...all other fields null }`

5. **Add timeout handling:** if the `job-done` event doesn't fire within 30 seconds, call `done({ success: false, error: 'timeout' })`. The `waitforTimeout` in wdio.conf.ts is 30000ms; `browser.executeAsync` inherits this.

6. **Run `npm run test:e2e` locally** to verify all five conversion tests pass. Fix any IPC field name mismatches (the most likely failure point — field names must exactly match what `convert_file` expects).

## Success signal
- `e2e/specs/conversions.spec.ts` exists with five tests.
- `npm run test:e2e` exits 0 with all five conversion tests passing.
- Each test produces an output file that is verified to exist and then cleaned up.
- `.github/workflows/ci.yml` includes `brew install ffmpeg imagemagick` before the fixture generation step.

## Notes
- `window.__TAURI_INTERNALS__` is the Tauri 2 internal API. In Tauri 2 the public API is `window.__TAURI__` (from `@tauri-apps/api`), but within the webview the internals object has `invoke` and `listen` directly. If `__TAURI_INTERNALS__` is not available, try `window.__TAURI__.tauri.invoke` or import via the module API. Check Tauri 2 docs for the correct `browser.execute()` entry point.
- The `job-done` event listener must be registered BEFORE calling `invoke` to avoid a race where the job completes synchronously before the listener is set up.
- `convert_file` signature at the Rust level uses `job_id: String` — at the JavaScript IPC layer serde deserializes camelCase: likely `jobId`. Confirm from the source in Step 2.
- Output file name: Fade likely names output as `{input_stem}_out.{ext}` or similar. The exact name comes from `event.payload.output_path` — don't hardcode it; use the event payload.
- If the video conversion test times out, increase the individual test timeout in `mochaOpts` or add a per-test `this.timeout(90000)` — H.264 encoding of a 3-second clip on CI can take 10–20 seconds.
- For the archive test: Fade's ZIP→TAR.GZ conversion may use a different output_format key. Check `src/lib/types/generated/ConvertOptions.ts` for the exact string value (may be `'tar.gz'` with a dot, not underscore).
