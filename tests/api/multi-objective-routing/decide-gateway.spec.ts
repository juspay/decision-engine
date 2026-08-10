import { test, expect, factory } from '../../fixtures/test'
import { expectValidGatewayResponse } from '../../helpers/assertions'

/**
 * Core reliability of the decision endpoint (/decide-gateway) — the single most important API in
 * the system. These assert invariants that must hold regardless of scoring internals:
 *  - a well-formed decision is always returned,
 *  - the decided gateway is drawn from the caller's eligible list,
 *  - the decision works across payment methods and elimination settings.
 *
 * The routing *correctness matrix* (which gateway wins for which score/segment) belongs in fast
 * Rust tests, not here — see docs/testing-strategy.md.
 */
test.describe('Decide Gateway (API)', () => {
  test('returns a valid ranked decision for SR routing', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: merchant.id,
        eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
      }),
    )

    expect(r.status).toBe(200)
    expectValidGatewayResponse(r.body)
  })

  test('decided gateway is drawn from the eligible list', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    const eligible = ['stripe', 'adyen']

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({ merchantId: merchant.id, eligibleGatewayList: eligible }),
    )

    expectValidGatewayResponse(r.body)
    expect(eligible).toContain(r.body.decided_gateway)
  })

  test('a single eligible gateway is always the decided gateway', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({ merchantId: merchant.id, eligibleGatewayList: ['stripe'] }),
    )

    expectValidGatewayResponse(r.body)
    expect(r.body.decided_gateway).toBe('stripe')
  })

  test('CARD payment returns a valid decision', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: merchant.id,
        eligibleGatewayList: ['stripe', 'adyen'],
        paymentInfo: { paymentMethodType: 'CARD', paymentMethod: 'VISA' },
      }),
    )

    expectValidGatewayResponse(r.body)
  })

  test('decision succeeds with elimination disabled', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: merchant.id,
        eligibleGatewayList: ['stripe', 'adyen'],
        eliminationEnabled: false,
      }),
    )

    expectValidGatewayResponse(r.body)
  })
})
