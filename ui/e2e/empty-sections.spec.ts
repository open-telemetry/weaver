import { spawn, type ChildProcess } from 'node:child_process'
import process from 'node:process'
import { expect, test } from '@playwright/test'

// The stats dashboard hides a signal type's section when that signal has no
// members. The shared render-registry fixture has every signal type, so this
// spec launches a second server against an attributes-only fixture and asserts
// the empty sections disappear.
//
// Skipped under WEAVER_EXTERNAL_SERVER (the Docker publish job), which serves a
// single fixed registry on a container and can't launch another weaver process.

const PORT = 8232
const BASE = `http://127.0.0.1:${PORT}`

async function waitForHealth(deadlineMs: number): Promise<void> {
  const deadline = Date.now() + deadlineMs
  for (;;) {
    try {
      const res = await fetch(`${BASE}/health`)
      if (res.ok) return
    } catch {
      // Server not accepting connections yet.
    }
    if (Date.now() > deadline) {
      throw new Error(`attributes-only server did not become healthy on ${BASE}`)
    }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }
}

test.describe('empty-section hiding', () => {
  test.skip(
    Boolean(process.env.WEAVER_EXTERNAL_SERVER),
    'cannot launch a second weaver server in the external-server (Docker) job'
  )

  let server: ChildProcess | undefined

  test.beforeAll(async () => {
    // The first `cargo run` may compile; allow a generous startup window.
    test.setTimeout(180_000)
    // `pnpm test:e2e` runs from ui/, so the repo root (weaver binary + fixtures)
    // is one level up — mirrors playwright.config.ts's webServer cwd.
    server = spawn(
      'cargo',
      [
        'run',
        '--',
        'serve',
        '-r',
        'ui/e2e/fixtures/attributes-only-registry',
        '--bind',
        `127.0.0.1:${PORT}`,
      ],
      { cwd: '..', stdio: 'ignore', detached: true }
    )
    await waitForHealth(170_000)
  })

  test.afterAll(() => {
    // Kill the whole process group (cargo + the weaver child it spawned).
    if (server?.pid) {
      try {
        process.kill(-server.pid, 'SIGTERM')
      } catch {
        // Already exited.
      }
    }
  })

  test('sections for empty signals are hidden, present ones remain', async ({ page }) => {
    // Confirm the fixture shape: attributes present, everything else empty.
    const res = await fetch(`${BASE}/api/v1/registry/stats`)
    expect(res.ok).toBeTruthy()
    const stats = await res.json()
    expect(stats.registry.attributes.attribute_count).toBeGreaterThan(0)
    for (const signal of ['metrics', 'spans', 'events', 'entities'] as const) {
      expect(stats.registry[signal].common.count).toBe(0)
    }

    await page.goto(`${BASE}/stats`)
    await expect(page.getByRole('heading', { name: 'Registry Stats' })).toBeVisible()

    // Signals that exist keep their sections.
    await expect(page.getByRole('heading', { name: 'Attributes', exact: true })).toBeVisible()
    await expect(
      page.getByRole('heading', { name: 'Stability & Deprecation', exact: true })
    ).toBeVisible()

    // Signals with zero members have no section at all.
    for (const hidden of ['Metrics', 'Spans', 'Entities', 'Documentation Coverage']) {
      await expect(page.getByRole('heading', { name: hidden, exact: true })).toHaveCount(0)
    }

    // The KPI overview still lists every signal type (including the empty ones).
    await expect(page.locator('a.stat')).toHaveCount(5)
  })
})
