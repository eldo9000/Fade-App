# TASK-1: Bootstrap Playwright Component Testing harness

## Goal
`npm run test:e2e` runs a Playwright component test that mounts `DataOptions.svelte` in a real Chromium browser, asserts it renders without errors, and CI on macOS-14 passes green with this step included.

## Context
Fade is a Tauri 2 + Svelte 5 desktop app. The goal is systematic UI testing of the options panels. `tauri-driver` does not support macOS in Tauri 2.x, so WebdriverIO is not viable. Instead, use **Playwright Component Testing** (`@playwright/experimental-ct-svelte`), which mounts Svelte components in a real browser without requiring the Tauri runtime.

**Why this works:** All six option components (ImageOptions, VideoOptions, AudioOptions, DataOptions, ArchiveOptions, FormatPicker) are pure presentational components — they declare `options = $bindable()` in `$props()` and have zero imports from `@tauri-apps/api` or any global stores. They can be mounted and tested in complete isolation.

**How Playwright CT works:**
1. Tests import `{ mount, test, expect }` from `@playwright/experimental-ct-svelte`.
2. `mount(ComponentName, { props: { ... } })` renders the component in a real browser page.
3. The returned object is a Playwright `Locator` rooted at the component's element.
4. Standard Playwright assertions (`toBeVisible`, `toHaveValue`, etc.) work normally.

**No Tauri mock is needed for the option components.** Tauri API calls live in `App.svelte` only.

**Existing CI:** `.github/workflows/ci.yml` runs on `macos-14`, node 22, Rust stable. E2E step goes after all existing test steps.

## In scope
- `package.json` — add `@playwright/experimental-ct-svelte`, `@playwright/test` as devDependencies; add `test:e2e` script
- `playwright-ct.config.ts` — new file at repo root
- `playwright/index.ts` — new file (required CT setup entry point; can be minimal)
- `e2e/specs/smoke.spec.ts` — new file (one CT smoke test)
- `.github/workflows/ci.yml` — add two new steps: install Playwright Chromium, run e2e

## Out of scope
- Any changes to `src/` Svelte source files
- Any changes to `src-tauri/` Rust source files
- Any changes to existing vitest tests in `src/tests/`
- Writing any test specs beyond the single smoke test
- Mocking `@tauri-apps/api` (not needed for option components)

## Steps

1. **Read `package.json`** to see current devDependencies and scripts before editing.

2. **Install Playwright packages.** From the repo root:
   ```
   npm install --save-dev @playwright/experimental-ct-svelte @playwright/test
   ```
   Then install the Chromium browser binary:
   ```
   npx playwright install chromium
   ```
   Verify the packages appear in `package.json` devDependencies.

3. **Create `playwright/index.ts`** (required by Playwright CT as the mounting context entry point). This file can be empty or contain a minimal comment:
   ```ts
   // Playwright CT setup — no global context needed for Fade option components.
   ```

4. **Create `playwright-ct.config.ts`** at the repo root:
   ```ts
   import { defineConfig, devices } from '@playwright/experimental-ct-svelte'

   export default defineConfig({
     testDir: 'e2e',
     snapshotDir: 'e2e/__snapshots__',
     timeout: 10_000,
     fullyParallel: true,
     forbidOnly: !!process.env.CI,
     retries: process.env.CI ? 2 : 0,
     workers: process.env.CI ? 1 : undefined,
     reporter: [['list'], ['html', { open: 'never' }]],
     use: {
       ctPort: 3100,
       ctSetupFilePath: './playwright/index.ts',
       trace: 'on-first-retry',
     },
     projects: [
       {
         name: 'chromium',
         use: { ...devices['Desktop Chrome'] },
       },
     ],
   })
   ```

5. **Create `e2e/specs/smoke.spec.ts`:**
   ```ts
   import { test, expect } from '@playwright/experimental-ct-svelte'
   import DataOptions from '../../src/lib/DataOptions.svelte'

   test('DataOptions mounts and renders', async ({ mount }) => {
     const component = await mount(DataOptions, {
       props: {
         options: {
           output_format: 'csv',
           csv_delimiter: ',',
           json_pretty: false,
         } as any,
       },
     })
     await expect(component).toBeVisible()
   })
   ```

6. **Add `test:e2e` script to `package.json`:**
   ```json
   "test:e2e": "playwright test --config playwright-ct.config.ts"
   ```

7. **Update `.github/workflows/ci.yml`.** After the existing `Backend unit tests` step, add:
   ```yaml
   - name: Install Playwright Chromium
     run: npx playwright install --with-deps chromium

   - name: E2E component tests
     run: npm run test:e2e
   ```

8. **Run `npm run test:e2e` locally** and confirm the smoke test passes. Fix any import or configuration errors before returning.

## Success signal
- `npm run test:e2e` exits 0 with "DataOptions mounts and renders: passed" in output.
- `playwright-ct.config.ts`, `playwright/index.ts`, `e2e/specs/smoke.spec.ts` all exist.
- `package.json` devDependencies includes `@playwright/experimental-ct-svelte` and `@playwright/test`, and scripts includes `test:e2e`.
- `.github/workflows/ci.yml` has the two new steps.

## Notes
- If `mount` fails with a Vite/Svelte compilation error, check that `@sveltejs/vite-plugin-svelte` is already installed (it is — it's in devDependencies). Playwright CT uses it automatically.
- If the test fails with "Cannot find module '../../src/lib/DataOptions.svelte'", adjust the import path relative to the spec file location.
- Svelte 5 components with `$props()` runes are fully supported by Playwright CT as long as the Svelte Vite plugin handles them (it does).
- `as any` on the props object is intentional for the smoke test — full ConvertOptions shape is tested in later tasks.
- Do NOT run `npx playwright install --with-deps` locally (it installs OS-level deps that may conflict). Just `npx playwright install chromium` locally.
