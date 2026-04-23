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
