import { test, expect } from '../../fixtures/test'

/**
 * Autopilot owns the SR sub-level rows marked `source: autopilot` and retunes them in place. A
 * `/rule/update` must not drop or revert them: when it did, Autopilot found its row gone, tuned
 * the cluster from scratch, and its retune history restarted mid-run.
 */
const row = (cardNetwork: string, bucketSize: number, source?: string) => ({
  paymentMethod: 'card',
  paymentMethodType: 'debit',
  cardNetwork,
  currency: 'USD',
  authType: 'THREE_DS',
  ...(source ? { source } : {}),
  bucketSize,
  hedgingPercent: 20,
  latencyThreshold: null,
})

const config = (rows: unknown[] | null) => ({
  type: 'successRate',
  data: { defaultBucketSize: 200, defaultHedgingPercent: 10, subLevelInputConfig: rows },
})

async function storedRows(api: any, merchantId: string) {
  const r = await api.getRuleConfig(merchantId, 'successRate')
  expect(r.status).toBe(200)
  return (r.body.config.data.subLevelInputConfig ?? []).map((entry: any) => ({
    cardNetwork: entry.cardNetwork,
    bucketSize: entry.bucketSize,
    source: entry.source ?? null,
  }))
}

test.describe('SR config updates keep Autopilot rows (API)', () => {
  test('an update that leaves out or carries a stale Autopilot row keeps the stored one', async ({ api, merchant }) => {
    // Stand in for Autopilot having tuned VISA to 300.
    expect((await api.createRuleConfig(merchant.id, config([row('VISA', 300, 'autopilot')]))).status).toBe(200)

    // A client that only sends its own rows.
    expect((await api.updateRuleConfig(merchant.id, config([row('MASTERCARD', 50)]))).status).toBe(200)
    expect(await storedRows(api, merchant.id)).toEqual([
      { cardNetwork: 'MASTERCARD', bucketSize: 50, source: null },
      { cardNetwork: 'VISA', bucketSize: 300, source: 'autopilot' },
    ])

    // A form loaded before the last retune sends VISA at 250 and no rows of its own.
    expect((await api.updateRuleConfig(merchant.id, config([row('VISA', 250, 'autopilot')]))).status).toBe(200)
    expect(await storedRows(api, merchant.id)).toEqual([{ cardNetwork: 'VISA', bucketSize: 300, source: 'autopilot' }])
  })

  test('a manual row for the same cluster takes it over from Autopilot', async ({ api, merchant }) => {
    expect((await api.createRuleConfig(merchant.id, config([row('VISA', 300, 'autopilot')]))).status).toBe(200)
    expect((await api.updateRuleConfig(merchant.id, config([row('VISA', 40)]))).status).toBe(200)
    expect(await storedRows(api, merchant.id)).toEqual([{ cardNetwork: 'VISA', bucketSize: 40, source: null }])
  })
})
