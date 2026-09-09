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

  test('concurrent feature enables for distinct merchants all survive', async ({ api }) => {
    // All merchants share ONE service_configuration row per feature, so parallel enables
    // exercise the locked read-modify-write path; a lost update surfaces as a disabled flag.
    const ids = Array.from({ length: 6 }, () => factory.merchantId('pwconc'))
    const flagPath = (id: string) => `/merchant-account/${id}/features/multi-objective-routing`

    try {
      await Promise.all(ids.map((id) => api.ensureMerchantAccount(id)))

      const enables = await Promise.all(
        ids.map((id) => api.raw('POST', flagPath(id), { body: { enabled: true } })),
      )
      for (const r of enables) expect(r.status).toBe(200)

      for (const id of ids) {
        const get = await api.raw('GET', `/merchant-account/${id}/features`)
        const entry = get.body.features.find((f: any) => f.feature === 'multi-objective-routing')
        expect(entry?.enabled, `flag for ${id} was lost by a concurrent update`).toBe(true)
      }
    } finally {
      // Parallel disables double as the removal-path concurrency check; then drop the merchants.
      const disables = await Promise.all(
        ids.map((id) =>
          api.raw('POST', flagPath(id), { body: { enabled: false }, failOnStatusCode: false }),
        ),
      )
      for (const r of disables) expect(r.status).toBe(200)
      await Promise.all(ids.map((id) => api.cleanupTestData(id)))
    }
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
})
