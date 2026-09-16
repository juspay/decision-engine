import { test, expect, factory } from '../../fixtures/test'
import { enableRealPayments, experimentPayload, makeRule } from '../../helpers/ab-test'

/**
 * A/B routing behaviour at /decide-gateway — arm assignment and the two switches that gate it.
 *
 * These run in parallel despite every test enabling `ab-test-real-payments`, which is one global
 * row holding one list for all merchants: `update_conf` swaps that row against the value it read
 * and re-applies on a lost race, so concurrent enables for different merchants no longer drop each
 * other. That property has its own guard in tests/api/merchant/merchant-features.spec.ts — if it
 * regresses, expect intermittent failures here too, where a payment routes as though no experiment
 * were active.
 */

// ── routing behaviour ─────────────────────────────────────────────────────────

/** Stand up an active experiment: rule control vs SR variant, at the given split. */
async function activateExperiment(api: any, merchantId: string, variantSplitPct: number) {
  const controlRule = await makeRule(api, merchantId, 'checkout')
  await api.activateRoutingAlgorithm(merchantId, controlRule)
  await api.createSuccessRateConfig(merchantId)

  const created = await api.createRoutingAlgorithm(
    experimentPayload(
      merchantId,
      { kind: 'rule', algorithm_id: controlRule },
      { kind: 'sr', sr_config: { enable_multi_objective: false, use_autopilot: true } },
      { variant_split_pct: variantSplitPct },
    ),
  )
  await api.activateRoutingAlgorithm(merchantId, created.body.rule_id)
  return { experimentId: created.body.rule_id as string, controlRule }
}

test.describe('A/B routing behaviour', () => {
  test('an active experiment stamps the arm on the decision', async ({ api, merchant }) => {
    const m = merchant.id
    await enableRealPayments(api, m)
    const { experimentId } = await activateExperiment(api, m, 50)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: m,
        paymentInfo: { paymentId: factory.paymentId('ab_stamp') },
        eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
      }),
    )

    expect(r.status).toBe(200)
    expect(r.body.ab_test_info).toBeTruthy()
    expect(r.body.ab_test_info.experimentId).toBe(experimentId)
    expect(['control', 'variant']).toContain(r.body.ab_test_info.arm)
  })

  /**
   * Assignment is a pure function of the payment id, not a per-call coin flip. This is what keeps a
   * retry in the arm its first attempt was in — without it, retries leak across arms and inflate
   * whichever side happens to catch them.
   */
  test('the same payment id always lands in the same arm', async ({ api, merchant }) => {
    const m = merchant.id
    await enableRealPayments(api, m)
    await activateExperiment(api, m, 50)

    const paymentId = factory.paymentId('ab_deterministic')
    const arms: string[] = []
    for (let i = 0; i < 4; i++) {
      const r = await api.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId: m,
          paymentInfo: { paymentId },
          eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
        }),
      )
      expect(r.body.ab_test_info).toBeTruthy()
      arms.push(r.body.ab_test_info.arm)
    }

    expect(new Set(arms).size).toBe(1)
  })

  test('a 0% split sends every payment to control', async ({ api, merchant }) => {
    const m = merchant.id
    await enableRealPayments(api, m)
    await activateExperiment(api, m, 0)

    for (let i = 0; i < 6; i++) {
      const r = await api.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId: m,
          paymentInfo: { paymentId: factory.paymentId(`ab_zero_${i}`) },
          eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
        }),
      )
      expect(r.body.ab_test_info?.arm).toBe('control')
    }
  })

  test('a 100% split sends every payment to the variant', async ({ api, merchant }) => {
    const m = merchant.id
    await enableRealPayments(api, m)
    await activateExperiment(api, m, 100)

    for (let i = 0; i < 6; i++) {
      const r = await api.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId: m,
          paymentInfo: { paymentId: factory.paymentId(`ab_full_${i}`) },
          eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
        }),
      )
      expect(r.body.ab_test_info?.arm).toBe('variant')
    }
  })

  test('no experiment means no arm stamp', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: m,
        paymentInfo: { paymentId: factory.paymentId('ab_none') },
        eligibleGatewayList: ['stripe', 'adyen'],
      }),
    )

    expect(r.status).toBe(200)
    expect(r.body.ab_test_info ?? null).toBeNull()
  })

  /**
   * The experiment being activated is not on its own enough to touch real traffic — the
   * `ab-test-real-payments` switch is the second, deliberate one.
   */
  test('an activated experiment does not intercept until real payments are enabled', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    await activateExperiment(api, m, 100)

    const r = await api.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId: m,
        paymentInfo: { paymentId: factory.paymentId('ab_gated') },
        eligibleGatewayList: ['stripe', 'adyen'],
      }),
    )

    expect(r.status).toBe(200)
    expect(r.body.ab_test_info ?? null).toBeNull()
  })
})
