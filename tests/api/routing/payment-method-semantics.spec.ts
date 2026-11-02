import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { poll } from '../../helpers/poll'

/**
 * `/decide-gateway` and `/routing/hybrid` read a payment's method the same way:
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
    expect(legacy.body.error_info.developer_message).toContain('look swapped')

    // A known type under a known method it does not belong to.
    const mismatched = await api.decideGateway(
      { merchantId: merchant.id, paymentInfo: { paymentMethod: 'card', paymentMethodType: 'google_pay' } },
      { failOnStatusCode: false },
    )
    expect(mismatched.status).toBe(400)
    expect(mismatched.body.error_info.developer_message).toContain('is not a card payment method type')
  })

  test('decide-gateway routes a method or type newer than this build', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const decided = await api.decideGateway({
      merchantId: merchant.id,
      eliminationEnabled: false,
      paymentInfo: { paymentMethod: 'new_method', paymentMethodType: 'new_type' },
    })
    expect(decided.status).toBe(200)
    expect(decided.body.decided_gateway).toBeTruthy()
  })

  test('a card routes as a card through decide-gateway and hybrid alike', async ({ api, merchant }) => {
    const m = merchant.id
    await setupSrMerchant(api, m)

    // Lower-case values are accepted too.
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

  test('SR config rows are checked and stored in the same meaning', async ({ api, merchant }) => {
    const row = (paymentMethod: string, paymentMethodType: string | null) => ({
      paymentMethod,
      paymentMethodType,
      bucketSize: 50,
      hedgingPercent: null,
      latencyThreshold: null,
    })

    const legacy = await api.createSuccessRateConfig(
      merchant.id,
      { subLevelInputConfig: [row('CREDIT', 'CARD')] },
      { failOnStatusCode: false },
    )
    expect(legacy.status).toBe(400)
    expect(JSON.stringify(legacy.body)).toContain('look swapped')

    const created = await api.createSuccessRateConfig(merchant.id, {
      subLevelInputConfig: [row('card', 'credit'), row('wallet', null)],
    })
    expect(created.status).toBe(200)
    const stored = await api.getSuccessRateConfig(merchant.id)
    expect(
      stored.body.config.data.subLevelInputConfig.map((r: any) => [r.paymentMethod, r.paymentMethodType]),
    ).toEqual([
      ['CARD', 'CREDIT'],
      ['WALLET', null],
    ])
  })

  test('routing keys list payment methods from the code', async ({ api }) => {
    const r = await api.raw('GET', '/config/routing-keys')
    expect(r.status).toBe(200)
    const values = (key: string) =>
      String(r.body.keys[key]?.values ?? '')
        .split(',')
        .map((v) => v.trim())
        .sort()
    expect(values('payment_method')).toContain('card')
    expect(values('payment_method')).toHaveLength(16)
    expect(values('card')).toEqual(['card', 'credit', 'debit'])
    expect(values('upi')).toEqual(['upi_collect', 'upi_intent', 'upi_qr'])
  })

  test('the payment audit records both endpoints in the same meaning', async ({ api, merchant }) => {
    test.setTimeout(90_000)
    const m = merchant.id
    await setupSrMerchant(api, m)

    const payments = [
      { pm: 'card', pmt: 'credit', extra: { cardType: 'CREDIT' } },
      { pm: 'wallet', pmt: 'google_pay', extra: {} },
      { pm: 'upi', pmt: 'upi_collect', extra: {} },
    ]
    const expected = new Map<string, [string, string]>()
    const routes: Promise<unknown>[] = []
    for (const p of payments) {
      // Through /decide-gateway, as a direct integration sends it.
      const decideId = factory.paymentId(`pm_audit_d_${p.pm}`)
      expected.set(decideId, [p.pm.toUpperCase(), p.pmt.toUpperCase()])
      routes.push(
        api
          .decideGateway({
            merchantId: m,
            eliminationEnabled: false,
            paymentInfo: { paymentId: decideId, paymentMethod: p.pm, paymentMethodType: p.pmt, ...p.extra },
          })
          .then((r) => expect(r.status).toBe(200)),
      )
      // Through /routing/hybrid, as rule parameters.
      const hybridId = factory.paymentId(`pm_audit_h_${p.pm}`)
      expected.set(hybridId, [p.pm.toUpperCase(), p.pmt.toUpperCase()])
      routes.push(
        api
          .raw('POST', '/routing/hybrid', {
            body: {
              static_routing_request: {
                created_by: m,
                payment_id: hybridId,
                parameters: {
                  payment_type: { type: 'enum_variant', value: 'order_payment' },
                  payment_method: { type: 'enum_variant', value: p.pm },
                  payment_method_type: { type: 'enum_variant', value: p.pmt },
                  amount: { type: 'number', value: 100 },
                },
                fallback_output: [factory.gatewayConnector('stripe')],
              },
              dynamic_routing_request: factory.srDecideGatewayRequest({
                merchantId: m,
                eliminationEnabled: false,
                paymentInfo: { paymentId: hybridId, paymentMethod: p.pm, paymentMethodType: p.pmt, ...p.extra },
              }),
            },
          })
          .then((r) => expect(r.status).toBe(200)),
      )
    }
    // Both endpoints at once.
    await Promise.all(routes)

    await poll(
      () => api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', page: 1, page_size: 50 } }),
      (r) => r.body.total_results === expected.size,
      { message: 'payments did not reach the audit', timeout: 40_000 },
    )

    // Each payment's events carry its method as the category and its type as the specific method.
    for (const [paymentId, [method, methodType]] of expected) {
      const r = await api.raw('GET', '/analytics/payment-audit', {
        qs: { range: '1d', page: 1, payment_id: paymentId },
      })
      const recorded = r.body.timeline
        .filter((event: any) => event.payment_method || event.payment_method_type)
        .map((event: any) => [event.payment_method, event.payment_method_type])
      expect(recorded.length, paymentId).toBeGreaterThan(0)
      for (const pair of recorded) expect(pair, paymentId).toEqual([method, methodType])
    }

    // The dropdowns keep categories and specific methods apart, and each filter finds both
    // endpoints' payments.
    const options = await api.raw('GET', '/analytics/payment-audit/filter-options', { qs: { range: '1d' } })
    expect(options.body.payment_methods).toEqual(['card', 'upi', 'wallet'])
    expect(options.body.payment_method_types).toEqual(['credit', 'google_pay', 'upi_collect'])
    for (const p of payments) {
      const byMethod = await api.raw('GET', '/analytics/payment-audit', {
        qs: { range: '1d', page: 1, payment_method: p.pm },
      })
      expect(byMethod.body.total_results, p.pm).toBe(2)
      expect([...byMethod.body.results.map((row: any) => row.routes.join(','))].sort(), p.pm).toEqual([
        'Decide Gateway',
        'Hybrid Routing',
      ])
      const byType = await api.raw('GET', '/analytics/payment-audit', {
        qs: { range: '1d', page: 1, payment_method_type: p.pmt },
      })
      expect(byType.body.total_results, p.pmt).toBe(2)
    }
  })
})
