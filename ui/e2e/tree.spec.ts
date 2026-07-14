import { expect, test } from '@playwright/test'

// Namespace tree view on the search page (ui/src/components/NamespaceTree.tsx).
// The fixture registry's names all live under `render.*`, and the whole result
// set is small enough that the tree auto-expands when it loads.

test('tree view groups results by namespace and links to detail pages', async ({ page }) => {
  await page.goto('/search')
  await page.getByRole('button', { name: 'Tree', exact: true }).click()
  await expect(page).toHaveURL(/view=tree/)

  // Root namespace folder is present (its accessible name includes the count badge).
  await expect(page.getByRole('button', { name: /^render\s/ })).toBeVisible()

  // Auto-expanded: a leaf deep in the tree links to its detail page.
  const leaf = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(leaf).toBeVisible()
  await leaf.click()
  await expect(page).toHaveURL(/\/attribute\/render\.attr\.string_single_example/)
})

test('tree view expand and collapse controls work', async ({ page }) => {
  await page.goto('/search?view=tree')
  const leaf = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(leaf).toBeVisible()

  await page.getByRole('button', { name: 'Collapse all' }).click()
  await expect(leaf).toBeHidden()

  // Level 1 opens only the root namespaces: `render.attr` becomes a visible
  // folder, but the leaves inside it stay hidden.
  await page.getByRole('button', { name: '1', exact: true }).click()
  await expect(page.getByRole('button', { name: /^attr\s/ })).toBeVisible()
  await expect(leaf).toBeHidden()

  await page.getByRole('button', { name: 'Expand all' }).click()
  await expect(leaf).toBeVisible()
})

test('the view toggle round-trips through the URL', async ({ page }) => {
  await page.goto('/search?view=tree')
  await expect(page.getByRole('button', { name: 'Expand all' })).toBeVisible()

  await page.getByRole('button', { name: 'List', exact: true }).click()
  await expect(page).not.toHaveURL(/view=tree/)
  await expect(page.getByRole('button', { name: 'Expand all' })).toBeHidden()
})
