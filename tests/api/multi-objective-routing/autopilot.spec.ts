import { test, expect } from '../../fixtures/test'

/**
 * Autopilot reliability at the API boundary.
 *
 * Autopilot's tuning math (bucket size / hedging %) is a background job best tested with Rust
 * property tests (see docs/testing-strategy.md). What we CAN and must guard end-to-end here is the
 * control surface an operator touches:
 *  - the autopilot / auto-calibration feature flags toggle and persist,
 *  - the "hard refresh" (/gateway-score/reset) flushes scores AND — critically — clears only
 *    autopilot-authored sub-level overrides while PRESERVING human-authored config.
 */
test.describe('Autopilot control surface (API)', () => {
  test('autopilot feature toggles on and off and persists', async ({ api, merchant }) => {
    const base = `/merchant-account/${merchant.id}/features`

    const enable = await api.raw('POST', `${base}/autopilot`, { body: { enabled: true } })
    expect(enable.status).toBe(200)

    let list = await api.raw('GET', base)
    expect(list.body.features.find((f: any) => f.feature === 'autopilot')?.enabled).toBe(true)

    const disable = await api.raw('POST', `${base}/autopilot`, { body: { enabled: false } })
    expect(disable.status).toBe(200)

    list = await api.raw('GET', base)
    expect(list.body.features.find((f: any) => f.feature === 'autopilot')?.enabled).toBe(false)
  })

  test('sr auto-calibration feature toggles independently of autopilot', async ({ api, merchant }) => {
    const base = `/merchant-account/${merchant.id}/features`

    await api.raw('POST', `${base}/auto-calibration`, { body: { enabled: true } })

    const list = await api.raw('GET', base)
    expect(list.body.features.find((f: any) => f.feature === 'auto-calibration')?.enabled).toBe(true)
    // Enabling auto-calibration must not implicitly flip the autopilot master flag.
    expect(list.body.features.find((f: any) => f.feature === 'autopilot')?.enabled).toBe(false)
  })

  test('gateway-score reset succeeds and reports counts', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const r = await api.raw('POST', '/gateway-score/reset', { body: { merchant_id: merchant.id } })

    expect(r.status).toBe(200)
    expect(r.body.merchant_id).toBe(merchant.id)
    expect(typeof r.body.deleted_keys).toBe('number')
    expect(typeof r.body.removed_overrides).toBe('number')
  })

  test('gateway-score reset preserves human sub-level overrides', async ({ api, merchant }) => {
    // srConfigData seeds a manual subLevelInputConfig entry (no `source: autopilot` marker).
    await api.createSuccessRateConfig(merchant.id)

    const reset = await api.raw('POST', '/gateway-score/reset', { body: { merchant_id: merchant.id } })
    expect(reset.status).toBe(200)
    // No autopilot-authored overrides exist, so none should be removed.
    expect(reset.body.removed_overrides).toBe(0)

    // The human-authored config must survive the reset.
    const cfg = await api.getSuccessRateConfig(merchant.id)
    expect(cfg.status).toBe(200)
    expect(Array.isArray(cfg.body.config.data.subLevelInputConfig)).toBe(true)
    expect(cfg.body.config.data.subLevelInputConfig.length).toBeGreaterThan(0)
  })

  test('gateway-score reset requires a merchant_id', async ({ api }) => {
    const r = await api.raw('POST', '/gateway-score/reset', { body: {}, failOnStatusCode: false })
    expect(r.status).toBeGreaterThanOrEqual(400)
  })
})
