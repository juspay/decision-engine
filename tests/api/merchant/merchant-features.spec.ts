import { test, expect } from '../../fixtures/test'

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
})
