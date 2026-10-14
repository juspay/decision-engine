import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { redisKeys } from '../../helpers/redis'

/**
 * The Redis keys success-rate scores are learned under. Production scores live under these keys,
 * so changing their shape silently throws away what every merchant's routing has learned: a
 * failure here needs a key migration, not a new expectation.
 *
 * Keys read `{gw_sr_v3_score_<merchant>_<order type>_<payment method type>_<payment method>_<gateway>_}score`
 * (and `...}queue`), with the payment method parts in Hyperswitch's meaning.
 */

type Payment = { label: string; pm: string; pmt?: string; extra?: Record<string, unknown> }

const HYBRID_KEYS: { payment: Payment; segment: string }[] = [
  {
    payment: { label: 'card', pm: 'card', pmt: 'credit', extra: { cardType: 'CREDIT', authType: 'THREE_DS' } },
    segment: 'ORDER_PAYMENT_CREDIT_CARD',
  },
  { payment: { label: 'wallet', pm: 'wallet', pmt: 'google_pay' }, segment: 'ORDER_PAYMENT_GOOGLE_PAY_WALLET' },
  { payment: { label: 'upi', pm: 'upi', pmt: 'upi_collect' }, segment: 'ORDER_PAYMENT_UPI_COLLECT_UPI' },
  // A payment without a type keys UNKNOWN in its place.
  { payment: { label: 'untyped', pm: 'wallet' }, segment: 'ORDER_PAYMENT_UNKNOWN_WALLET' },
]

async function setupSrMerchant(api: ApiClient, merchantId: string) {
  await api.createSuccessRateConfig(merchantId)
  const rule = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, { name: factory.ruleName('score_keys'), gateway: 'stripe' }),
  )
  await api.activateRoutingAlgorithm(merchantId, rule.body.rule_id)
  expect((await api.setMerchantFeature(merchantId, 'sr-routing', true)).status).toBe(200)
}

/** Routes a payment through /routing/hybrid as Hyperswitch does; returns the decided gateway. */
async function routeHybrid(api: ApiClient, merchantId: string, paymentId: string, payment: Payment) {
  const parameters: Record<string, unknown> = {
    payment_type: { type: 'enum_variant', value: 'order_payment' },
    payment_method: { type: 'enum_variant', value: payment.pm },
    amount: { type: 'number', value: 100 },
  }
  if (payment.pmt) parameters.payment_method_type = { type: 'enum_variant', value: payment.pmt }
  const r = await api.raw('POST', '/routing/hybrid', {
    body: {
      static_routing_request: {
        created_by: merchantId,
        payment_id: paymentId,
        parameters,
        fallback_output: [factory.gatewayConnector('stripe')],
      },
      dynamic_routing_request: factory.srDecideGatewayRequest({
        merchantId,
        eliminationEnabled: false,
        paymentInfo: {
          paymentId,
          paymentMethod: payment.pm,
          paymentMethodType: payment.pmt ?? '',
          ...payment.extra,
        },
      }),
    },
  })
  expect(r.status, JSON.stringify(r.body).slice(0, 500)).toBe(200)
  const gateway: string = r.body.dynamic_routing?.decision?.decided_gateway
  expect(gateway, JSON.stringify(r.body).slice(0, 500)).toBeTruthy()
  return gateway
}

async function scorePayment(api: ApiClient, merchantId: string, paymentId: string, gateway: string) {
  const r = await api.updateGatewayScore(
    factory.updateGatewayScoreRequest({ merchantId, gateway, paymentId, status: 'CHARGED' }),
  )
  expect(r.status).toBe(200)
}

/** The SR v3 score and queue keys a payment routed to `gateway` should have written. */
function srKeys(merchantId: string, segment: string, gateway: string) {
  const base = `{gw_sr_v3_score_${merchantId}_${segment}_${gateway}_}`
  return [`${base}queue`, `${base}score`]
}

test.describe('Success-rate score keys (API)', () => {
  test('hybrid payments keep their production score keys', async ({ api, merchant }) => {
    const m = merchant.id
    await setupSrMerchant(api, m)

    const expected: string[] = []
    for (const { payment, segment } of HYBRID_KEYS) {
      const paymentId = factory.paymentId(`score_keys_${payment.label}`)
      const gateway = await routeHybrid(api, m, paymentId, payment)
      await scorePayment(api, m, paymentId, gateway)
      expected.push(...srKeys(m, segment, gateway))
    }

    // Scores are written just after the update responds.
    await expect.poll(() => redisKeys(`{gw_sr_v3_score_${m}_*`)).toEqual(expected.sort())
  })

  test('decide-gateway learns into the same keys as hybrid', async ({ api, merchant }) => {
    const m = merchant.id
    await setupSrMerchant(api, m)

    const paymentId = factory.paymentId('score_keys_decide')
    const decided = await api.decideGateway({
      merchantId: m,
      eliminationEnabled: false,
      paymentInfo: { paymentId, paymentMethod: 'card', paymentMethodType: 'credit', cardType: 'CREDIT' },
    })
    expect(decided.status).toBe(200)
    await scorePayment(api, m, paymentId, decided.body.decided_gateway)

    await expect.poll(() => redisKeys(`{gw_sr_v3_score_${m}_*`)).toEqual(
      srKeys(m, 'ORDER_PAYMENT_CREDIT_CARD', decided.body.decided_gateway),
    )
  })
})
