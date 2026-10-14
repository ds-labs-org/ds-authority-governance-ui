import { defineConfig, devices } from '@playwright/test';

// One worker: every spec shares one hub + registry (stateful), so specs use
// unique ids/DIDs instead of relying on a clean slate.
export default defineConfig({
  testDir: './tests',
  workers: 1,
  fullyParallel: false,
  retries: 0,
  forbidOnly: !!process.env.CI,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : [['list']],
  globalSetup: './harness/global-setup.mjs',
  globalTeardown: './harness/global-teardown.mjs',
  use: {
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    actionTimeout: 10_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
