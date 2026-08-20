import { expect, test, type Locator, type Page } from '@playwright/test'

// Detail-page render tests against the fixture registry (e2e/fixtures/
// render-registry). A crawl visits every item to catch render-time crashes,
// plus targeted per-branch checks.

interface SearchResult {
  result_type: 'attribute' | 'metric' | 'span' | 'event' | 'entity'
  key?: string
  name?: string
  type?: unknown
}

interface DetailTarget {
  url: string
  id: string
}

function detailTargetFor(result: SearchResult): DetailTarget {
  switch (result.result_type) {
    case 'attribute':
      return { url: `/attribute/${result.key}`, id: String(result.key) }
    case 'metric':
      return { url: `/metric/${result.name}`, id: String(result.name) }
    case 'span':
      return { url: `/span/${result.type}`, id: String(result.type) }
    case 'event':
      return { url: `/event/${result.name}`, id: String(result.name) }
    case 'entity':
      return { url: `/entity/${result.type}`, id: String(result.type) }
  }
}

// Records uncaught page exceptions and console.error output; empty means clean.
function collectPageErrors(page: Page): string[] {
  const errors: string[] = []
  page.on('pageerror', (err) => {
    errors.push(`[${page.url()}] pageerror: ${err.message}`)
  })
  page.on('console', (msg) => {
    if (msg.type() !== 'error') return
    const text = msg.text()
    if (/favicon/i.test(text)) return
    errors.push(`[${page.url()}] console.error: ${text}`)
  })
  return errors
}

// Locates a table row containing the given attribute key, scoped to a card.
function attributeRow(card: Locator, attributeKey: string): Locator {
  return card.getByRole('row').filter({ hasText: attributeKey })
}

test.describe('registry crawl', () => {
  test('every item has a detail page that renders without runtime errors', async ({
    page,
    request,
  }) => {
    const response = await request.get('/api/v1/registry/search?limit=1000')
    expect(response.ok()).toBeTruthy()
    const body = (await response.json()) as { results: SearchResult[] }
    const results = body.results
    // Sanity: the fixture is non-trivial and covers all five signal types.
    expect(results.length).toBeGreaterThan(20)
    const kinds = new Set(results.map((r) => r.result_type))
    for (const kind of ['attribute', 'metric', 'span', 'event', 'entity'] as const) {
      expect(kinds).toContain(kind)
    }

    const errors = collectPageErrors(page)

    for (const result of results) {
      const { url, id } = detailTargetFor(result)
      await page.goto(url)

      // Heading echoes the item id: rendered past the loading spinner.
      await expect(page.getByRole('heading', { name: id, exact: true, level: 1 })).toBeVisible()
      await expect(page.getByText(/^Error:/)).toHaveCount(0)
    }

    expect(errors, `Unexpected console/page errors:\n${errors.join('\n')}`).toEqual([])
  })
})

test.describe('attribute rendering', () => {
  test('a single scalar example renders as a list item', async ({ page }) => {
    await page.goto('/attribute/render.attr.string_single_example')
    await expect(page.getByRole('heading', { name: 'Examples' })).toBeVisible()
    await expect(page.getByText('a single scalar string example')).toBeVisible()
    await expect(page.getByText('No examples available.')).toHaveCount(0)
  })

  test('multiple examples render as separate list items', async ({ page }) => {
    await page.goto('/attribute/render.attr.string_multi_example')
    for (const example of ['first', 'second', 'third']) {
      await expect(page.getByText(`"${example}"`)).toBeVisible()
    }
  })

  test('an attribute with no examples shows the empty fallback', async ({ page }) => {
    await page.goto('/attribute/render.attr.markdown')
    await expect(page.getByText('No examples available.')).toBeVisible()
  })

  test('an enum type renders its members in a table', async ({ page }) => {
    await page.goto('/attribute/render.attr.enum_type')
    await expect(page.getByText('enum { GET, POST, PUT }')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Enum Values' })).toBeVisible()
    for (const value of ['GET', 'POST', 'PUT']) {
      await expect(page.getByRole('cell', { name: value, exact: true })).toBeVisible()
    }
  })

  test('markdown in brief and note is rendered', async ({ page }) => {
    await page.goto('/attribute/render.attr.markdown')
    // Links from the brief and note markdown render as anchors.
    await expect(page.getByRole('link', { name: 'link' }).first()).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Note' })).toBeVisible()
    await expect(page.getByRole('listitem').filter({ hasText: 'a bulleted list item' })).toBeVisible()
  })

  test('a renamed-deprecated attribute links to its successor', async ({ page }) => {
    await page.goto('/attribute/render.attr.deprecated_renamed')
    await expect(page.getByRole('heading', { name: 'render.attr.deprecated_renamed', level: 1 })).toBeVisible()
    // Deprecation alert with a link to the successor attribute.
    const alert = page.getByRole('alert').filter({ hasText: 'Deprecated' })
    await expect(alert).toBeVisible()
    // The fixture omits the note, it is inferred from `renamed_to`.
    await expect(alert.getByText('Replaced by `render.attr.string_single_example`.')).toBeVisible()
    const successor = alert.getByRole('link', { name: 'render.attr.string_single_example' })
    await expect(successor).toBeVisible()
    await successor.click()
    await expect(page).toHaveURL(/\/attribute\/render\.attr\.string_single_example$/)
  })

  test('an obsoleted-deprecated attribute shows a note but no successor link', async ({ page }) => {
    await page.goto('/attribute/render.attr.deprecated_obsoleted')
    const alert = page.getByRole('alert').filter({ hasText: 'Deprecated' })
    await expect(alert).toBeVisible()
    await expect(alert.getByText('This attribute no longer exists and has no replacement.')).toBeVisible()
    await expect(alert.getByRole('link')).toHaveCount(0)
  })

  // Stability badges: each fixture attribute pins a different level.
  const stabilityCases: Array<{ key: string; label: string }> = [
    { key: 'render.attr.string_single_example', label: 'Stable' },
    { key: 'render.attr.string_multi_example', label: 'Development' },
    { key: 'render.attr.int_single_example', label: 'Alpha' },
    { key: 'render.attr.int_multi_example', label: 'Beta' },
    { key: 'render.attr.double_example', label: 'Release Candidate' },
    { key: 'render.attr.stability_deprecated', label: 'Deprecated' },
  ]
  for (const { key, label } of stabilityCases) {
    test(`stability badge renders "${label}"`, async ({ page }) => {
      await page.goto(`/attribute/${key}`)
      await expect(page.getByText(label, { exact: true })).toBeVisible()
    })
  }
})

test.describe('metric rendering', () => {
  const instruments: Array<{ name: string; instrument: string; unit: string }> = [
    { name: 'render.metric.counter', instrument: 'counter', unit: '{request}' },
    { name: 'render.metric.histogram', instrument: 'histogram', unit: 's' },
    { name: 'render.metric.gauge', instrument: 'gauge', unit: 'By' },
    { name: 'render.metric.updowncounter', instrument: 'updowncounter', unit: '1' },
  ]
  for (const { name, instrument, unit } of instruments) {
    test(`${instrument} instrument and unit render`, async ({ page }) => {
      await page.goto(`/metric/${name}`)
      await expect(page.getByRole('heading', { name: 'Instrument' })).toBeVisible()
      await expect(page.getByText(instrument, { exact: true })).toBeVisible()
      await expect(page.getByRole('heading', { name: 'Unit' })).toBeVisible()
      await expect(page.getByText(unit, { exact: true })).toBeVisible()
    })
  }

  test('metric attribute table shows type and every requirement level', async ({ page }) => {
    await page.goto('/metric/render.metric.counter')
    const card = page.locator('.card', { has: page.getByRole('heading', { name: 'Metric Attributes' }) })

    // Type column is populated (regression: it read the wrong field and was blank).
    const typeByAttr: Record<string, string> = {
      'render.attr.string_single_example': 'string',
      'render.attr.int_single_example': 'int',
      'render.attr.boolean_example': 'boolean',
      'render.attr.double_example': 'double',
      'render.attr.enum_type': 'enum',
    }
    for (const [attr, type] of Object.entries(typeByAttr)) {
      await expect(attributeRow(card, attr).getByRole('cell', { name: type, exact: true })).toBeVisible()
    }

    // Requirement level badges.
    await expect(attributeRow(card, 'render.attr.string_single_example').getByText('required', { exact: true })).toBeVisible()
    await expect(attributeRow(card, 'render.attr.int_single_example').getByText('recommended', { exact: true })).toBeVisible()
    await expect(attributeRow(card, 'render.attr.boolean_example').getByText('opt_in', { exact: true })).toBeVisible()
    await expect(attributeRow(card, 'render.attr.double_example').getByText('conditionally required', { exact: true })).toBeVisible()
  })

  test('nested entity associations render as a boolean expression', async ({ page }) => {
    await page.goto('/metric/render.metric.nested_association')
    const card = page.locator('.card', { has: page.getByRole('heading', { name: 'Entity Associations' }) })
    await expect(card).toBeVisible()
    // tenant AND (host OR container)
    await expect(card.getByRole('link', { name: 'render.entity.tenant' })).toBeVisible()
    await expect(card.getByRole('link', { name: 'render.entity.host' })).toBeVisible()
    await expect(card.getByRole('link', { name: 'render.entity.container' })).toBeVisible()
    await expect(card.getByText('and', { exact: true })).toBeVisible()
    await expect(card.getByText('or', { exact: true })).toBeVisible()
  })
})

test.describe('span rendering', () => {
  const kinds = ['client', 'server', 'internal', 'producer', 'consumer']
  for (const kind of kinds) {
    test(`${kind} span kind renders`, async ({ page }) => {
      await page.goto(`/span/render.span.${kind}`)
      await expect(page.getByRole('heading', { name: `render.span.${kind}`, level: 1 })).toBeVisible()
      await expect(page.getByText(kind, { exact: true })).toBeVisible()
    })
  }

  test('sampling-relevant attribute shows a sampling badge and typed rows', async ({ page }) => {
    await page.goto('/span/render.span.client')
    const card = page.locator('.card', { has: page.getByRole('heading', { name: 'Span Attributes' }) })
    await expect(card).toBeVisible()
    await expect(attributeRow(card, 'render.attr.string_single_example').getByText('sampling', { exact: true })).toBeVisible()
    // Span attribute type column (already correct for spans) is populated.
    await expect(attributeRow(card, 'render.attr.enum_type').getByRole('cell', { name: 'enum', exact: true })).toBeVisible()
  })
})

test.describe('event rendering', () => {
  test('event attribute table renders with requirement levels', async ({ page }) => {
    await page.goto('/event/render.event.thing')
    const card = page.locator('.card', { has: page.getByRole('heading', { name: 'Event Attributes' }) })
    await expect(card).toBeVisible()
    await expect(attributeRow(card, 'render.attr.string_single_example').getByText('required', { exact: true })).toBeVisible()
    await expect(attributeRow(card, 'render.attr.boolean_example').getByText('opt_in', { exact: true })).toBeVisible()
  })

  test('event entity associations render top-level one_of links', async ({ page }) => {
    await page.goto('/event/render.event.thing')
    const card = page.locator('.card', { has: page.getByRole('heading', { name: 'Entity Associations' }) })
    await expect(card.getByRole('link', { name: 'render.entity.host' })).toBeVisible()
    await expect(card.getByRole('link', { name: 'render.entity.tenant' })).toBeVisible()
  })
})

test.describe('entity rendering', () => {
  test('identity and description attribute tables render with types', async ({ page }) => {
    await page.goto('/entity/render.entity.host')

    const identity = page.locator('.card', { has: page.getByRole('heading', { name: 'Identity Attributes' }) })
    await expect(identity).toBeVisible()
    await expect(attributeRow(identity, 'render.host.id').getByRole('cell', { name: 'string', exact: true })).toBeVisible()

    const description = page.locator('.card', { has: page.getByRole('heading', { name: 'Description Attributes' }) })
    await expect(description).toBeVisible()
    await expect(attributeRow(description, 'render.host.name').getByRole('cell', { name: 'string', exact: true })).toBeVisible()
  })

  test('an entity with only identity attributes omits the description table', async ({ page }) => {
    await page.goto('/entity/render.entity.tenant')
    await expect(page.getByRole('heading', { name: 'Identity Attributes' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Description Attributes' })).toHaveCount(0)
  })
})
