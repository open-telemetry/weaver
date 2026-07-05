import { defineConfig, devices } from '@playwright/test'

const env = (globalThis as any).process?.env as {
  WEAVER_BASE_URL?: string
  WEAVER_EXTERNAL_SERVER?: string
  CI?: string
}

// UI e2e tests run against the self-contained fixture registry in
// e2e/fixtures/render-registry (no live_check dependency). Port 8231 avoids
// reusing a local dev server on 8080.
const baseURL = env?.WEAVER_BASE_URL ?? 'http://127.0.0.1:8231'

// Set by the Docker publish job, which serves the mounted fixture itself.
const useExternalServer = !!env?.WEAVER_EXTERNAL_SERVER

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!env?.CI,
  retries: env?.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  // The first `cargo run` may compile, so allow a generous startup window.
  webServer: useExternalServer
    ? undefined
    : {
        command: 'cargo run -- serve -r ui/e2e/fixtures/render-registry --bind 127.0.0.1:8231',
        cwd: '..',
        url: `${baseURL}/health`,
        reuseExistingServer: !env?.CI,
        timeout: 180_000,
        stdout: 'pipe',
        stderr: 'pipe',
      },
})
