import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'

/**
 * `/decide-gateway` and `/routing/hybrid` read a payment's method the way Hyperswitch does:
 * `paymentMethod` is the category (`CARD`) and `paymentMethodType` the specific method
 * (`CREDIT`). A card payment is routed as a card whichever endpoint it comes through.
 */

const CARD_DIMENSION = 'ORDER_PAYMENT, CREDIT, CARD, CREDIT'

async function setupSrMerchant(api: ApiClient, merchantId: string) {
  await api.createSuccessRateConfig(merchantId)
  const rule = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName('pm_semantics'),
      gateway: 'stripe',
    }),
  )
  await api.activateRoutingAlgorithm(merchantId, rule.body.rule_id)
  const toggle = await api.setMerchantFeature(merchantId, 'sr-routing', true)
  expect(toggle.status).toBe(200)
}

test.describe('Payment method meaning (API)', () => {
  test('decide-gateway rejects the legacy order and says to swap', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const legacy = await api.decideGateway(
      { merchantId: merchant.id, paymentInfo: { paymentMethod: 'CREDIT', paymentMethodType: 'CARD' } },
      { failOnStatusCode: false },
    )
    expect(legacy.status).toBe(400)
    expect(legacy.body.error_info.code).toBe('INVALID_PAYMENT_METHOD')
    expect(legacy.body.error_info.developer_message).toContain('looks swapped')

    // A card network is not a payment method type.
    const network = await api.decideGateway(
      { merchantId: merchant.id, paymentInfo: { paymentMethod: 'CARD', paymentMethodType: 'VISA' } },
      { failOnStatusCode: false },
    )
    expect(network.status).toBe(400)
    expect(network.body.error_info.developer_message).toContain('paymentMethodType `VISA`')
  })

  test('a card routes as a card through decide-gateway and hybrid alike', async ({ api, merchant }) => {
    const m = merchant.id
    await setupSrMerchant(api, m)

    // Hyperswitch's own casing is accepted.
    const decided = await api.decideGateway({
      merchantId: m,
      eliminationEnabled: false,
      paymentInfo: { paymentMethod: 'card', paymentMethodType: 'credit', cardType: 'CREDIT' },
    })
    expect(decided.status).toBe(200)
    expect(decided.body.routing_dimension).toBe(CARD_DIMENSION)
    expect(decided.body.routing_dimension_level).toBe('CARD_LEVEL')

    const paymentId = factory.paymentId('pm_semantics_hybrid')
    const hybrid = await api.raw('POST', '/routing/hybrid', {
      body: {
        static_routing_request: {
          created_by: m,
          payment_id: paymentId,
          parameters: {
            payment_type: { type: 'enum_variant', value: 'order_payment' },
            payment_method: { type: 'enum_variant', value: 'card' },
            payment_method_type: { type: 'enum_variant', value: 'credit' },
            card_type: { type: 'enum_variant', value: 'credit' },
            amount: { type: 'number', value: 100 },
          },
          fallback_output: [factory.gatewayConnector('stripe')],
        },
        dynamic_routing_request: factory.srDecideGatewayRequest({
          merchantId: m,
          eliminationEnabled: false,
          paymentInfo: { paymentId },
        }),
      },
    })
    expect(hybrid.status).toBe(200)
    const decision = hybrid.body.dynamic_routing?.decision
    expect(decision?.routing_dimension).toBe(CARD_DIMENSION)
    expect(decision?.routing_dimension_level).toBe('CARD_LEVEL')

    // The payment's outcome is scored from what its decision saved.
    const scored = await api.updateGatewayScore(
      factory.updateGatewayScoreRequest({
        merchantId: m,
        gateway: decision.decided_gateway,
        paymentId,
        status: 'CHARGED',
      }),
    )
    expect(scored.status).toBe(200)
  })
})
