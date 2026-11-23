import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { redisKeys, redisListLength, redisPfcount, redisTtl } from '../../helpers/redis'

/**
 * Behaviour that must hold when the engine runs as several instances sharing one Redis and
 * database, as it does in production. Pod A is the suite's server (API_BASE_URL); pod B is a second
 * instance (API_BASE_URL_B) that playwright.config.ts starts beside it. Each test routes on one pod
 * and reports or reconfigures on the other, so state kept in one instance's memory fails them.
 */

const POD_A = process.env.API_BASE_URL || 'http://localhost:8080'
const POD_B = process.env.API_BASE_URL_B || 'http://localhost:8099'
const PODS = [POD_A, POD_B]

test.beforeAll(async ({ request }) => {
  const reachable = await request.get(`${POD_B}/health`).then((r) => r.ok(), () => false)
  // CI always starts pod B; a local run against a single server skips instead of failing.
  if (!process.env.CI) test.skip(!reachable, `second instance not running at ${POD_B}`)
  expect(reachable, `second instance not running at ${POD_B}`).toBe(true)
})

/** The payment id's bucket in 0..100, as the engine assigns arms from it. */
function bucket(paymentId: string): bigint {
  let hash = 5381n
  for (const byte of Buffer.from(paymentId)) hash = (hash * 33n + BigInt(byte)) & ((1n << 64n) - 1n)
  return hash % 100n
}

/** A fresh payment id whose bucket is in `[from, to)`. */
function paymentIn(from: number, to: number): string {
  for (let i = 0; i < 5000; i++) {
    const id = factory.paymentId('multi_pod')
    const b = bucket(id)
    if (b >= BigInt(from) && b < BigInt(to)) return id
  }
  throw new Error(`Unable to find a payment ID in buckets ${from}..${to}`)
}

// With a 20% variant, buckets 0..20 are the variant's and 20..100 control's.
const variantPayment = () => paymentIn(0, 20)
const controlPayment = () => paymentIn(20, 100)

async function route(api: ApiClient, pod: string, merchantId: string, paymentId: string): Promise<string> {
  const r = await api.raw('POST', `${pod}/routing/evaluate`, {
    body: factory.ruleEvaluatePayload(merchantId, {}, { payment_id: paymentId }),
  })
  return r.body.output.connector.gateway_name
}

async function feedback(api: ApiClient, pod: string, merchantId: string, paymentId: string, gateway: string, status: string) {
  const r = await api.raw('POST', `${pod}/update-gateway-score`, {
    body: factory.updateGatewayScoreRequest({ merchantId, paymentId, gateway, status }),
  })
  expect(r.status).toBe(200)
}

async function experimentActive(api: ApiClient, merchantId: string, experimentId: string): Promise<boolean> {
  const active = await api.listActiveRoutingAlgorithms(merchantId)
  return active.body.some((algorithm: { id: string }) => algorithm.id === experimentId)
}

async function deactivate(api: ApiClient, pod: string, merchantId: string, experimentId: string) {
  await api.raw('POST', `${pod}/routing/deactivate`, {
    failOnStatusCode: false,
    body: { created_by: merchantId, routing_algorithm_id: experimentId },
  })
}

/** Distinct payments counted per arm in the experiment's current observation period. */
async function guardrailCounts(merchantId: string, experimentId: string): Promise<Record<string, number>> {
  const counts: Record<string, number> = {}
  for (const key of await redisKeys(`ab_guardrail_{${merchantId}_${experimentId}}_*`)) {
    // The period marker and the routed-payment sets aren't counts.
    if (key.endsWith('_run') || key.endsWith('_routed')) continue
    // ab_guardrail_{m_e}_<run>_<arm>_<txns|succ>
    counts[key.split('}_')[1].replace(/^\d+_/, '')] = await redisPfcount(key)
  }
  return counts
}

/**
 * A baseline rule (stripe) is active; the experiment splits /routing/evaluate between control
 * (checkout) and a 20% variant (adyen). Returns the experiment id, not yet activated.
 */
async function createExperiment(
  api: ApiClient,
  merchantId: string,
  minSampleSize: number,
  extra: { endpoints?: string[]; max_sample_size?: number } = {},
): Promise<string> {
  const baseline = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'stripe' }))
  const control = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'checkout' }))
  const variant = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway: 'adyen' }))
  await api.activateRoutingAlgorithm(merchantId, baseline.body.rule_id)
  const experiment = await api.createRoutingAlgorithm({
    name: factory.ruleName('multi_pod'), description: 'Multi-instance guardrail',
    created_by: merchantId, algorithm_for: 'payment', metadata: {},
    algorithm: { type: 'ab_test', data: {
      control: { rule_algorithm_id: control.body.rule_id },
      variant: { rule_algorithm_id: variant.body.rule_id },
      endpoints: ['evaluate'], variant_split_pct: 20,
      min_sample_size: minSampleSize, guardrail_threshold_pp: 10,
      ...extra,
    } },
  })
  expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
  await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
  return experiment.body.rule_id as string
}

/** Activates on `pod`, then waits until both pods route the variant's payments to it. */
async function activate(api: ApiClient, pod: string, merchantId: string, experimentId: string) {
  const r = await api.raw('POST', `${pod}/routing/activate`, {
    body: { created_by: merchantId, routing_algorithm_id: experimentId },
  })
  expect(r.status, JSON.stringify(r.body)).toBe(200)
  // Wait until both instances route the variant's payments to it.
  for (const each of PODS) {
    await expect.poll(() => route(api, each, merchantId, variantPayment()), { timeout: 20_000, intervals: [500] }).toBe('adyen')
  }
}

test.describe('A/B guardrail across instances (API)', () => {
  test('feedback on another instance than routing stops a breached variant', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 20)
    try {
      await activate(api, POD_A, m, experimentId)
      const control = controlPayment()
      const variant = variantPayment()
      expect(await route(api, POD_A, m, control)).toBe('checkout')
      expect(await route(api, POD_A, m, variant)).toBe('adyen')

      await feedback(api, POD_B, m, control, 'checkout', 'AUTHORIZED')
      await feedback(api, POD_B, m, variant, 'adyen', 'FAILURE')
      await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000, intervals: [300] }).toBe(false)

      // Both instances stop routing to the variant.
      for (const pod of PODS) expect(await route(api, pod, m, variantPayment())).toBe('stripe')
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('outcomes reported to different instances add up to the minimum sample', async ({ api, merchant }) => {
    const m = merchant.id
    // A minimum sample of 40 needs 4 resolved payments before the guardrail judges.
    const experimentId = await createExperiment(api, m, 40)
    try {
      await activate(api, POD_A, m, experimentId)
      const controls = [controlPayment(), controlPayment()]
      const variants = [variantPayment(), variantPayment()]
      for (const id of controls) await route(api, POD_A, m, id)
      for (const id of variants) await route(api, POD_B, m, id)

      await feedback(api, POD_A, m, controls[0], 'checkout', 'AUTHORIZED')
      await feedback(api, POD_B, m, variants[0], 'adyen', 'FAILURE')
      await expect.poll(() => guardrailCounts(m, experimentId)).toEqual({
        control_txns: 1, control_succ: 1, variant_txns: 1,
      })
      expect(await experimentActive(api, m, experimentId)).toBe(true)

      await feedback(api, POD_B, m, controls[1], 'checkout', 'AUTHORIZED')
      await feedback(api, POD_A, m, variants[1], 'adyen', 'FAILURE')
      await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000, intervals: [300] }).toBe(false)
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('repeated and retried feedback across instances counts a payment once', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 400)
    try {
      await activate(api, POD_A, m, experimentId)
      const variant = variantPayment()
      await route(api, POD_A, m, variant)

      await feedback(api, POD_A, m, variant, 'adyen', 'FAILURE')
      await feedback(api, POD_B, m, variant, 'adyen', 'FAILURE')
      await feedback(api, POD_B, m, variant, 'adyen', 'AUTHORIZED')
      await feedback(api, POD_A, m, variant, 'adyen', 'AUTHORIZED')
      await expect.poll(() => guardrailCounts(m, experimentId)).toEqual({ variant_txns: 1, variant_succ: 1 })
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('a payment routed before activation is not counted when its feedback arrives after', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 400)
    try {
      const early = variantPayment()
      expect(await route(api, POD_A, m, early)).toBe('stripe')
      await activate(api, POD_B, m, experimentId)
      const late = variantPayment()
      expect(await route(api, POD_A, m, late)).toBe('adyen')

      await feedback(api, POD_B, m, early, 'stripe', 'FAILURE')
      await feedback(api, POD_B, m, late, 'adyen', 'FAILURE')
      await expect.poll(() => guardrailCounts(m, experimentId)).toEqual({ variant_txns: 1 })
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('a reactivation on one instance starts a fresh period the other instance judges in', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 20)
    try {
      await activate(api, POD_A, m, experimentId)
      const control = controlPayment()
      const variant = variantPayment()
      await route(api, POD_A, m, control)
      await route(api, POD_A, m, variant)
      await feedback(api, POD_B, m, control, 'checkout', 'AUTHORIZED')
      await feedback(api, POD_A, m, variant, 'adyen', 'FAILURE')
      await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000, intervals: [300] }).toBe(false)

      // The breach was judged on the earlier period's outcomes, which must not stop the new one.
      await activate(api, POD_B, m, experimentId)
      const nextControl = controlPayment()
      await route(api, POD_A, m, nextControl)
      await feedback(api, POD_A, m, nextControl, 'checkout', 'AUTHORIZED')
      await expect.poll(() => guardrailCounts(m, experimentId)).toMatchObject({ control_txns: 1, control_succ: 1 })
      expect(await experimentActive(api, m, experimentId)).toBe(true)
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('with several variants, feedback on another instance stops only the breached one', async ({ api, merchant }) => {
    const m = merchant.id
    const baseline = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'stripe' }))
    const control = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'checkout' }))
    const failing = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'adyen' }))
    const healthy = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'razorpay' }))
    await api.activateRoutingAlgorithm(m, baseline.body.rule_id)
    const created = await api.createRoutingAlgorithm({
      name: factory.ruleName('multi_pod_variants'), description: 'Multi-instance variant stop',
      created_by: m, algorithm_for: 'payment', metadata: {},
      algorithm: { type: 'ab_test', data: {
        control: { rule_algorithm_id: control.body.rule_id },
        // Buckets 0..20 and 20..40; control keeps 40..100.
        variants: [
          { id: 'failing', split_pct: 20, rule_algorithm_id: failing.body.rule_id },
          { id: 'healthy', split_pct: 20, rule_algorithm_id: healthy.body.rule_id },
        ],
        endpoints: ['evaluate'], min_sample_size: 20, guardrail_threshold_pp: 10,
      } },
    })
    expect(created.status, JSON.stringify(created.body)).toBe(200)
    const experimentId = created.body.rule_id as string
    await api.setMerchantFeature(m, 'ab-test-real-payments', true)
    try {
      await api.activateRoutingAlgorithm(m, experimentId)
      for (const pod of PODS) {
        await expect.poll(() => route(api, pod, m, paymentIn(0, 20)), { timeout: 20_000, intervals: [500] }).toBe('adyen')
      }
      const controlId = paymentIn(40, 100)
      const failingId = paymentIn(0, 20)
      const healthyId = paymentIn(20, 40)
      await route(api, POD_A, m, controlId)
      await route(api, POD_B, m, failingId)
      await route(api, POD_A, m, healthyId)

      await feedback(api, POD_B, m, controlId, 'checkout', 'AUTHORIZED')
      await feedback(api, POD_A, m, healthyId, 'razorpay', 'AUTHORIZED')
      await feedback(api, POD_A, m, failingId, 'adyen', 'FAILURE')

      const stoppedVariants = async () => {
        const listed = await api.listActiveRoutingAlgorithms(m)
        const stored = listed.body.find((algorithm: { id: string }) => algorithm.id === experimentId)
        if (!stored) return null
        const algorithm = typeof stored.algorithm_data === 'string' ? JSON.parse(stored.algorithm_data) : (stored.algorithm_data ?? stored.algorithm)
        return algorithm.data.variants.map((v: { id: string; stopped?: boolean }) => [v.id, v.stopped === true])
      }
      await expect.poll(stoppedVariants, { timeout: 15_000, intervals: [300] }).toEqual([['failing', true], ['healthy', false]])

      // Both instances route the stopped variant's payments by the active rule and keep the other.
      for (const pod of PODS) {
        expect(await route(api, pod, m, paymentIn(0, 20))).toBe('stripe')
        expect(await route(api, pod, m, paymentIn(20, 40))).toBe('razorpay')
      }
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('a rule-only experiment payment is counted without training SR scores', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 400)
    try {
      await activate(api, POD_A, m, experimentId)
      const variant = variantPayment()
      expect(await route(api, POD_A, m, variant)).toBe('adyen')
      await feedback(api, POD_B, m, variant, 'adyen', 'FAILURE')

      await expect.poll(() => guardrailCounts(m, experimentId)).toEqual({ variant_txns: 1 })
      // Its context exists only for the guardrail, so no success-rate score was learned from it.
      expect(await redisKeys(`{gw_sr_v3_score_${m}_*`)).toEqual([])
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })
})

test.describe('A/B guardrail outcomes and completion across instances (API)', () => {
  /** Routes a payment on `pod`'s /routing/hybrid; returns the connector its rule output chose. */
  async function routeHybrid(api: ApiClient, pod: string, merchantId: string, paymentId: string): Promise<string> {
    const r = await api.raw('POST', `${pod}/routing/hybrid`, {
      body: {
        // The same parameters /routing/evaluate is called with in these tests. A connector the rule
        // chooses is served only when the caller can route to it.
        static_routing_request: {
          ...factory.ruleEvaluatePayload(merchantId, {}, { payment_id: paymentId }),
          fallback_output: ['stripe', 'checkout', 'adyen'].map((gateway) => factory.gatewayConnector(gateway)),
        },
        dynamic_routing_request: factory.srDecideGatewayRequest({
          merchantId,
          paymentInfo: { ...factory.paymentInfo(), paymentId },
        }),
      },
    })
    expect(r.status, JSON.stringify(r.body).slice(0, 500)).toBe(200)
    return r.body.evaluated_connectors[0].gateway_name
  }

  test('an outcome counts however late it arrives, after the payment\'s scoring context expired', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)
    // Rule-routed hybrid payments get a scoring context, kept for three seconds.
    await api.setMerchantFeature(m, 'sr-scores-from-rule-routing', true)
    const ttl = await api.raw('PUT', `${POD_A}/admin/merchant-account/${m}/scoring-data-ttl`, { body: { ttl_secs: 3 } })
    expect(ttl.status, JSON.stringify(ttl.body)).toBe(200)
    const experimentId = await createExperiment(api, m, 400, { endpoints: ['hybrid_routing'] })
    try {
      await api.activateRoutingAlgorithm(m, experimentId)
      await expect.poll(() => routeHybrid(api, POD_A, m, variantPayment()), { timeout: 20_000, intervals: [500] }).toBe('adyen')

      const variant = variantPayment()
      expect(await routeHybrid(api, POD_A, m, variant)).toBe('adyen')
      await expect.poll(() => redisKeys(`gateway_scoring_data_${m}_${variant}`)).toEqual([`gateway_scoring_data_${m}_${variant}`])
      // The context expires; the payment's outcome still belongs to the experiment.
      await expect.poll(() => redisKeys(`gateway_scoring_data_${m}_${variant}`), { timeout: 10_000, intervals: [500] }).toEqual([])
      await feedback(api, POD_B, m, variant, 'adyen', 'AUTHORIZED')
      await expect.poll(() => guardrailCounts(m, experimentId)).toMatchObject({ variant_txns: 1, variant_succ: 1 })
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test('an experiment completes once it has routed its max_sample_size payments', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 2, { max_sample_size: 3 })
    try {
      // Activation routes a variant payment on each instance; one more reaches the cap.
      await activate(api, POD_A, m, experimentId)
      const control = controlPayment()
      expect(await route(api, POD_B, m, control)).toBe('checkout')
      expect(await experimentActive(api, m, experimentId)).toBe(true)

      await feedback(api, POD_A, m, control, 'checkout', 'AUTHORIZED')
      await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000, intervals: [300] }).toBe(false)
      // Both instances route every payment by the active rule again.
      for (const pod of PODS) {
        expect(await route(api, pod, m, variantPayment())).toBe('stripe')
        expect(await route(api, pod, m, controlPayment())).toBe('stripe')
      }
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })
})

test.describe('Routing configuration across instances (API)', () => {
  test('a rule activated on one instance routes on the other', async ({ api, merchant }) => {
    const m = merchant.id
    const first = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'stripe' }))
    const second = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'adyen' }))

    await api.raw('POST', `${POD_A}/routing/activate`, { body: { created_by: m, routing_algorithm_id: first.body.rule_id } })
    for (const pod of PODS) expect(await route(api, pod, m, factory.paymentId('multi_pod_rule'))).toBe('stripe')

    // Pod A has the first rule cached; switching on pod B must reach it.
    await api.raw('POST', `${POD_B}/routing/activate`, { body: { created_by: m, routing_algorithm_id: second.body.rule_id } })
    for (const pod of PODS) {
      await expect.poll(() => route(api, pod, m, factory.paymentId('multi_pod_rule')), { timeout: 10_000, intervals: [300] }).toBe('adyen')
    }
  })
})

test.describe('Success-rate scoring across instances (API)', () => {
  test('a payment decided on one instance is scored from feedback on the other', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)

    const paymentId = factory.paymentId('multi_pod_sr')
    const decided = await api.raw('POST', `${POD_A}/decide-gateway`, {
      body: factory.srDecideGatewayRequest({
        merchantId: m,
        eliminationEnabled: false,
        paymentInfo: { ...factory.paymentInfo(), paymentId, paymentMethod: 'card', paymentMethodType: 'credit', cardType: 'CREDIT' },
      }),
    })
    expect(decided.status, JSON.stringify(decided.body).slice(0, 500)).toBe(200)
    const gateway: string = decided.body.decided_gateway

    // Pod B finds the scoring context pod A saved and learns into the shared score keys.
    await feedback(api, POD_B, m, paymentId, gateway, 'CHARGED')
    const base = `{gw_sr_v3_score_${m}_ORDER_PAYMENT_CREDIT_CARD_${gateway}_}`
    await expect.poll(() => redisKeys(`{gw_sr_v3_score_${m}_*`)).toEqual([`${base}queue`, `${base}score`])
  })
})

/**
 * Each instance caches service configs and merchant accounts in memory for minutes; a change made
 * through one instance reaches the others by Redis pub/sub invalidation. Every test first makes pod
 * B cache the old value, changes it through pod A, and expects pod B to act on the new one within
 * seconds, far inside the cache TTL, which only the invalidation explains.
 */
test.describe('Cached configuration across instances (API)', () => {
  const PROPAGATION = { timeout: 5_000, intervals: [200] }

  /** Decides one payment on `pod`; `paymentInfo` overrides the default card payment. */
  async function decide(api: ApiClient, pod: string, merchantId: string, paymentInfo: Record<string, unknown>) {
    const r = await api.raw('POST', `${pod}/decide-gateway`, {
      body: factory.srDecideGatewayRequest({
        merchantId,
        eliminationEnabled: false,
        paymentInfo: { ...factory.paymentInfo(), ...paymentInfo },
      }),
    })
    expect(r.status, JSON.stringify(r.body).slice(0, 500)).toBe(200)
    return r.body.decided_gateway as string
  }

  test('a merchant feature toggled on one instance applies on the other', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)
    const rule = await api.createRoutingAlgorithm(factory.singleRoutingPayload(m, { gateway: 'stripe' }))
    await api.activateRoutingAlgorithm(m, rule.body.rule_id)
    const hybrid = () => api.raw('POST', `${POD_B}/routing/hybrid`, {
      body: {
        static_routing_request: {
          created_by: m,
          parameters: {
            payment_method: { type: 'enum_variant', value: 'card' },
            amount: { type: 'number', value: 100 },
          },
          fallback_output: [factory.gatewayConnector('stripe')],
        },
        dynamic_routing_request: factory.srDecideGatewayRequest({ merchantId: m }),
      },
    }).then((r) => r.body.dynamic_routing?.status ?? 'skipped')

    // Pod B caches sr-routing as off.
    expect(await hybrid()).toBe('skipped')

    const on = await api.raw('POST', `${POD_A}/merchant-account/${m}/features/sr-routing`, { body: { enabled: true } })
    expect(on.status).toBe(200)
    await expect.poll(hybrid, PROPAGATION).toBe('success')

    const off = await api.raw('POST', `${POD_A}/merchant-account/${m}/features/sr-routing`, { body: { enabled: false } })
    expect(off.status).toBe(200)
    await expect.poll(hybrid, PROPAGATION).toBe('skipped')
  })

  test('live A/B testing toggled on one instance applies on the other', async ({ api, merchant }) => {
    const m = merchant.id
    const experimentId = await createExperiment(api, m, 400)
    try {
      // Both instances route the variant's payments to it, so pod B has the flag cached as on.
      await activate(api, POD_A, m, experimentId)
      const toggle = (enabled: boolean) =>
        api.raw('POST', `${POD_A}/merchant-account/${m}/features/ab-test-real-payments`, { body: { enabled } })
      const routedOnB = () => route(api, POD_B, m, variantPayment())
      // Well inside even the shortest cache lifetime the flag has had, so only invalidation explains it.
      const promptly = { timeout: 1_500, intervals: [100] }

      expect((await toggle(false)).status).toBe(200)
      await expect.poll(routedOnB, promptly).toBe('stripe')

      expect((await toggle(true)).status).toBe(200)
      await expect.poll(routedOnB, promptly).toBe('adyen')
    } finally {
      await deactivate(api, POD_A, m, experimentId)
    }
  })

  test("a merchant's scoring-context TTL set on one instance applies on the other", async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)
    const contextTtl = async () => {
      const paymentId = factory.paymentId('multi_pod_ttl')
      await decide(api, POD_B, m, { paymentId })
      return redisTtl(`gateway_scoring_data_${m}_${paymentId}`)
    }

    // Pod B caches the merchant as having no TTL of its own: contexts get the default.
    const defaultTtl = await contextTtl()
    expect(defaultTtl).toBeGreaterThan(0)

    const set = await api.raw('PUT', `${POD_A}/admin/merchant-account/${m}/scoring-data-ttl`, {
      body: { ttl_secs: defaultTtl + 3600 },
    })
    expect(set.status, JSON.stringify(set.body)).toBe(200)
    await expect.poll(contextTtl, PROPAGATION).toBeGreaterThan(defaultTtl + 3000)
  })

  test("a merchant's success-rate config updated on one instance applies on the other", async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m, { defaultBucketSize: 200, subLevelInputConfig: [] })
    /** Routes and reports a fresh payment of `paymentMethodType` on pod B; returns its score queue length. */
    const learnedBucket = async (paymentMethodType: string) => {
      const paymentId = factory.paymentId('multi_pod_bucket')
      const gateway = await decide(api, POD_B, m, {
        paymentId, paymentMethod: 'card', paymentMethodType, cardType: paymentMethodType.toUpperCase(),
      })
      await feedback(api, POD_B, m, paymentId, gateway, 'CHARGED')
      const queue = `{gw_sr_v3_score_${m}_ORDER_PAYMENT_${paymentMethodType.toUpperCase()}_CARD_${gateway}_}queue`
      await expect.poll(() => redisListLength(queue)).toBeGreaterThan(0)
      return redisListLength(queue)
    }

    // Pod B caches the merchant's config with a bucket of 200.
    expect(await learnedBucket('credit')).toBe(200)

    const updated = await api.raw('POST', `${POD_A}/rule/update`, {
      body: { merchant_id: m, config: { type: 'successRate', data: factory.srConfigData({ defaultBucketSize: 60, subLevelInputConfig: [] }) } },
    })
    expect(updated.status, JSON.stringify(updated.body)).toBe(200)
    // A new segment's queue is created at the bucket size pod B has for the merchant.
    await expect.poll(() => learnedBucket('debit'), PROPAGATION).toBe(60)
  })
})
