import { test, expect, factory } from '../../fixtures/test'
import { ApiClient } from '../../fixtures/api-client'
import { ensureDashboardSession } from '../../fixtures/session'

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
    for (const expected of ['autopilot', 'elimination', 'cost-savings', 'sr-routing']) {
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

  test('toggling a feature for many merchants at once keeps every toggle', async ({ request }) => {
    // Every merchant's flag for a feature lives in one shared row. Concurrent toggles must each
    // land, not overwrite one another and silently flip the feature back for some merchant.
    const merchants = await Promise.all(
      Array.from({ length: 8 }, async (_, i) => {
        const client = new ApiClient(request)
        const id = factory.merchantId(`feature_race_${i}`)
        await client.ensureMerchantAccount(id)
        await ensureDashboardSession(client, id)
        return { id, client }
      }),
    )

    try {
      await Promise.all(
        merchants.map(({ id, client }) =>
          client.setMerchantFeature(id, 'ab-test-real-payments', true),
        ),
      )

      for (const { id, client } of merchants) {
        const r = await client.getMerchantFeatures(id)
        const flag = r.body.features.find((f: any) => f.feature === 'ab-test-real-payments')
        expect(flag?.enabled, id).toBe(true)
      }
    } finally {
      await Promise.all(
        merchants.map(({ id, client }) =>
          client.deleteMerchantAccount(id, { failOnStatusCode: false }),
        ),
      )
    }
  })
})
