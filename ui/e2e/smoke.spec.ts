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

test('deprecated items are hidden by default and controlled by hide/show/only buttons', async ({
  page,
}) => {
  await page.goto('/search')
  await expect(page.locator('a.card').first()).toBeVisible()

  // Verify the stability dropdown does not include Deprecated.
  const stabilitySelect = page.getByLabel('Filter by stability')
  await expect(stabilitySelect.locator('option[value="deprecated"]')).toHaveCount(0)

  const deprecatedCard = page.locator('a[href="/attribute/render.attr.deprecated_renamed"]')
  const nonDeprecatedCard = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(deprecatedCard).toHaveCount(0)
  await expect(nonDeprecatedCard).toBeVisible()

  const showButton = page.getByRole('button', { name: 'Show deprecated items' })
  const hideButton = page.getByRole('button', { name: 'Hide deprecated items' })
  const onlyButton = page.getByRole('button', { name: 'Only deprecated items' })
  await expect(hideButton).toHaveAttribute('aria-pressed', 'true')

  // Show deprecated items: both deprecated and non-deprecated items appear.
  await showButton.click()
  await expect(deprecatedCard).toBeVisible()
  await expect(nonDeprecatedCard).toBeVisible()
  await expect(hideButton).toHaveAttribute('aria-pressed', 'false')
  await expect(showButton).toHaveAttribute('aria-pressed', 'true')
  await expect(page).toHaveURL(/deprecated=show/)

  // Verify both stability badge (Stable) and Deprecated badge appear on the search card.
  await expect(deprecatedCard.locator('.badge', { hasText: 'Stable' })).toBeVisible()
  await expect(deprecatedCard.locator('.badge', { hasText: 'Deprecated' })).toBeVisible()

  // Only deprecated items: only deprecated items appear.
  await onlyButton.click()
  await expect(deprecatedCard).toBeVisible()
  await expect(nonDeprecatedCard).toHaveCount(0)
  await expect(onlyButton).toHaveAttribute('aria-pressed', 'true')
  await expect(page).toHaveURL(/deprecated=only/)

  // Combine Stability filter (Development) with Only deprecated items:
  // only render.attr.deprecated_obsoleted (Development + Deprecated) should appear.
  await stabilitySelect.selectOption('development')
  const obsoletedCard = page.locator('a[href="/attribute/render.attr.deprecated_obsoleted"]')
  await expect(obsoletedCard).toBeVisible()
  await expect(deprecatedCard).toHaveCount(0)
  await expect(obsoletedCard.locator('.badge', { hasText: 'Development' })).toBeVisible()
  await expect(obsoletedCard.locator('.badge', { hasText: 'Deprecated' })).toBeVisible()

  // Reset stability filter and click Hide deprecated items.
  await stabilitySelect.selectOption('')
  await hideButton.click()
  await expect(deprecatedCard).toHaveCount(0)
  await expect(nonDeprecatedCard).toBeVisible()
  await expect(hideButton).toHaveAttribute('aria-pressed', 'true')
  await expect(page).not.toHaveURL(/deprecated=/)
})

test('?deprecated=only query parameter and sort modes (deprecated, name, stability) work across pages and hide in tree view', async ({
  page,
  request,
}) => {
  await page.goto('/search?deprecated=only')

  const onlyButton = page.getByRole('button', { name: 'Only deprecated items' })
  await expect(onlyButton).toHaveAttribute('aria-pressed', 'true')

  const deprecatedCard = page.locator('a[href="/attribute/render.attr.deprecated_renamed"]')
  const nonDeprecatedCard = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(deprecatedCard).toBeVisible()
  await expect(nonDeprecatedCard).toHaveCount(0)

  // Verify backend sorts across the full result set before paginating (even with limit=1).
  const depPage1 = await request.get('/api/v1/registry/search?sort=deprecated&limit=1&offset=0')
  expect(depPage1.ok()).toBeTruthy()
  const depPage1Json = await depPage1.json()
  expect(depPage1Json.results[0].deprecated).toBeTruthy()

  // 1. Selecting "Sort: Deprecated first" from default view automatically shows deprecated items first.
  await page.goto('/search')
  const sortSelect = page.getByLabel('Sort by')
  await sortSelect.selectOption('deprecated')
  await expect(page).toHaveURL(/sort=deprecated/)
  await expect(page).toHaveURL(/deprecated=show/)
  await expect(page.locator('a.card').first()).toHaveAttribute('href', /deprecated/)

  // 2. Selecting "Sort: Name (A–Z)" sorts results alphabetically (render.attr event comes first, followed by render.attr.boolean_example).
  await page.getByRole('button', { name: 'Hide deprecated items' }).click()
  await sortSelect.selectOption('name')
  await expect(page).toHaveURL(/sort=name/)
  await expect(page.locator('a.card').first()).toHaveAttribute('href', '/event/render.attr')
  await expect(page.locator('a.card').nth(1)).toHaveAttribute(
    'href',
    '/attribute/render.attr.boolean_example'
  )
  const nameKeys = await page.locator('a.card .font-mono').allInnerTexts()
  expect(nameKeys.length).toBeGreaterThan(2)
  for (let i = 1; i < nameKeys.length; i++) {
    expect(nameKeys[i - 1] <= nameKeys[i]).toBeTruthy()
  }

  // 3. Selecting "Sort: Stability" puts Stable items first and Development items last.
  await sortSelect.selectOption('stability')
  await expect(page).toHaveURL(/sort=stability/)
  await expect(page.locator('a.card').first().locator('.badge', { hasText: 'Stable' })).toBeVisible()
  await expect(
    page.locator('a.card').last().locator('.badge', { hasText: 'Development' })
  ).toBeVisible()

  // 4. Switching to Tree view hides the "Sort by" dropdown.
  await page.getByRole('button', { name: 'Tree' }).click()
  await expect(page.getByLabel('Sort by')).toHaveCount(0)
})

test('stats page shows counts and links into filtered search including deprecated items', async ({
  page,
}) => {
  await page.goto('/stats')

  await expect(page.getByRole('heading', { name: 'Registry Stats' })).toBeVisible()

  // The Attributes stat card links to /search?type=attribute (ui/src/routes/stats.tsx).
  const attributesCard = page.locator('a.stat', { hasText: 'Attributes' })
  await expect(attributesCard).toBeVisible()

  const countText = await attributesCard.locator('.stat-value').innerText()
  expect(Number.parseInt(countText, 10)).toBeGreaterThan(0)

  await attributesCard.click()
  await expect(page).toHaveURL(/\/search\?.*type=attribute/)

  // Deprecated items subtitle links to /search?deprecated=only.
  await page.goto('/stats')
  const deprecatedLink = page.locator('a', { hasText: /deprecated definitions in total/ })
  await expect(deprecatedLink).toBeVisible()
  await deprecatedLink.click()
  await expect(page).toHaveURL(/\/search\?.*deprecated=only/)
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
