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

test('an item whose name coincides with a namespace stays a sibling of that folder', async ({
  page,
}) => {
  // `render.attr` is both an event name and the folder holding the
  // `render.attr.*` attributes - a naming coincidence, not ownership, so
  // this event must stay a sibling of the folder rather than being nested
  // inside it. Collapsing the folder should not hide it.
  await page.goto('/search?view=tree')
  const event = page.locator('a[href="/event/render.attr"]')
  await expect(event).toBeVisible()

  const attrFolder = page.getByRole('button', { name: /^attr\s/ })
  await attrFolder.click()
  await expect(attrFolder).toHaveAttribute('aria-expanded', 'false')
  await expect(event).toBeVisible()
})

test('folders and items under a namespace sort together alphabetically', async ({ page }) => {
  // `render.coordinator` (an event) sorts alphabetically between the
  // `render.container` and `render.entity` namespace folders - folders and
  // items are siblings and must interleave in one alphabetical list rather
  // than rendering as a folders-then-items grouping.
  await page.goto('/search?view=tree')
  const renderFolder = page.getByRole('button', { name: /^render\s/ })
  await expect(renderFolder).toBeVisible()
  const renderChildren = renderFolder.locator('xpath=..').locator(':scope > ul > li')
  await expect(renderChildren.first()).toBeVisible()
  const labels = await renderChildren.locator(':scope > button, :scope > a').allTextContents()
  const container = labels.findIndex((label) => label.startsWith('container'))
  const coordinator = labels.findIndex((label) => label.includes('coordinator'))
  const entity = labels.findIndex((label) => label.startsWith('entity'))

  expect(container).toBeGreaterThanOrEqual(0)
  expect(coordinator).toBeGreaterThan(container)
  expect(entity).toBeGreaterThan(coordinator)
})

test('the view toggle round-trips through the URL', async ({ page }) => {
  await page.goto('/search?view=tree')
  await expect(page.getByRole('button', { name: 'Expand all' })).toBeVisible()

  await page.getByRole('button', { name: 'List', exact: true }).click()
  await expect(page).not.toHaveURL(/view=tree/)
  await expect(page.getByRole('button', { name: 'Expand all' })).toBeHidden()
})

test('collapsed tree state survives navigating to a leaf and back', async ({ page }) => {
  await page.goto('/search?view=tree')
  const leaf = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(leaf).toBeVisible()

  // Collapse to level 1 (a deliberate, non-default state) - a bulk action, so
  // it's a compact `base` param rather than every folder path enumerated.
  await page.getByRole('button', { name: '1', exact: true }).click()
  const attrFolder = page.getByRole('button', { name: /^attr\s/ })
  await expect(attrFolder).toBeVisible()
  await expect(leaf).toBeHidden()
  await expect(page).toHaveURL(/base=lvl1/)

  // A single per-folder toggle on top of that base is a small override.
  await attrFolder.click()
  await expect(leaf).toBeVisible()
  await expect(page).toHaveURL(/open=/)
  await leaf.click()
  await expect(page).toHaveURL(/\/attribute\/render\.attr\.string_single_example/)

  await page.goBack()
  await expect(page).toHaveURL(/view=tree/)
  // The manually-expanded `attr` folder survived, but its still-collapsed
  // sibling folders did not spuriously re-expand.
  await expect(leaf).toBeVisible()
})

test('scroll position survives navigating to a leaf and back', async ({ page }) => {
  // Only the results panel scrolls (the filters above stay put); a modest
  // viewport keeps it short enough for the (small) fixture tree to overflow.
  await page.setViewportSize({ width: 800, height: 500 })
  await page.goto('/search?view=tree')
  const leaf = page.locator('a[href="/attribute/render.attr.string_single_example"]')
  await expect(leaf).toBeVisible()

  const panel = page.locator('[data-scroll-restoration-id="search-results"]')
  // Wait for the actual 'scroll' event (not just the scrollTop change,
  // which happens synchronously) - the router's own listener only captures
  // the position once that event fires, and navigating away too soon would
  // race it, same as the click-auto-scroll gotcha below.
  await panel.evaluate(
    (el) =>
      new Promise<void>((resolve) => {
        el.addEventListener('scroll', () => resolve(), { once: true })
        el.scrollTo({ top: 60 })
      })
  )
  const scrolledTo = await panel.evaluate((el) => el.scrollTop)

  // Playwright's `.click()` always scrolls its target into view first to
  // compute real screen coordinates - even with `force: true` - which would
  // overwrite the position just set up above (the leaf is now off-screen).
  // A native in-page click leaves scroll untouched.
  await page.evaluate(() => {
    const link = document.querySelector<HTMLElement>(
      'a[href="/attribute/render.attr.string_single_example"]'
    )
    link?.click()
  })
  await expect(page).toHaveURL(/\/attribute\/render\.attr\.string_single_example/)

  await page.goBack()
  await expect(page).toHaveURL(/view=tree/)
  // The route fetches its data after mount (no route loader), so this also
  // guards against restoring too early - before the tree has re-rendered to
  // its full height - and silently landing back at the top.
  await expect.poll(() => panel.evaluate((el) => el.scrollTop)).toBe(scrolledTo)
})
