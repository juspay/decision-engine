import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'

function deterministicVariant(paymentId: string, splitPct: number): boolean {
  let hash = 5381n
  for (const byte of Buffer.from(paymentId)) hash = (hash * 33n + BigInt(byte)) & ((1n << 64n) - 1n)
  return hash % 100n < BigInt(splitPct)
}

function paymentForVariant(): string {
  for (let i = 0; i < 1000; i++) {
    const candidate = factory.paymentId('guardrail_variant')
    if (deterministicVariant(candidate, 20)) return candidate
  }
  throw new Error('Unable to find a deterministic variant payment ID')
}

async function createExperiment(api: ApiClient, merchantId: string) {
  const baseline = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'stripe' }))
  const control = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'checkout' }))
  const variant = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'adyen' }))
  await api.activateRoutingAlgorithm(merchantId, baseline.body.rule_id)
  const experiment = await api.createRoutingAlgorithm({
    name: factory.ruleName('guardrail'), description: 'Feedback-triggered experiment stop',
    created_by: merchantId, algorithm_for: 'payment', metadata: {},
    algorithm: { type: 'ab_test', data: {
      control: { rule_algorithm_id: control.body.rule_id },
      variant: { rule_algorithm_id: variant.body.rule_id },
      endpoints: ['evaluate'], variant_split_pct: 20,
      min_sample_size: 1000, guardrail_threshold_pp: 10,
    } },
  })
  await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
  await api.activateRoutingAlgorithm(merchantId, experiment.body.rule_id)
  return experiment.body.rule_id as string
}

async function route(api: ApiClient, merchantId: string, paymentId: string): Promise<string> {
  const result = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(merchantId, {}, { payment_id: paymentId }))
  return result.body.output.connector.gateway_name
}

async function feedback(api: ApiClient, merchantId: string, paymentId: string, gateway: string, status: 'AUTHORIZED' | 'FAILURE') {
  await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId, paymentId, gateway, status }))
}

async function experimentActive(api: ApiClient, merchantId: string, experimentId: string): Promise<boolean> {
  const active = await api.listActiveRoutingAlgorithms(merchantId)
  return active.body.some((algorithm: { id: string }) => algorithm.id === experimentId)
}

test('payment feedback stops a breached experiment and leaves the active rule in place', async ({ api, merchant }) => {
  test.setTimeout(90_000)
  const experimentId = await createExperiment(api, merchant.id)
  try {
    // Feature-flag reads are cached for five seconds. Wait for a real variant decision.
    await expect.poll(() => route(api, merchant.id, paymentForVariant()), { timeout: 15_000, intervals: [500] }).toBe('adyen')

    const controls: string[] = []
    const variants: string[] = []
    for (let i = 0; (controls.length < 1 || variants.length < 1) && i < 100; i++) {
      const paymentId = factory.paymentId('guardrail')
      const gateway = await route(api, merchant.id, paymentId)
      if (gateway === 'checkout' && controls.length < 1) controls.push(paymentId)
      if (gateway === 'adyen' && variants.length < 1) variants.push(paymentId)
    }
    expect(controls).toHaveLength(1)
    expect(variants).toHaveLength(1)

    await feedback(api, merchant.id, controls[0], 'checkout', 'AUTHORIZED')
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(true) // Both arms need an outcome.
    await feedback(api, merchant.id, variants[0], 'adyen', 'FAILURE')
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(false)
    expect(await route(api, merchant.id, paymentForVariant())).toBe('stripe')

    // Duplicate feedback and a successful retry cannot restart the experiment.
    await feedback(api, merchant.id, variants[0], 'adyen', 'FAILURE')
    await feedback(api, merchant.id, variants[0], 'adyen', 'AUTHORIZED')
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(false)

    await api.activateRoutingAlgorithm(merchant.id, experimentId)
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(true)
    expect(await route(api, merchant.id, paymentForVariant())).toBe('adyen')
  } finally {
    await api.raw('POST', '/routing/deactivate', {
      failOnStatusCode: false,
      body: { created_by: merchant.id, routing_algorithm_id: experimentId },
    })
  }
})
