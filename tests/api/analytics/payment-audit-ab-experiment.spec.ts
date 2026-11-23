import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { poll } from '../../helpers/poll'
import { decide } from '../../helpers/decide'

/**
 * The payment audit tells which A/B arm routed a payment, or that none did. Payments route through
 * /decide, which applies an experiment's `hybrid_routing` split.
 */

const FEATURE_FLAG_CACHE_MS = 5_000
const INGEST_TIMEOUT = 40_000

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

/** Turns on live-traffic A/B for the merchant and waits until the decider sees it. */
async function enableRealPayments(api: ApiClient, merchantId: string) {
  for (let attempt = 0; attempt < 5; attempt++) {
    await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
    await sleep(FEATURE_FLAG_CACHE_MS + 500)
    const features = await api.getMerchantFeatures(merchantId)
    const flag = features.body.features.find((f: any) => f.feature === 'ab-test-real-payments')
    if (flag?.enabled) return
  }
  throw new Error(`ab-test-real-payments did not stay enabled for ${merchantId}`)
}

async function singleRule(api: ApiClient, merchantId: string, gateway: string) {
  const created = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName(`audit_${gateway}`),
      gateway,
    }),
  )
  return created.body.rule_id as string
}

/** A merchant whose payments route through /decide with a stripe rule plus SR. */
async function decideMerchant(api: ApiClient, merchantId: string) {
  await api.setMerchantFeature(merchantId, 'sr-routing', true)
  await api.createSuccessRateConfig(merchantId)
  const stripeRule = await singleRule(api, merchantId, 'stripe')
  await api.activateRoutingAlgorithm(merchantId, stripeRule)
  return stripeRule
}

async function routeDecide(api: ApiClient, paymentId: string) {
  // Either the rule or SR may decide: the merchant's sr-routing setting gates SR outside an
  // experiment, while an experiment arm with an SR layer always runs it.
  const decided = await decide(api, paymentId, ['stripe', 'razorpay'])
  expect(decided.connectors?.[0]?.connector, JSON.stringify(decided).slice(0, 500)).toBeTruthy()
}

function audit(api: ApiClient, paymentId: string) {
  return api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', payment_id: paymentId } })
}

/** The audit's search results for the merchant, as the list page reads them. */
function searchList(api: ApiClient) {
  return api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', page: 1, page_size: 50 } })
}

test.describe('Payment audit A/B experiment assignment (API)', () => {
  test.setTimeout(90_000)

  test('each payment routed under the experiment shows its arm', async ({ api, merchant }) => {
    const m = merchant.id
    const stripeRule = await decideMerchant(api, m)
    const razorpayRule = await singleRule(api, m, 'razorpay')
    const experiment = await api.createRoutingAlgorithm({
      name: factory.ruleName('audit_ab'),
      description: 'audit arm badge',
      created_by: m,
      algorithm_for: 'payment',
      metadata: {},
      algorithm: {
        type: 'ab_test',
        data: {
          control: { rule_algorithm_id: stripeRule, sr: {} },
          variant: { rule_algorithm_id: razorpayRule, sr: {} },
          endpoints: ['hybrid_routing'],
          variant_split_pct: 49,
          min_sample_size: 1000,
          guardrail_threshold_pp: 100,
        },
      },
    })
    expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
    const experimentId = experiment.body.rule_id as string
    await api.activateRoutingAlgorithm(m, experimentId)
    await enableRealPayments(api, m)

    const paymentIds: string[] = []
    for (let i = 0; i < 8; i++) {
      const paymentId = factory.paymentId(`audit_ab_${i}`)
      await routeDecide(api, paymentId)
      paymentIds.push(paymentId)
    }

    // The arm the experiment recorded for each payment.
    const txns = await poll(
      () =>
        api.raw('GET', `/analytics/experiment/${experimentId}/transactions`, {
          qs: { endpoint: 'hybrid_routing', page: 1, page_size: 50 },
        }),
      (r) => r.body.transactions?.length === paymentIds.length,
      { message: 'experiment did not record every payment', timeout: INGEST_TIMEOUT },
    )
    const armOf = new Map<string, string>(
      txns.body.transactions.map((t: any) => [t.payment_id, t.variant_arm]),
    )

    for (const paymentId of paymentIds) {
      const r = await audit(api, paymentId)
      expect(r.body.results[0]?.experiments, paymentId).toEqual([
        expect.objectContaining({
          experiment_id: experimentId,
          arm: armOf.get(paymentId),
          endpoint: 'hybrid_routing',
        }),
      ])
      // The assignment rides alongside the timeline; it doesn't add a timeline event.
      expect(r.body.timeline.map((e: any) => e.flow_type)).not.toContain('routing_evaluate_ab_test')
    }

    // The search results carry each payment's arm too, so the list can tag it.
    const list = await poll(
      () => searchList(api),
      (res) =>
        paymentIds.every((id) => res.body.results?.some((row: any) => row.payment_id === id)),
      { message: 'payments did not reach the audit search', timeout: INGEST_TIMEOUT },
    )
    for (const paymentId of paymentIds) {
      const row = list.body.results.find((r: any) => r.payment_id === paymentId)
      expect(row.experiments, paymentId).toEqual([
        expect.objectContaining({ experiment_id: experimentId, arm: armOf.get(paymentId) }),
      ])
    }
  })

  test('a payment routed without an experiment says so', async ({ api, merchant }) => {
    const m = merchant.id
    await decideMerchant(api, m)

    const paymentId = factory.paymentId('audit_no_ab')
    await routeDecide(api, paymentId)

    const r = await poll(
      () => audit(api, paymentId),
      (res) => res.body.timeline?.length > 0,
      { message: 'payment did not reach the audit', timeout: INGEST_TIMEOUT },
    )
    expect(r.body.results[0]?.experiments).toEqual([])

    const list = await poll(
      () => searchList(api),
      (res) => res.body.results?.some((row: any) => row.payment_id === paymentId),
      { message: 'payment did not reach the audit search', timeout: INGEST_TIMEOUT },
    )
    expect(list.body.results.find((row: any) => row.payment_id === paymentId).experiments).toEqual(
      [],
    )
  })
})
