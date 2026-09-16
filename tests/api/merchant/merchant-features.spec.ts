import { test, expect, factory } from '../../fixtures/test'

/**
 * Merchant feature flags + debit-routing toggle. These are operator-facing switches that gate real
 * routing behavior, so their read/write/persist path must be reliable.
 */
test.describe('Merchant features & debit routing (API)', () => {
  test('features list returns all known feature flags', async ({ api, merchant }) => {
    const r = await api.raw('GET', `/merchant-account/${merchant.id}/features`)

    expect(r.status).toBe(200)
    expect(r.body.merchant_id).toBe(merchant.id)
    expect(Array.isArray(r.body.features)).toBe(true)

    const slugs = r.body.features.map((f: any) => f.feature)
    for (const expected of ['autopilot', 'auto-calibration', 'elimination', 'multi-objective-routing']) {
      expect(slugs).toContain(expected)
    }
  })

  test('debit routing flag defaults to false for a new merchant', async ({ api, merchant }) => {
    const get = await api.raw('GET', `/merchant-account/${merchant.id}/debit-routing`)
    expect(get.status).toBe(200)
    expect(get.body.debit_routing_enabled).toBe(false)
  })

  test('debit routing flag toggles on and off and persists', async ({ api, merchant }) => {
    const path = `/merchant-account/${merchant.id}/debit-routing`

    const on = await api.raw('POST', path, { body: { enabled: true } })
    expect(on.status).toBe(200)
    let get = await api.raw('GET', path)
    expect(get.body.debit_routing_enabled).toBe(true)

    const off = await api.raw('POST', path, { body: { enabled: false } })
    expect(off.status).toBe(200)
    get = await api.raw('GET', path)
    expect(get.body.debit_routing_enabled).toBe(false)
  })

  /**
   * REGRESSION GUARD for lost feature-flag updates under concurrency.
   *
   * A feature's enabled merchants live in ONE service_configuration row, as a single FeatureConf
   * blob, so every toggle of that feature — for any merchant — read-modify-writes the same row.
   * With a plain update the last writer won the whole list: each caller had read the list as it was
   * before the others' changes, so all but one merchant were silently dropped. The API still
   * answered 200 with the feature shown as enabled, and the merchant simply never got the
   * behaviour.
   *
   * Toggling for N merchants at once must leave all N enabled.
   *
   * Uses `gsm-scoring-filter` rather than a flag the routing specs depend on: this test deliberately
   * hammers one row, and picking an inert key keeps that contention out of everyone else's way.
   */
  test('concurrent enables for different merchants do not drop each other', async ({ api }) => {
    const merchants = Array.from({ length: 8 }, () => factory.merchantId('featrace'))
    await Promise.all(merchants.map(id => api.ensureMerchantAccount(id)))

    try {
      const results = await Promise.all(
        merchants.map(id =>
          api.raw('POST', `/merchant-account/${id}/features/gsm-scoring-filter`, {
            body: { enabled: true },
            failOnStatusCode: false,
          }),
        ),
      )
      for (const [i, r] of results.entries()) {
        expect(r.status, `enable for ${merchants[i]} failed: ${JSON.stringify(r.body)}`).toBe(200)
      }

      // Read each back independently — the write responses are built from the writer's own view.
      const enabled = await Promise.all(
        merchants.map(async id => {
          const r = await api.raw('GET', `/merchant-account/${id}/features`)
          return r.body.features.find((f: any) => f.feature === 'gsm-scoring-filter')?.enabled
        }),
      )

      const dropped = merchants.filter((_, i) => enabled[i] !== true)
      expect(dropped, `these merchants lost their enable: ${dropped.join(', ')}`).toEqual([])
    } finally {
      // Leave the shared row as we found it.
      await Promise.all(
        merchants.map(id =>
          api.raw('POST', `/merchant-account/${id}/features/gsm-scoring-filter`, {
            body: { enabled: false },
            failOnStatusCode: false,
          }),
        ),
      )
      await Promise.all(merchants.map(id => api.cleanupTestData(id)))
    }
  })
})
