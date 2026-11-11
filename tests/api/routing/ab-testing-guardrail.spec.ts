import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'

/** The payment id's bucket in 0..100, as the engine assigns arms from it. */
function bucket(paymentId: string): bigint {
  let hash = 5381n
  for (const byte of Buffer.from(paymentId)) hash = (hash * 33n + BigInt(byte)) & ((1n << 64n) - 1n)
  return hash % 100n
}

/** A fresh payment id whose bucket is in `[from, to)`: variants own consecutive slices in order. */
function paymentInBuckets(from: number, to: number): string {
  for (let i = 0; i < 2000; i++) {
    const candidate = factory.paymentId('guardrail_variant')
    const b = bucket(candidate)
    if (b >= BigInt(from) && b < BigInt(to)) return candidate
  }
  throw new Error(`Unable to find a payment ID in buckets ${from}..${to}`)
}

function paymentForVariant(): string {
  return paymentInBuckets(0, 20)
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
      min_sample_size: 20, guardrail_threshold_pp: 10,
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
    // Feedback is processed after the response, so the stop lands shortly after.
    await expect.poll(() => experimentActive(api, merchant.id, experimentId), { timeout: 10_000, intervals: [200] }).toBe(false)
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

test('with several variants, feedback stops only the breached variant', async ({ api, merchant }) => {
  test.setTimeout(90_000)
  const baseline = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, { gateway: 'stripe' }))
  const control = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, { gateway: 'checkout' }))
  const failing = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, { gateway: 'adyen' }))
  const healthy = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, { gateway: 'razorpay' }))
  await api.activateRoutingAlgorithm(merchant.id, baseline.body.rule_id)
  const created = await api.createRoutingAlgorithm({
    name: factory.ruleName('guardrail_multi'), description: 'Feedback-triggered variant stop',
    created_by: merchant.id, algorithm_for: 'payment', metadata: {},
    algorithm: { type: 'ab_test', data: {
      control: { rule_algorithm_id: control.body.rule_id },
      variants: [
        // Buckets 0..20 and 20..40; control keeps 40..100.
        { id: 'failing', split_pct: 20, rule_algorithm_id: failing.body.rule_id },
        { id: 'healthy', split_pct: 20, rule_algorithm_id: healthy.body.rule_id },
      ],
      endpoints: ['evaluate'],
      min_sample_size: 20, guardrail_threshold_pp: 10,
    } },
  })
  expect(created.status, JSON.stringify(created.body)).toBe(200)
  const experimentId = created.body.rule_id as string
  await api.setMerchantFeature(merchant.id, 'ab-test-real-payments', true)
  await api.activateRoutingAlgorithm(merchant.id, experimentId)
  try {
    // Feature-flag reads are cached for five seconds. Wait for a real variant decision.
    await expect.poll(() => route(api, merchant.id, paymentInBuckets(0, 20)), { timeout: 15_000, intervals: [500] }).toBe('adyen')

    const controlPayment = paymentInBuckets(40, 100)
    const failingPayment = paymentInBuckets(0, 20)
    const healthyPayment = paymentInBuckets(20, 40)
    expect(await route(api, merchant.id, controlPayment)).toBe('checkout')
    expect(await route(api, merchant.id, failingPayment)).toBe('adyen')
    expect(await route(api, merchant.id, healthyPayment)).toBe('razorpay')

    await feedback(api, merchant.id, controlPayment, 'checkout', 'AUTHORIZED')
    await feedback(api, merchant.id, healthyPayment, 'razorpay', 'AUTHORIZED')
    await feedback(api, merchant.id, failingPayment, 'adyen', 'FAILURE')

    // Feedback is processed after the response, so the stop lands shortly after.
    const stoppedVariants = async () => {
      const listed = await api.listActiveRoutingAlgorithms(merchant.id)
      const stored = listed.body.find((algorithm: { id: string }) => algorithm.id === experimentId)
      if (!stored) return null
      const algorithm = typeof stored.algorithm_data === 'string' ? JSON.parse(stored.algorithm_data) : (stored.algorithm_data ?? stored.algorithm)
      return algorithm.data.variants.map((v: { id: string; stopped?: boolean }) => [v.id, v.stopped === true])
    }
    await expect.poll(stoppedVariants, { timeout: 10_000, intervals: [200] })
      .toEqual([['failing', true], ['healthy', false]])

    // The experiment keeps running. The breached variant's payments are untracked and route by the
    // active rule, like payments the experiment doesn't cover; the other variant keeps its own.
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(true)
    expect(await route(api, merchant.id, paymentInBuckets(0, 20))).toBe('stripe')
    expect(await route(api, merchant.id, paymentInBuckets(20, 40))).toBe('razorpay')

    // A deliberate reactivation starts a new observation period with every variant running.
    await api.raw('POST', '/routing/deactivate', {
      body: { created_by: merchant.id, routing_algorithm_id: experimentId },
    })
    await api.activateRoutingAlgorithm(merchant.id, experimentId)
    expect(await route(api, merchant.id, paymentInBuckets(0, 20))).toBe('adyen')
  } finally {
    await api.raw('POST', '/routing/deactivate', {
      failOnStatusCode: false,
      body: { created_by: merchant.id, routing_algorithm_id: experimentId },
    })
  }
})
