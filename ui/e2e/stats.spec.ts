import { expect, test } from '@playwright/test'

// Rich stats dashboard tests. Model-agnostic; run against the same self-contained
// render-registry fixture the other specs use. The dashboard is driven by the full
// `weaver registry stats` output served at /api/v1/registry/stats.

test('stats dashboard renders KPIs, sections and charts', async ({ page }) => {
  await page.goto('/stats')

  await expect(page.getByRole('heading', { name: 'Registry Stats' })).toBeVisible()
  await expect(page.getByText(/Error loading registry stats/)).toHaveCount(0)

  // Five KPI tiles, each showing a non-negative count.
  const tiles = page.locator('a.stat')
  await expect(tiles).toHaveCount(5)
  for (const label of ['Attributes', 'Metrics', 'Spans', 'Events', 'Entities']) {
    const tile = page.locator('a.stat', { hasText: label })
    const value = await tile.locator('.stat-value').innerText()
    expect(Number.parseInt(value.replace(/,/g, ''), 10)).toBeGreaterThanOrEqual(0)
  }

  // Section headings for each analytical area.
  for (const heading of [
    'Stability & Deprecation',
    'Attributes',
    'Metrics',
    'Spans',
    'Entities',
    'Documentation Coverage',
  ]) {
    await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible()
  }

  // Recharts renders one SVG surface per chart card. There are seven charts;
  // wait for the first to paint, then assert the bulk are present.
  const surfaces = page.locator('.recharts-surface')
  await expect(surfaces.first()).toBeVisible()
  expect(await surfaces.count()).toBeGreaterThanOrEqual(6)
})

test('stats dashboard KPI matches the stats API', async ({ page, request }) => {
  const response = await request.get('/api/v1/registry/stats')
  expect(response.ok()).toBeTruthy()
  const stats = await response.json()

  // The full stats shape is served (not just counts).
  expect(stats.registry.attributes).toHaveProperty('attribute_type_breakdown')
  expect(stats.registry.metrics).toHaveProperty('instrument_breakdown')

  const apiAttributeCount = stats.registry.attributes.attribute_count as number

  await page.goto('/stats')
  const tileValue = await page
    .locator('a.stat', { hasText: 'Attributes' })
    .locator('.stat-value')
    .innerText()
  expect(Number.parseInt(tileValue.replace(/,/g, ''), 10)).toBe(apiAttributeCount)
})
