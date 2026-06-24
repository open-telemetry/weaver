import { defineConfig, devices } from '@playwright/test'

// Where the UI under test is served. Defaults to the `weaver serve` default bind.
const baseURL = process.env.WEAVER_BASE_URL ?? 'http://127.0.0.1:8080'

// When the server is managed externally (e.g. a Docker container started by CI),
// set WEAVER_EXTERNAL_SERVER=1 so Playwright does not try to start one itself.
const useExternalServer = !!process.env.WEAVER_EXTERNAL_SERVER

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  // Start `weaver serve` against the bundled live_check model unless a server is
  // already provided externally. The first `cargo run` may compile, so allow a
  // generous startup window and reuse an already-running dev server locally.
  webServer: useExternalServer
    ? undefined
    : {
        command: 'cargo run -- serve -r crates/weaver_live_check/model',
        cwd: '..',
        url: `${baseURL}/health`,
        reuseExistingServer: !process.env.CI,
        timeout: 180_000,
        stdout: 'pipe',
        stderr: 'pipe',
      },
})
