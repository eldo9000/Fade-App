# TASK-1: Bootstrap WebdriverIO + tauri-driver E2E harness

## Goal
`npm run test:e2e` launches the Fade desktop app via WebDriver, runs a smoke test that asserts the window title is "Fade", and the CI job on macOS-14 passes green with this step included.

## Context
Fade is a Tauri 2 + Svelte 5 desktop app. The goal is to add a true end-to-end test harness that drives the actual running desktop app. Playwright cannot target the embedded WebKit webview in Tauri — the correct tool is **WebdriverIO**, which speaks WebDriver protocol natively and pairs with `tauri-driver` (the official Tauri WebDriver adapter).

On macOS, `tauri-driver` wraps `safaridriver`, which ships with the OS. The macOS-14 GitHub Actions runner already has it.

**How the harness works:**
1. The Tauri app is compiled with a `automation` cargo feature enabled, which tells the Tauri runtime to expose a WebDriver endpoint.
2. `tauri-driver` is started as a child process — it acts as the WebDriver server (port 4444).
3. WebdriverIO connects to port 4444, which launches the app binary and allows test commands to control its webview.

**Cargo feature strategy:** Add a `[features]` section to `src-tauri/Cargo.toml` with `automation = ["tauri/automation"]`. The release build never uses `--features automation`; only the e2e build step does.

**Reference:** https://tauri.app/develop/tests/webdriver/ and https://webdriver.io/docs/tauri

**Existing CI:** `.github/workflows/ci.yml` runs on `macos-14`. Current steps: checkout, setup-node (v22), setup Rust stable, git auth for private cargo deps, cargo cache, `npm ci`, `npm test` (vitest), `cargo fmt --check`, `cargo clippy`, `cargo test --lib`. E2E step goes after all existing steps.

**App binary path after `cargo build --features automation`:** `src-tauri/target/debug/fade` (the macOS binary, not the .app bundle — tauri-driver needs the raw binary).

## In scope
- `src-tauri/Cargo.toml` — add `[features]` block
- `package.json` — add WebdriverIO devDependencies + `test:e2e` script
- `e2e/wdio.conf.ts` — new file (WebdriverIO config)
- `e2e/specs/smoke.spec.ts` — new file (one smoke test)
- `.github/workflows/ci.yml` — add e2e steps after existing test steps

## Out of scope
- Any changes to `src/` Svelte source files
- Any changes to `src-tauri/src/` Rust source files
- Any changes to existing vitest tests in `src/tests/`
- Writing any test specs beyond the single smoke test

## Steps

1. **Add automation feature to Cargo.toml.** In `src-tauri/Cargo.toml`, add a `[features]` section after the `[dependencies]` block:
   ```
   [features]
   automation = ["tauri/automation"]
   ```
   Do not add `automation` to the default features list. The tauri dep line itself (`tauri = { version = "2", features = [...] }`) remains unchanged — `tauri/automation` is only activated when `--features automation` is passed at build time.

2. **Install WebdriverIO packages.** From the repo root, run:
   ```
   npm install --save-dev webdriverio @wdio/cli @wdio/local-runner @wdio/mocha-framework @wdio/spec-reporter @types/node
   ```
   Verify they appear in `package.json` devDependencies.

3. **Create `e2e/wdio.conf.ts`.** This file must:
   - Import `spawn` / `ChildProcess` from `child_process` and `resolve` from `path`
   - Declare a `let tauriDriver: ChildProcess` variable
   - Export a `config` object with:
     - `port: 4444`, `path: '/'`
     - `specs: ['e2e/specs/**/*.spec.ts']`
     - `maxInstances: 1`
     - `capabilities`: one entry with `browserName: 'wry'`, `'tauri:options': { application: resolve(__dirname, '../src-tauri/target/debug/fade') }`, `acceptInsecureCerts: true`
     - `waitforTimeout: 30000`, `connectionRetryTimeout: 120000`, `connectionRetryCount: 0`
     - `onPrepare`: spawn `tauri-driver` with `stdio: [null, process.stdout, process.stderr]`; store in `tauriDriver`
     - `onComplete`: call `tauriDriver.kill()`
     - `framework: 'mocha'`, `reporters: ['spec']`
     - `mochaOpts: { ui: 'bdd', timeout: 60000 }`
   
   Reference the WebdriverIO Tauri documentation if the exact shape of `'tauri:options'` differs from the above.

4. **Create `e2e/specs/smoke.spec.ts`.** Single describe block, one test:
   ```
   describe('Smoke', () => {
     it('window title is Fade', async () => {
       const title = await browser.getTitle()
       expect(title).toBe('Fade')
     })
   })
   ```

5. **Add `test:e2e` script to `package.json`:**
   ```json
   "test:e2e": "wdio run e2e/wdio.conf.ts"
   ```

6. **Update `.github/workflows/ci.yml`.** After the existing `Backend unit tests` step, add these steps in order:
   - **Install tauri-driver:**
     ```yaml
     - name: Install tauri-driver
       run: cargo install tauri-driver
     ```
   - **Enable safaridriver:**
     ```yaml
     - name: Enable safaridriver
       run: sudo safaridriver --enable
     ```
   - **Build app with automation feature:**
     ```yaml
     - name: Build app (automation feature)
       run: cargo build --manifest-path src-tauri/Cargo.toml --features automation
     ```
   - **Run E2E smoke test:**
     ```yaml
     - name: E2E smoke test
       run: npm run test:e2e
     ```

7. **Verify locally:** Run `cargo build --manifest-path src-tauri/Cargo.toml --features automation` and confirm it compiles without errors. Then check that `src-tauri/target/debug/fade` exists.

8. **Run smoke test locally** (requires tauri-driver installed locally via `cargo install tauri-driver`): `npm run test:e2e`. Confirm it passes.

## Success signal
- `npm run test:e2e` exits 0 with "Smoke > window title is Fade: passed" in output.
- `.github/workflows/ci.yml` includes the four new steps and CI runs green.
- `e2e/wdio.conf.ts` and `e2e/specs/smoke.spec.ts` exist.
- `src-tauri/Cargo.toml` has a `[features]` section with `automation = ["tauri/automation"]`.

## Notes
- If `tauri-driver` on macOS errors with "safaridriver not found" or permission denied, `sudo safaridriver --enable` must be run first — include this in CI and in local setup notes.
- The app binary on macOS is `target/debug/fade` (not `target/debug/Fade.app`) for WebDriver purposes. If tauri-driver refuses to launch it and requires the .app bundle, adjust the path to `target/debug/Fade.app/Contents/MacOS/Fade` and note this in a comment in wdio.conf.ts.
- `cargo install tauri-driver` adds ~60s to CI on first run (no cache). That's acceptable for now.
- If TypeScript compilation of wdio.conf.ts fails, add `"ts-node": "^10"` to devDependencies and a `tsconfig.json` in `e2e/` that sets `"module": "commonjs"`.
