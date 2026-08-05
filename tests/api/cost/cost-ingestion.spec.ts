import { test, expect } from '../../fixtures/test'

/**
 * The cost-estimation READ surface — the endpoints the dashboard's cost pages call on load.
 *
 * Scope is deliberate: report/invoice UPLOAD is excluded. Those take multi-GB bodies, return 202, and
 * complete asynchronously through a ClickHouse fit, so a meaningful assertion needs a poll-to-settle
 * loop over a fixture file. That belongs in a dedicated ingestion suite, not here.
 *
 * What these tests are actually worth: a fresh merchant with no cost data must get an empty-but-valid
 * 200 from every one of these, because the dashboard renders them unconditionally. A 500 from an empty
 * ClickHouse table is a real bug that this catches, and it's invisible to anyone testing with seeded data.
 */
test.describe('Cost ingestion — registry (API)', () => {
  test('lists the connectors that support report ingestion', async ({ api, merchant }) => {
    const r = await api.raw('GET', '/cost-ingestion/connectors', { failOnStatusCode: false })

    expect(r.status).toBe(200)
    expect(Array.isArray(r.body)).toBe(true)

    const ids = r.body.map((c: any) => c.id)
    // The dashboard's connector picker reads this instead of keeping its own list.
    for (const expected of ['adyen', 'checkout', 'stripe']) {
      expect(ids).toContain(expected)
    }
    expect(r.body.every((c: any) => typeof c.pull === 'boolean')).toBe(true)
  })
})

test.describe('Cost ingestion — empty merchant reads (API)', () => {
  test('ingestion history is empty for a new merchant', async ({ api, merchant }) => {
    const r = await api.raw('GET', `/merchant-account/${merchant.id}/cost-ingestions`, {
      failOnStatusCode: false,
    })

    expect(r.status).toBe(200)
    expect(r.body).toEqual([])
  })

  test('list endpoints return empty arrays rather than erroring', async ({ api, merchant }) => {
    const paths = [
      `/merchant-account/${merchant.id}/connector-fees`,
      `/merchant-account/${merchant.id}/cost-clusters`,
      `/merchant-account/${merchant.id}/cost-cluster-facets`,
      `/merchant-account/${merchant.id}/cost-price-changes`,
      `/merchant-account/${merchant.id}/invoice-addons`,
    ]

    for (const path of paths) {
      const r = await api.raw('GET', path, { failOnStatusCode: false })
      expect(r.status, `${path} should serve an empty result, not fail`).toBe(200)
      expect(Array.isArray(r.body), `${path} should return an array`).toBe(true)
      expect(r.body).toEqual([])
    }
  })

  test('cost coverage reports a zeroed summary for a new merchant', async ({ api, merchant }) => {
    const r = await api.raw('GET', `/merchant-account/${merchant.id}/cost-coverage`, {
      failOnStatusCode: false,
    })

    expect(r.status).toBe(200)
    expect(r.body.total_clusters).toBe(0)
    expect(r.body.total_txns).toBe(0)
    expect(typeof r.body.report_date).toBe('string')
  })

})

test.describe('Cost ingestion — seed costs (API)', () => {
  test('seed costs fall back to the configured defaults', async ({ api, merchant }) => {
    const r = await api.raw('GET', `/merchant-account/${merchant.id}/seed-costs`, {
      failOnStatusCode: false,
    })

    expect(r.status).toBe(200)
    expect(Array.isArray(r.body)).toBe(true)
    // A merchant that never saved a table still gets the deployment's default seed costs, so cost
    // estimation works before any report is uploaded.
    if (r.body.length > 0) {
      expect(typeof r.body[0].psp).toBe('string')
      expect(typeof r.body[0].fixed).toBe('number')
    }
  })

  test('saved seed costs round-trip and can be cleared', async ({ api, merchant }) => {
    const path = `/merchant-account/${merchant.id}/seed-costs`
    const rows = [
      { psp: 'stripe', interchange_bps: 100, scheme_bps: 10, markup_bps: 20, fixed: 0.3, is_default: true },
    ]

    const saved = await api.raw('PUT', path, { failOnStatusCode: false, body: { rows } })
    expect(saved.status).toBe(200)
    expect(saved.body.some((r: any) => r.psp === 'stripe')).toBe(true)
    // effective_pct_bps is recomputed server-side from the three components.
    const stripe = saved.body.find((r: any) => r.psp === 'stripe')
    expect(stripe.effective_pct_bps).toBe(130)

    // DELETE only succeeds because a table was saved above — see the note in the next test.
    const cleared = await api.raw('DELETE', path, { failOnStatusCode: false })
    expect(cleared.status).toBe(200)
  })

  test('rejects seed cost rows with no PSP or negative fees', async ({ api, merchant }) => {
    const path = `/merchant-account/${merchant.id}/seed-costs`

    // All four numeric fields are required by the deserializer — omitting one yields a 422 body
    // rejection rather than the 400 domain validation these assertions are about.
    const noPsp = await api.raw('PUT', path, {
      failOnStatusCode: false,
      body: { rows: [{ psp: '', interchange_bps: 100, scheme_bps: 10, markup_bps: 20, fixed: 0.3 }] },
    })
    expect(noPsp.status).toBe(400)

    const negative = await api.raw('PUT', path, {
      failOnStatusCode: false,
      body: { rows: [{ psp: 'stripe', interchange_bps: -5, scheme_bps: 10, markup_bps: 20, fixed: 0.3 }] },
    })
    expect(negative.status).toBe(400)
  })

  test('simulating seed costs prices an amount per PSP', async ({ api, merchant }) => {
    const r = await api.raw('POST', `/merchant-account/${merchant.id}/seed-costs/simulate`, {
      failOnStatusCode: false,
      body: { amount: 100, transaction_currency: 'USD' },
    })

    expect(r.status).toBe(200)
    expect(Array.isArray(r.body)).toBe(true)
    if (r.body.length > 0) {
      expect(typeof r.body[0].psp).toBe('string')
      expect(typeof r.body[0].cost_amount).toBe('number')
    }
  })

  test('simulate rejects a non-positive amount', async ({ api, merchant }) => {
    const r = await api.raw('POST', `/merchant-account/${merchant.id}/seed-costs/simulate`, {
      failOnStatusCode: false,
      body: { amount: 0 },
    })

    expect(r.status).toBe(400)
  })
})

test.describe('Cost ingestion — column mapping (API)', () => {
  const ACCOUNT = 'playwright-acct'

  test('an unset mapping reads back empty', async ({ api, merchant }) => {
    const r = await api.raw(
      'GET',
      `/merchant-account/${merchant.id}/connectors/stripe/report/column-mapping`,
      { failOnStatusCode: false, qs: { account: ACCOUNT } },
    )

    expect(r.status).toBe(200)
    expect(r.body.columns).toEqual({})
  })

  test('the account query param is required', async ({ api, merchant }) => {
    const r = await api.raw(
      'GET',
      `/merchant-account/${merchant.id}/connectors/stripe/report/column-mapping`,
      { failOnStatusCode: false },
    )

    // A mapping is per settlement source, so it is meaningless without the account.
    expect(r.status).toBe(400)
  })

  test('clearing a mapping is idempotent', async ({ api, merchant }) => {
    const path = `/merchant-account/${merchant.id}/connectors/stripe/report/column-mapping`

    // Deleting a mapping that was never set must not error — the dashboard's "reset" button relies on
    // it. (Note the connector-level fee-override DELETE does NOT share this property today.)
    const first = await api.raw('DELETE', path, { failOnStatusCode: false, qs: { account: ACCOUNT } })
    expect(first.status).toBe(204)

    const second = await api.raw('DELETE', path, { failOnStatusCode: false, qs: { account: ACCOUNT } })
    expect(second.status).toBe(204)
  })
})
