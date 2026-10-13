import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { poll } from '../../helpers/poll'

/**
 * The payment audit tells which A/B arm routed a payment, or that none did. Hyperswitch routes a
 * cut-over profile's payments through /routing/hybrid, so that is where the experiment splits.
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

/** A merchant whose payments route through /routing/hybrid with a stripe rule plus SR. */
async function hybridMerchant(api: ApiClient, merchantId: string) {
  await api.setMerchantFeature(merchantId, 'sr-routing', true)
  await api.createSuccessRateConfig(merchantId)
  const stripeRule = await singleRule(api, merchantId, 'stripe')
  await api.activateRoutingAlgorithm(merchantId, stripeRule)
  return stripeRule
}

async function routeHybrid(api: ApiClient, merchantId: string, paymentId: string) {
  const r = await api.raw('POST', '/routing/hybrid', {
    body: {
      static_routing_request: {
        created_by: merchantId,
        payment_id: paymentId,
        parameters: {
          payment_method: { type: 'enum_variant', value: 'card' },
          amount: { type: 'number', value: 100 },
        },
        fallback_output: [factory.gatewayConnector('stripe')],
      },
      dynamic_routing_request: factory.srDecideGatewayRequest({
        merchantId,
        eliminationEnabled: false,
        paymentInfo: { paymentId },
      }),
    },
  })
  // Either half may decide: the merchant's sr-routing setting gates the dynamic half outside an
  // experiment, while an experiment arm with an SR layer always runs it.
  const decided =
    r.body?.dynamic_routing?.decision?.decided_gateway ??
    r.body?.evaluated_connectors?.[0]?.gateway_name
  expect(decided, JSON.stringify(r.body).slice(0, 500)).toBeTruthy()
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
    const stripeRule = await hybridMerchant(api, m)
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
          min_sample_size: 1,
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
      await routeHybrid(api, m, paymentId)
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
    await hybridMerchant(api, m)

    const paymentId = factory.paymentId('audit_no_ab')
    await routeHybrid(api, m, paymentId)

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
