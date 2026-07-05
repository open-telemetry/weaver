import { expect, test } from '@playwright/test'

// App-shell smoke tests (search, stats, API docs) — the pages render.spec.ts
// doesn't cover. Model-agnostic; run against the same fixture registry.

test('app loads and search renders results', async ({ page }) => {
  // Root redirects to /search (see ui/src/routes/index.tsx).
  await page.goto('/')
  await expect(page).toHaveURL(/\/search/)

  await expect(page.getByRole('heading', { name: 'Search' })).toBeVisible()
  await expect(
    page.getByPlaceholder(/Search attributes, metrics, spans/)
  ).toBeVisible()

  // Empty search auto-runs on mount and lists all items.
  await expect(page.getByText(/Showing \d+ of \d+ items/)).toBeVisible()
  await expect(page.locator('a.card').first()).toBeVisible()

  // No error alert should be present.
  await expect(page.getByText(/^Error:/)).toHaveCount(0)
})

test('searching and clicking a result opens a detail page', async ({ page }) => {
  await page.goto('/search')

  await page
    .getByPlaceholder(/Search attributes, metrics, spans/)
    .fill('counter')

  // Wait for the (debounced) results to settle, then open the first card.
  const firstCard = page.locator('a.card').first()
  await expect(firstCard).toBeVisible()
  const cardKey = await firstCard.locator('.font-mono').first().innerText()
  await firstCard.click()

  // Lands on a typed detail route whose heading echoes the item key.
  await expect(page).toHaveURL(/\/(attribute|metric|span|event|entity)\//)
  await expect(
    page.getByRole('heading', { name: cardKey, exact: true })
  ).toBeVisible()
})

test('stats page shows counts and links into filtered search', async ({ page }) => {
  await page.goto('/stats')

  await expect(page.getByRole('heading', { name: 'Registry Stats' })).toBeVisible()

  // The Attributes stat card links to /search?type=attribute (ui/src/routes/stats.tsx).
  const attributesCard = page.locator('a.stat', { hasText: 'Attributes' })
  await expect(attributesCard).toBeVisible()

  const countText = await attributesCard.locator('.stat-value').innerText()
  expect(Number.parseInt(countText, 10)).toBeGreaterThan(0)

  await attributesCard.click()
  await expect(page).toHaveURL(/\/search\?.*type=attribute/)
})

test('API docs render the Swagger UI for the OpenAPI spec', async ({ page }) => {
  await page.goto('/api-docs')

  // Swagger UI mounts and the spec loads.
  await expect(page.locator('.swagger-ui .info .title')).toContainText('Weaver API')
  await expect(page.getByText('Failed to load API definition')).toHaveCount(0)

  // Operations and the Schemas (models) section render.
  await expect(page.locator('.swagger-ui .opblock').first()).toBeVisible()
  await expect(page.locator('.swagger-ui section.models')).toBeVisible()
})

test('Schemas section stays toggleable after navigating away and back', async ({ page }) => {
  // Regression: swagger-ui-react breaks on remount, so docs stay mounted in
  // AppLayout; navigating away and back must not break the Schemas toggle.
  await page.goto('/api-docs')

  const models = page.locator('.swagger-ui section.models')
  await expect(models).toBeVisible()

  // Client-side navigation away and back via the sidebar (not a full reload).
  await page.getByRole('button', { name: 'Search', exact: true }).click()
  await expect(page).toHaveURL(/\/search/)
  await page.getByRole('button', { name: 'API Documentation', exact: true }).click()
  await expect(page).toHaveURL(/\/api-docs/)

  // The section starts expanded; the toggle must still collapse and re-expand it.
  const toggle = models.getByRole('button', { name: 'Schemas', exact: true })
  await expect(models).toHaveClass(/is-open/)
  await toggle.click()
  await expect(models).not.toHaveClass(/is-open/)
  await toggle.click()
  await expect(models).toHaveClass(/is-open/)
})
