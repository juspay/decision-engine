import { test, expect, factory, poll } from '../../fixtures/test'
import {
  expectValidGatewayResponse,
  expectValidRuleConfigResponse,
  expectValidScoreUpdate,
} from '../../helpers/assertions'

/**
 * API-contract port of cypress/e2e/api/sr-routing.cy.js.
 *
 * The `merchant` fixture replaces the Cypress `waitForService` + `ensureMerchantAccount` beforeEach
 * and auto-cleans the merchant afterwards. The lazy `cy.wrap` reduce chain becomes a plain
 * `for` loop with `await`.
 */
test.describe('SR Routing (API)', () => {
  // Validates write-through cache: an SR config update must be immediately visible via /rule/get
  // without waiting for TTL expiry.
  test('SR config update is immediately visible after write (cache consistency)', async ({ api, merchant }) => {
    const m = merchant.id

    const created = await api.createSuccessRateConfig(m, { defaultBucketSize: 150, defaultHedgingPercent: 3 })
    expectValidRuleConfigResponse(created.body, 'successRate')

    let got = await api.getSuccessRateConfig(m)
    expect(got.body.config.data.defaultBucketSize).toBe(150)
    expect(got.body.config.data.defaultHedgingPercent).toBe(3)

    const updated = await api.updateSuccessRateConfig(m, { defaultBucketSize: 300, defaultHedgingPercent: 10 })
    expectValidRuleConfigResponse(updated.body, 'successRate')

    // Immediately after update — must reflect new values, not a stale cache hit.
    got = await api.getSuccessRateConfig(m)
    expect(got.body.config.data.defaultBucketSize).toBe(300)
    expect(got.body.config.data.defaultHedgingPercent).toBe(10)
  })

  // Validates write-through cache eviction on delete: /rule/get must return non-200 immediately.
  test('SR config is not found immediately after deletion (cache eviction)', async ({ api, merchant }) => {
    const m = merchant.id

    const created = await api.createSuccessRateConfig(m)
    expectValidRuleConfigResponse(created.body, 'successRate')

    const got = await api.getSuccessRateConfig(m)
    expectValidRuleConfigResponse(got.body, 'successRate')

    const del = await api.deleteSuccessRateConfig(m)
    expect(del.status).toBe(200)

    // Must not serve the deleted config from cache.
    const afterDelete = await api.getSuccessRateConfig(m, { failOnStatusCode: false })
    expect(afterDelete.status).not.toBe(200)
  })

  // Validates the explore-exploit fix: gateway scores must decrease after repeated failure feedback
  // (i.e. scores are updating, not frozen at 1.0 by the top-gateway exclusion bug).
  //
  // Each failure requires a prior /decide-gateway with the same paymentId because the backend stores
  // GatewayScoringData in Redis keyed by paymentId during decide, and /update-gateway-score looks it
  // up by that key.
  test('gateway score decreases after repeated failure feedback (explore-exploit fix)', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m, { defaultBucketSize: 10, defaultHedgingPercent: 50 })

    const gateways = ['stripe', 'adyen']
    // Only the gateway a decision actually picked can be scored: /update-gateway-score looks up the
    // GatewayScoringData that /decide-gateway stored in Redis under the same paymentId. With hedging
    // at 50% the pick varies per iteration, so track every gateway that really received a failure
    // instead of assuming the first one keeps winning.
    const failed = new Set<string>()
    const FAILURES = 5

    for (let i = 0; i < FAILURES; i++) {
      const pid = factory.paymentId(`fail_${i}`)

      const decide = await api.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId: m,
          eligibleGatewayList: gateways,
          paymentInfo: { paymentMethodType: 'CARD', paymentMethod: 'VISA', paymentId: pid },
        }),
      )
      expectValidGatewayResponse(decide.body)

      const score = await api.updateGatewayScore(
        factory.updateGatewayScoreRequest({
          merchantId: m,
          gateway: decide.body.decided_gateway,
          paymentId: pid,
          status: 'FAILURE',
        }),
      )
      expectValidScoreUpdate(score.body)
      failed.add(decide.body.decided_gateway)
    }

    // Proves scores are updating, not frozen at 1.0 by the top-gateway exclusion bug.
    //
    // Two things keep this stable that the original form got wrong. It asserts on the MINIMUM across
    // the gateways that actually received a failure, so it doesn't depend on which one hedging picked
    // first; and it POLLS, because feedback is applied asynchronously — a single read can land before
    // the last update-gateway-score has propagated, which is what made this flake under parallel load.
    const settled = await poll(
      () =>
        api.decideGateway(
          factory.srDecideGatewayRequest({
            merchantId: m,
            eligibleGatewayList: gateways,
            paymentInfo: { paymentMethodType: 'CARD', paymentMethod: 'VISA' },
          }),
        ),
      ({ body }) => Math.min(...[...failed].map((g) => body.gateway_priority_map[g])) < 1.0,
      {
        message: `Expected a penalised gateway (${[...failed].join(', ')}) to score below 1.0`,
        timeout: 15_000,
        interval: 1_000,
      },
    )
    expectValidGatewayResponse(settled.body)
  })
})
