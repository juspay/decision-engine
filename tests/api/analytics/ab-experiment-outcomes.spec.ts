import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { clickhouseInsert } from '../../helpers/clickhouse'
import { poll } from '../../helpers/poll'

/**
 * A/B experiment results attribute each payment's outcome from the statuses reported for it on
 * /update-gateway-score, joined to the experiment's routing event in ClickHouse. Nothing kept at
 * routing time has to survive until the webhook: an outcome reported days later still counts.
 *
 * Regression guard for outcomes that were dropped when the webhook outlived a 1h Redis record.
 */

const DAY_MS = 24 * 60 * 60 * 1000
const INGEST_TIMEOUT = 40_000

type Arm = 'control' | 'variant'

/** Score updates a payment receives, in order, and what the results should make of them. */
const OUTCOME_SCRIPTS: {
  name: string
  statuses: string[]
  resolved: boolean
  succeeded: boolean
  firstSucceeded: boolean
}[] = [
  { name: 'charged', statuses: ['CHARGED'], resolved: true, succeeded: true, firstSucceeded: true },
  {
    name: 'declined',
    statuses: ['AUTHORIZATION_FAILED'],
    resolved: true,
    succeeded: false,
    firstSucceeded: false,
  },
  {
    name: 'retried',
    statuses: ['AUTHORIZATION_FAILED', 'CHARGED'],
    resolved: true,
    succeeded: true,
    firstSucceeded: false,
  },
  {
    // A pending status is not an attempt's outcome, so it must not use up the first attempt.
    name: 'pending_then_charged',
    statuses: ['PENDING_VBV', 'CHARGED'],
    resolved: true,
    succeeded: true,
    firstSucceeded: true,
  },
  { name: 'no_webhook', statuses: [], resolved: false, succeeded: false, firstSucceeded: false },
]

/** How long the decider reuses the A/B feature flag before reading it again (`FEATURE_CONF_CACHE_TTL_MS`). */
const FEATURE_FLAG_CACHE_MS = 5_000

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

async function realPaymentsEnabled(api: ApiClient, merchantId: string): Promise<boolean> {
  const r = await api.getMerchantFeatures(merchantId)
  return r.body.features.some((f: any) => f.feature === 'ab-test-real-payments' && f.enabled)
}

/**
 * Turns on live-traffic A/B for the merchant and waits until the decider sees it. Every merchant's
 * flag lives in one shared row that is updated read-modify-write, so a merchant enabled by a
 * parallel spec at the same moment can be dropped: confirm it is still on once the decider's cache
 * has expired, and enable it again if not.
 */
async function enableRealPayments(api: ApiClient, merchantId: string) {
  for (let attempt = 0; attempt < 5; attempt++) {
    await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
    // Payments routed before the decider sees the flag skip the experiment.
    await sleep(FEATURE_FLAG_CACHE_MS + 500)
    if (await realPaymentsEnabled(api, merchantId)) return
  }
  throw new Error(`ab-test-real-payments did not stay enabled for ${merchantId}`)
}

async function enableExperiment(api: ApiClient, merchantId: string, data: Record<string, unknown>) {
  const experiment = await api.createRoutingAlgorithm({
    name: factory.ruleName('ab_outcomes'),
    description: 'A/B outcome attribution',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: { variant_split_pct: 49, min_sample_size: 1, guardrail_threshold_pp: 100, ...data },
    },
  })
  expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
  const id = experiment.body.rule_id as string
  await api.activateRoutingAlgorithm(merchantId, id)
  await enableRealPayments(api, merchantId)
  return id
}

async function ruleArms(api: ApiClient, merchantId: string) {
  const rule = async (gateway: string) => {
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(merchantId, {
        name: factory.ruleName(`ab_${gateway}`),
        gateway,
      }),
    )
    return created.body.rule_id as string
  }
  const control = await rule('stripe')
  await api.activateRoutingAlgorithm(merchantId, control)
  return {
    control: { rule_algorithm_id: control },
    variant: { rule_algorithm_id: await rule('razorpay') },
  }
}

/** A routing event as the experiment records it, written straight to ClickHouse at `atMs`. */
function routingEvent(
  merchantId: string,
  experimentId: string,
  paymentId: string,
  arm: Arm,
  atMs: number,
  extra: Record<string, unknown> = {},
) {
  return {
    event_id: `${paymentId}_${arm}_${atMs}`,
    api_flow: 'rule_based_routing',
    flow_type: 'routing_evaluate_ab_test',
    merchant_id: merchantId,
    payment_id: paymentId,
    lookup_key: paymentId,
    gateway: arm === 'variant' ? 'razorpay' : 'stripe',
    status: 'ab_test_decision',
    route: 'routing_evaluate',
    details: JSON.stringify({
      experiment_id: experimentId,
      variant_arm: arm,
      endpoint: 'evaluate',
      preview_kind: 'routing_evaluate_ab_test',
      routing_source: 'real_payment_intercept',
      ...extra,
    }),
    created_at_ms: atMs,
  }
}

async function sendStatuses(
  api: ApiClient,
  merchantId: string,
  paymentId: string,
  gateway: string,
  statuses: string[],
) {
  for (const status of statuses) {
    await api.updateGatewayScore(
      factory.updateGatewayScoreRequest({ merchantId, gateway, paymentId, status }),
    )
  }
}

function results(api: ApiClient, experimentId: string, qs: Record<string, unknown> = {}) {
  return api.raw('GET', `/analytics/experiment/${experimentId}/results`, {
    qs: { min_sample_size: 1, guardrail_threshold_pp: 100, ...qs },
  })
}

function transactions(api: ApiClient, experimentId: string, qs: Record<string, unknown> = {}) {
  return api.raw('GET', `/analytics/experiment/${experimentId}/transactions`, {
    qs: { page: 1, page_size: 100, ...qs },
  })
}

/** Per-arm counts the results should report for payments that ran `scripts`. */
function expectedArm(scripts: typeof OUTCOME_SCRIPTS) {
  const resolved = scripts.filter((s) => s.resolved).length
  const success = scripts.filter((s) => s.succeeded).length
  return {
    transaction_count: scripts.length,
    success_count: success,
    failure_count: resolved - success,
    first_attempt_auth_rate: scripts.filter((s) => s.firstSucceeded).length / scripts.length,
  }
}

test.describe('A/B experiment outcomes (API)', () => {
  test.setTimeout(90_000)

  test('a webhook arriving days after routing is attributed to its arm', async ({
    api,
    merchant,
    request,
  }) => {
    const m = merchant.id
    const experimentId = await enableExperiment(api, m, await ruleArms(api, m))

    // Routed two days ago: nothing from routing time is left when the webhooks arrive now.
    const routedAt = Date.now() - 2 * DAY_MS
    const payments = (['control', 'variant'] as Arm[]).flatMap((arm) =>
      OUTCOME_SCRIPTS.map((script, i) => ({
        arm,
        script,
        paymentId: factory.paymentId(`late_${arm}_${script.name}`),
        at: routedAt + i,
      })),
    )
    await clickhouseInsert(
      request,
      'analytics_domain_events',
      payments.map((p) => routingEvent(m, experimentId, p.paymentId, p.arm, p.at)),
    )

    for (const p of payments) {
      await sendStatuses(
        api,
        m,
        p.paymentId,
        p.arm === 'variant' ? 'razorpay' : 'stripe',
        p.script.statuses,
      )
    }

    const expected = expectedArm(OUTCOME_SCRIPTS)
    // The window ends a day ago, before any webhook: it bounds routing, not outcomes.
    const window = { start_ms: routedAt - DAY_MS, end_ms: routedAt + DAY_MS, endpoint: 'evaluate' }
    const settled = await poll(
      () => results(api, experimentId, window),
      (r) =>
        r.body.control?.success_count === expected.success_count &&
        r.body.variant?.success_count === expected.success_count,
      { message: 'late outcomes were not attributed', timeout: INGEST_TIMEOUT },
    )

    for (const arm of ['control', 'variant'] as Arm[]) {
      const metrics = settled.body[arm]
      expect(metrics.transaction_count, arm).toBe(expected.transaction_count)
      expect(metrics.success_count, arm).toBe(expected.success_count)
      expect(metrics.failure_count, arm).toBe(expected.failure_count)
      expect(metrics.auth_rate, arm).toBeCloseTo(
        expected.success_count / expected.transaction_count,
        9,
      )
      expect(metrics.first_attempt_auth_rate, arm).toBeCloseTo(expected.first_attempt_auth_rate, 9)
    }

    // A window that ends before the payments were routed holds none of them.
    const before = await results(api, experimentId, {
      start_ms: routedAt - 2 * DAY_MS,
      end_ms: routedAt - DAY_MS,
    })
    expect(before.body.control.transaction_count + before.body.variant.transaction_count).toBe(0)
  })

  test('payments routed through the experiment show their reported outcomes', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const experimentId = await enableExperiment(api, m, {
      ...(await ruleArms(api, m)),
      endpoints: ['evaluate'],
    })

    const routed: {
      paymentId: string
      gateway: string
      script: (typeof OUTCOME_SCRIPTS)[number]
    }[] = []
    for (const [i, script] of [...OUTCOME_SCRIPTS, ...OUTCOME_SCRIPTS].entries()) {
      const paymentId = factory.paymentId(`routed_${i}_${script.name}`)
      const evaluated = await api.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(
          m,
          {
            payment_method: { type: 'enum_variant', value: 'card' },
            amount: { type: 'number', value: 100 },
          },
          { payment_id: paymentId },
        ),
      )
      const gateway =
        evaluated.body.evaluated_output?.[0]?.gateway_name ?? evaluated.body.output?.gateway_name
      expect(['stripe', 'razorpay'], JSON.stringify(evaluated.body)).toContain(gateway)
      routed.push({ paymentId, gateway, script })
    }
    for (const r of routed) {
      await sendStatuses(api, m, r.paymentId, r.gateway, r.script.statuses)
    }

    const byArm = (arm: Arm) =>
      routed.filter((r) => (r.gateway === 'razorpay') === (arm === 'variant')).map((r) => r.script)
    const expected = {
      control: expectedArm(byArm('control')),
      variant: expectedArm(byArm('variant')),
    }
    const settled = await poll(
      () => results(api, experimentId, { endpoint: 'evaluate' }),
      (r) =>
        r.body.control?.success_count === expected.control.success_count &&
        r.body.variant?.success_count === expected.variant.success_count,
      { message: 'routed payments did not resolve', timeout: INGEST_TIMEOUT },
    )
    for (const arm of ['control', 'variant'] as Arm[]) {
      expect(settled.body[arm].transaction_count, arm).toBe(expected[arm].transaction_count)
      expect(settled.body[arm].failure_count, arm).toBe(expected[arm].failure_count)
      if (expected[arm].transaction_count > 0) {
        expect(settled.body[arm].first_attempt_auth_rate, arm).toBeCloseTo(
          expected[arm].first_attempt_auth_rate,
          9,
        )
      }
    }

    const txns = await transactions(api, experimentId, { endpoint: 'evaluate' })
    expect(txns.body.total).toBe(routed.length)
    for (const r of routed) {
      const row = txns.body.transactions.find((t: any) => t.payment_id === r.paymentId)
      const want = !r.script.resolved
        ? 'ab_test_decision'
        : r.script.succeeded
          ? 'success'
          : 'failure'
      expect(row?.status, r.paymentId).toBe(want)
      expect(row?.variant_arm, r.paymentId).toBe(r.gateway === 'razorpay' ? 'variant' : 'control')
    }
  })

  test('cost routing on the variant arm reports cost and savings per arm', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)
    const experimentId = await enableExperiment(api, m, {
      control: { sr: { enable_multi_objective: false } },
      // A tiny margin lets cost decide the ranking, so the variant runs cost routing.
      variant: { sr: { enable_multi_objective: true, margin: 0.001 } },
      endpoints: ['decide_gateway'],
    })

    const routed: { paymentId: string; gateway: string }[] = []
    for (let i = 0; i < 16; i++) {
      const paymentId = factory.paymentId(`cost_${i}`)
      const decided = await api.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId: m,
          eligibleGatewayList: ['stripe', 'adyen', 'checkout'],
          paymentInfo: { paymentId, amount: 100, paymentMethodType: 'CARD', paymentMethod: 'VISA' },
        }),
      )
      expect(decided.status, JSON.stringify(decided.body)).toBe(200)
      routed.push({ paymentId, gateway: decided.body.decided_gateway })
    }
    for (const r of routed) {
      await sendStatuses(api, m, r.paymentId, r.gateway, ['CHARGED'])
    }

    const settled = await poll(
      () => results(api, experimentId, { endpoint: 'decide_gateway' }),
      (r) =>
        (r.body.control?.success_count ?? 0) + (r.body.variant?.success_count ?? 0) ===
        routed.length,
      { message: 'cost experiment payments did not resolve', timeout: INGEST_TIMEOUT },
    )
    const { control, variant } = settled.body

    // Each payment's arm, as the experiment recorded it.
    const txns = await transactions(api, experimentId, { endpoint: 'decide_gateway' })
    const count = (arm: Arm) =>
      txns.body.transactions.filter((t: any) => t.variant_arm === arm).length
    expect(count('control') + count('variant')).toBe(routed.length)
    expect(count('control'), 'payments in the control arm').toBeGreaterThan(0)
    expect(count('variant'), 'payments in the variant arm').toBeGreaterThan(0)
    expect(control.transaction_count).toBe(count('control'))
    expect(variant.transaction_count).toBe(count('variant'))
    // Only the arm that ran cost routing has cost data; the other saved nothing.
    expect(control.avg_chosen_cost_bps).toBeNull()
    expect(control.total_cost_saved).toBeNull()
    expect(variant.avg_chosen_cost_bps).toBeGreaterThan(0)
    expect(variant.avg_cost_saved_bps).toBeGreaterThanOrEqual(0)
    expect(variant.total_cost_saved).toBeGreaterThanOrEqual(0)
    expect(settled.body.net_delta_bps).not.toBeNull()
  })

  test('a payment keeps its cost when a later decision on another endpoint has none', async ({
    api,
    merchant,
    request,
  }) => {
    const m = merchant.id
    const experimentId = await enableExperiment(api, m, await ruleArms(api, m))

    const paymentId = factory.paymentId('cost_two_endpoints')
    const routedAt = Date.now() - DAY_MS
    await clickhouseInsert(request, 'analytics_domain_events', [
      routingEvent(m, experimentId, paymentId, 'variant', routedAt, {
        endpoint: 'decide_gateway',
        chosen_cost_bps: 180,
        cost_saved_bps: 40,
        amount: 100,
      }),
      // Routed again afterwards on an endpoint that never runs cost routing.
      routingEvent(m, experimentId, paymentId, 'variant', routedAt + 1000),
    ])
    await sendStatuses(api, m, paymentId, 'razorpay', ['CHARGED'])

    const settled = await poll(
      () => results(api, experimentId),
      (r) => r.body.variant?.success_count === 1,
      { message: 'payment did not resolve', timeout: INGEST_TIMEOUT },
    )
    expect(settled.body.variant.transaction_count).toBe(1)
    expect(settled.body.variant.avg_chosen_cost_bps).toBeCloseTo(180, 9)
    expect(settled.body.variant.avg_cost_saved_bps).toBeCloseTo(40, 9)
    // 40 bps saved on a successful 100 payment.
    expect(settled.body.variant.total_cost_saved).toBeCloseTo(0.4, 9)
  })
})
