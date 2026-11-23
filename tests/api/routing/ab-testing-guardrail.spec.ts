import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { abGuardrailCounts, redisKeys, redisTtl } from '../../helpers/redis'
import { decidedConnector } from '../../helpers/decide'

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
      // Large enough that the experiment doesn't complete during the test.
      endpoints: ['hybrid_routing'], variant_split_pct: 20,
      min_sample_size: 1000, guardrail_threshold_pp: 10,
    } },
  })
  await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
  await api.activateRoutingAlgorithm(merchantId, experiment.body.rule_id)
  return experiment.body.rule_id as string
}

/** Routes a payment through /decide; returns the connector chosen. */
async function route(api: ApiClient, _merchantId: string, paymentId: string): Promise<string> {
  return decidedConnector(api, paymentId)
}

async function feedback(api: ApiClient, merchantId: string, paymentId: string, gateway: string, status: 'AUTHORIZED' | 'FAILURE') {
  await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId, paymentId, gateway, status }))
}

/**
 * Routes every payment, expecting `gateway`, then reports `status` for each. Routing comes first so
 * a stop the outcomes trigger can't change where the rest are routed.
 */
async function resolve(api: ApiClient, merchantId: string, paymentIds: string[], gateway: string, status: 'AUTHORIZED' | 'FAILURE') {
  for (const paymentId of paymentIds) expect(await route(api, merchantId, paymentId)).toBe(gateway)
  for (const paymentId of paymentIds) await feedback(api, merchantId, paymentId, gateway, status)
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

    // Every control payment succeeds and every variant payment fails: a drop clear on few payments,
    // but the guardrail judges only once each arm has 30 outcomes. 20 is under that; 40 is over it
    // with room for the counts reading a payment or two low.
    const controls = Array.from({ length: 40 }, () => paymentInBuckets(20, 100))
    const variants = Array.from({ length: 40 }, () => paymentForVariant())
    await resolve(api, merchant.id, controls, 'checkout', 'AUTHORIZED')
    await resolve(api, merchant.id, variants.slice(0, 20), 'adyen', 'FAILURE')
    await expect.poll(async () => (await abGuardrailCounts(merchant.id, experimentId)).variant_txns ?? 0)
      .toBeGreaterThanOrEqual(19)
    expect(await experimentActive(api, merchant.id, experimentId)).toBe(true)

    await resolve(api, merchant.id, variants.slice(20), 'adyen', 'FAILURE')
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

test('every guardrail key expires, the observation period included', async ({ api, merchant }) => {
  const experimentId = await createExperiment(api, merchant.id)
  try {
    await expect.poll(() => route(api, merchant.id, paymentForVariant()), { timeout: 15_000, intervals: [500] }).toBe('adyen')
    const payment = paymentForVariant()
    expect(await route(api, merchant.id, payment)).toBe('adyen')
    await feedback(api, merchant.id, payment, 'adyen', 'AUTHORIZED')
    await expect.poll(() => abGuardrailCounts(merchant.id, experimentId)).toMatchObject({ variant_txns: 1 })

    // The period key, the routed set and both outcome counts: each set to the configured 30 days.
    const keys = await redisKeys(`ab_guardrail_{${merchant.id}_${experimentId}}_*`)
    expect(keys.some((key) => key.endsWith('_run'))).toBe(true)
    for (const key of keys) {
      const ttl = await redisTtl(key)
      expect(ttl, key).toBeGreaterThan(2_592_000 - 60)
      expect(ttl, key).toBeLessThanOrEqual(2_592_000)
    }
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
      endpoints: ['hybrid_routing'],
      min_sample_size: 1000, guardrail_threshold_pp: 10,
    } },
  })
  expect(created.status, JSON.stringify(created.body)).toBe(200)
  const experimentId = created.body.rule_id as string
  await api.setMerchantFeature(merchant.id, 'ab-test-real-payments', true)
  await api.activateRoutingAlgorithm(merchant.id, experimentId)
  try {
    // Feature-flag reads are cached for five seconds. Wait for a real variant decision.
    await expect.poll(() => route(api, merchant.id, paymentInBuckets(0, 20)), { timeout: 15_000, intervals: [500] }).toBe('adyen')

    // 40 outcomes per arm: over the 30 the guardrail needs, with room for the counts reading low.
    const forty = (from: number, to: number) => Array.from({ length: 40 }, () => paymentInBuckets(from, to))
    await resolve(api, merchant.id, forty(40, 100), 'checkout', 'AUTHORIZED')
    await resolve(api, merchant.id, forty(20, 40), 'razorpay', 'AUTHORIZED')
    await resolve(api, merchant.id, forty(0, 20), 'adyen', 'FAILURE')

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
