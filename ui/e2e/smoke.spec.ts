import { expect, test } from '@playwright/test'

// Foundational smoke tests: prove the embedded UI is built and interactive
// against a running `weaver serve` (binary or container). These assume the
// bundled crates/weaver_live_check/model registry is loaded.

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
    .fill('finding')

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
