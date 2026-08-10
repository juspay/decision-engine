import { test, expect, factory, poll } from '../../fixtures/test'
import {
  expectValidAnalyticsOverview,
  expectValidGatewayResponse,
  expectValidScoreUpdate,
} from '../../helpers/assertions'

/**
 * API-contract port of three Cypress specs:
 *   - cypress/e2e/api/dynamic-routing.cy.js
 *   - cypress/e2e/api/volume-split.cy.js
 *   - cypress/e2e/api/routing-mutation.cy.js
 *
 * The `merchant` fixture replaces the Cypress `waitForService` + `ensureMerchantAccount` beforeEach
 * and auto-cleans the merchant afterwards. Lazy `cy.wrap`/`Cypress._.times` chains become plain
 * `for`/`await` loops, and `cy.pollRequest` becomes the shared `poll` helper (tests/helpers/poll.ts).
 */

/** Port of the volume-split spec's `extractConnector` helper (operates on the response body). */
function extractConnector(body: any): string | null {
  return (
    body.evaluated_output?.[0]?.gateway_name ||
    body.output.connector?.gateway_name ||
    body.output.connectors?.[0]?.gateway_name ||
    null
  )
}

test.describe('Dynamic Routing API', () => {
  test('decides a gateway, updates connector feedback, and records analytics trail', async ({ api, merchant }) => {
    // Poll payment-audit + analytics-overview twice at up to 30s each; give the test room past the 60s default.
    test.setTimeout(120_000)
    const m = merchant.id
    // Replaces the beforeEach `cy.createSuccessRateConfig(merchantId)`.
    await api.createSuccessRateConfig(m)

    const firstPaymentId = factory.paymentId('dynamic_first')
    const secondPaymentId = factory.paymentId('dynamic_second')

    const decideFirst = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: m,
        paymentInfo: {
          paymentId: firstPaymentId,
          paymentMethodType: 'UPI',
          paymentMethod: 'UPI_PAY',
        },
      }),
    )
    expectValidGatewayResponse(decideFirst.body)
    const chosenGateway: string = decideFirst.body.decided_gateway
    const initialScore = decideFirst.body.gateway_priority_map[chosenGateway]
    expect(typeof chosenGateway).toBe('string')

    const score = await api.updateGatewayScore(
      factory.updateGatewayScoreRequest({
        merchantId: m,
        gateway: chosenGateway,
        paymentId: firstPaymentId,
        status: 'FAILURE',
        txnLatency: { gatewayLatency: 8000 },
      }),
    )
    expectValidScoreUpdate(score.body)
    expect(score.body.gateway).toBe(chosenGateway)
    expect(score.body.payment_id).toBe(firstPaymentId)

    const decideSecond = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: m,
        paymentInfo: {
          paymentId: secondPaymentId,
          paymentMethodType: 'UPI',
          paymentMethod: 'UPI_PAY',
        },
      }),
    )
    expectValidGatewayResponse(decideSecond.body)
    expect(decideSecond.body.gateway_priority_map[chosenGateway]).toBeLessThanOrEqual(initialScore)

    // Poll the payment-audit trail until both the decision and the score-update events land.
    // `failOnStatusCode: false` keeps transient non-2xx responses from throwing so the predicate can retry.
    const audit = await poll(
      () =>
        api.raw('GET', '/analytics/payment-audit', {
          failOnStatusCode: false,
          qs: { range: '1h', payment_id: firstPaymentId },
        }),
      ({ body }) =>
        Array.isArray(body.timeline) &&
        body.timeline.some((event: any) => event.flow_type === 'decide_gateway_decision') &&
        body.timeline.some((event: any) => event.flow_type === 'update_gateway_score_update'),
      { message: 'Expected payment audit decision + gateway update trail' },
    )
    const flowTypes = audit.body.timeline.map((event: any) => event.flow_type)
    expect(flowTypes).toContain('decide_gateway_decision')
    expect(flowTypes).toContain('update_gateway_score_update')

    // Poll the analytics overview until the dynamic-routing route hits are recorded.
    const overview = await poll(
      () =>
        api.raw('GET', '/analytics/overview', {
          failOnStatusCode: false,
          qs: { range: '1h' },
        }),
      ({ body }) =>
        Array.isArray(body.route_hits) &&
        body.route_hits.some((hit: any) => hit.route === '/decide_gateway' && hit.count >= 2) &&
        body.route_hits.some((hit: any) => hit.route === '/update_gateway' && hit.count >= 1),
      { message: 'Expected dynamic routing route hits in analytics overview' },
    )
    expectValidAnalyticsOverview(overview.body)
  })
})

test.describe('Volume Split Routing API', () => {
  test('creates, activates, evaluates, and approximates configured volume split', async ({ api, merchant }) => {
    // 100 sequential evaluations can outlast the 60s default timeout.
    test.setTimeout(120_000)
    const m = merchant.id
    const payload = factory.volumeSplitRoutingPayload(m, {
      name: factory.ruleName('volume_split'),
      data: [
        { split: 70, output: factory.gatewayConnector('stripe') },
        { split: 30, output: factory.gatewayConnector('paytm') },
      ],
    })
    const counts = new Map<string | null, number>()

    const create = await api.createRoutingAlgorithm(payload)
    const routingAlgorithmId = create.body.rule_id
    await api.activateRoutingAlgorithm(m, routingAlgorithmId)

    for (let index = 0; index < 100; index++) {
      const evaluation = await api.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(m, {}, { payment_id: factory.paymentId(`volume_eval_${index}`) }),
      )
      expect(evaluation.body.output.type).toBe('volume_split')
      const connector = extractConnector(evaluation.body)
      expect(['stripe', 'paytm']).toContain(connector)
      counts.set(connector, (counts.get(connector) || 0) + 1)
    }

    const stripeCount = counts.get('stripe') || 0
    const paytmCount = counts.get('paytm') || 0

    // Tolerance of ±20 around the configured 70/30 split (≈ ±4σ for n=100,p=0.7)
    // keeps the failure rate below 0.001% while still catching a broken distribution.
    expect(stripeCount).toBeGreaterThanOrEqual(50)
    expect(stripeCount).toBeLessThanOrEqual(90)
    expect(paytmCount).toBeGreaterThanOrEqual(10)
    expect(paytmCount).toBeLessThanOrEqual(50)
  })
})

test.describe('Routing Mutation Regression API', () => {
  test('changes the selected connector after the active routing rule is replaced', async ({ api, merchant }) => {
    const m = merchant.id
    const firstPayload = factory.singleRoutingPayload(m, {
      name: factory.ruleName('single_stripe'),
      gateway: 'stripe',
    })
    const secondPayload = factory.singleRoutingPayload(m, {
      name: factory.ruleName('single_checkout'),
      gateway: 'checkout',
    })

    const createFirst = await api.createRoutingAlgorithm(firstPayload)
    const firstRuleId = createFirst.body.rule_id
    await api.activateRoutingAlgorithm(m, firstRuleId)

    const evalFirst = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(evalFirst.body.output.type).toBe('straight_through')
    expect(evalFirst.body.output.connector.gateway_name).toBe('stripe')

    const createSecond = await api.createRoutingAlgorithm(secondPayload)
    const secondRuleId = createSecond.body.rule_id
    expect(secondRuleId).not.toBe(firstRuleId)
    await api.activateRoutingAlgorithm(m, secondRuleId)

    const evalSecond = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(evalSecond.body.output.type).toBe('straight_through')
    expect(evalSecond.body.output.connector.gateway_name).toBe('checkout')
  })
})
