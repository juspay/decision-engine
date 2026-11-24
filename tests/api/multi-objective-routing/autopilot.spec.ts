import { test, expect, factory } from '../../fixtures/test'
import decideContract from '../../fixtures/contracts/decide-request.json'

/**
 * Autopilot reliability at the API boundary.
 *
 * Autopilot's tuning math (bucket size / hedging %) is a background job best tested with Rust
 * property tests (see docs/testing-strategy.md). What we CAN and must guard end-to-end here is the
 * control surface an operator touches:
 *  - the autopilot feature flag toggles and persists,
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

/**
 * The calibration job itself, end to end: it runs every `[sr_auto_calibration] interval_secs` (60s in
 * the dev config) over the last `lookback_secs` of SR decisions, and writes an autopilot-sourced
 * sub-level entry for each segment with `min_volume` decisions across at least two gateways.
 */
test.describe('Autopilot calibration (API)', () => {
  /** Enrols a merchant in SR routing and autopilot, with no manual sub-level entries. */
  async function enrol(api: any, merchantId: string) {
    await api.createSuccessRateConfig(merchantId, { subLevelInputConfig: [] })
    expect((await api.setMerchantFeature(merchantId, 'sr-routing', true)).status).toBe(200)
    expect((await api.setMerchantFeature(merchantId, 'autopilot', true)).status).toBe(200)
  }

  /** Waits for the job's next cycles to write an autopilot entry, and returns it. */
  async function calibratedEntry(api: any, merchantId: string) {
    const entries = async () => {
      const cfg = await api.getSuccessRateConfig(merchantId)
      return (cfg.body?.config?.data?.subLevelInputConfig ?? []).filter((e: any) => e.source === 'autopilot')
    }
    await expect.poll(async () => (await entries()).length, { timeout: 180_000, intervals: [5_000] }).toBeGreaterThan(0)
    const [entry] = await entries()
    return entry
  }

  test('calibrates bucket size and hedging from SR traffic on /decide', async ({ api, merchant }) => {
    test.setTimeout(240_000)
    const m = merchant.id
    await enrol(api, m)
    // With no active rule, SR decides among the three eligible connectors; fresh scores tie, so
    // decisions spread across them.
    for (let i = 0; i < 40; i++) {
      const body = {
        ...structuredClone(decideContract),
        payment_id: factory.paymentId('autopilot'),
        routing_config: {
          ...structuredClone(decideContract.routing_config),
          eligible_connectors: ['stripe', 'adyen', 'checkout'].map((name) => {
            const { gateway_name, gateway_id } = factory.gatewayConnector(name)
            return { connector: gateway_name, merchant_connector_id: gateway_id! }
          }),
        },
      }
      const r = await api.raw('POST', '/decide', { body })
      expect(r.status, JSON.stringify(r.body).slice(0, 300)).toBe(200)
      expect(r.body.success_rate?.status).toBe('applied')
    }
    const entry = await calibratedEntry(api, m)
    // The contract payment is a credit card.
    expect(entry.paymentMethod?.toUpperCase()).toBe('CARD')
    expect(entry.paymentMethodType?.toUpperCase()).toBe('CREDIT')
    expect(entry.bucketSize).toBeGreaterThan(0)
    expect(entry.hedgingPercent).toBeGreaterThan(0)
  })
})
