import { test, expect, factory } from '../../fixtures/test'
import type { Page } from '@playwright/test'
import type { ApiClient } from '../../fixtures/api-client'
import { decidedConnector } from '../../helpers/decide'

/**
 * An experiment's page through its lifecycle, against the real engine: payments are routed through
 * /decide and their outcomes reported, and the page shows what the engine did.
 *
 * - A variant whose drop is clear breaches the guardrail and stops the experiment.
 * - An experiment completes once its outcomes reach the sample target, or without a verdict once
 *   it has routed twice the target while outcomes don't arrive.
 * - With several variants, only the one that breaches stops; the others keep running.
 */
test.use({ viewport: { width: 1600, height: 1000 } })

/** The payment id's bucket in 0..100, as the engine assigns arms from it. */
function bucket(paymentId: string): bigint {
  let hash = 5381n
  for (const byte of Buffer.from(paymentId)) hash = (hash * 33n + BigInt(byte)) & ((1n << 64n) - 1n)
  return hash % 100n
}

/** A fresh payment id whose bucket is in `[from, to)`: variants own consecutive slices in order. */
function paymentIn(from: number, to: number): string {
  for (let i = 0; i < 5000; i++) {
    const id = factory.paymentId('ab_lifecycle')
    const b = bucket(id)
    if (b >= BigInt(from) && b < BigInt(to)) return id
  }
  throw new Error(`Unable to find a payment ID in buckets ${from}..${to}`)
}

async function route(api: ApiClient, _merchantId: string, paymentId: string): Promise<string> {
  return decidedConnector(api, paymentId)
}

/**
 * Routes `count` payments from buckets `[from, to)`, expecting `gateway`, then reports `status` for
 * each. Routing comes first so a stop the outcomes trigger can't change where the rest are routed.
 */
async function resolve(api: ApiClient, merchantId: string, count: number, [from, to]: [number, number], gateway: string, status: string) {
  const ids = Array.from({ length: count }, () => paymentIn(from, to))
  for (const id of ids) expect(await route(api, merchantId, id)).toBe(gateway)
  for (const id of ids) {
    await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId, paymentId: id, gateway, status }))
  }
}

async function rule(api: ApiClient, merchantId: string, gateway: string): Promise<string> {
  const created = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchantId, { gateway }))
  expect(created.status).toBe(200)
  return created.body.rule_id
}

/** Creates and activates the experiment behind a baseline rule routing to stripe. */
async function startExperiment(api: ApiClient, merchantId: string, name: string, data: Record<string, unknown>) {
  await api.activateRoutingAlgorithm(merchantId, await rule(api, merchantId, 'stripe'))
  const experiment = await api.createRoutingAlgorithm({
    name, description: 'A/B lifecycle', created_by: merchantId, algorithm_for: 'payment', metadata: {},
    // /decide applies an experiment's `hybrid_routing` split.
    algorithm: { type: 'ab_test', data: { endpoints: ['hybrid_routing'], guardrail_threshold_pp: 3, ...data } },
  })
  expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
  const id = experiment.body.rule_id as string
  await api.setMerchantFeature(merchantId, 'ab-test-real-payments', true)
  await api.activateRoutingAlgorithm(merchantId, id)
  // Wait for a real variant decision.
  await expect.poll(() => route(api, merchantId, paymentIn(0, 20)), { timeout: 20_000, intervals: [500] }).not.toBe('stripe')
  return id
}

async function experimentActive(api: ApiClient, merchantId: string, experimentId: string): Promise<boolean> {
  const active = await api.listActiveRoutingAlgorithms(merchantId)
  return active.body.some((algorithm: { id: string }) => algorithm.id === experimentId)
}

/** Opens the experiment's page, reloading until `text` shows: analytics land a moment after the API. */
async function openShowing(page: Page, experimentId: string, text: string) {
  await expect(async () => {
    await page.goto(`/routing/ab-testing?experiment=${experimentId}`)
    await expect(page.getByText(text).first()).toBeVisible({ timeout: 5_000 })
  }).toPass({ timeout: 60_000, intervals: [2_000] })
}

test.describe('A/B experiment lifecycle', () => {
  test('a variant whose drop is clear breaches the guardrail and stops the experiment', async ({ api, merchant, authedPage }) => {
    test.setTimeout(120_000)
    const m = merchant.id
    const experimentId = await startExperiment(api, m, 'Guardrail breach', {
      control: { rule_algorithm_id: await rule(api, m, 'checkout') },
      variant: { rule_algorithm_id: await rule(api, m, 'adyen') },
      variant_split_pct: 20,
      min_sample_size: 1000,
    })
    // Every control payment succeeds and every variant payment fails; 40 each is past the 30 the
    // guardrail needs.
    await resolve(api, m, 40, [20, 100], 'checkout', 'AUTHORIZED')
    await resolve(api, m, 40, [0, 20], 'adyen', 'FAILURE')
    await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000 }).toBe(false)

    await openShowing(authedPage, experimentId, 'beyond the 3pp guardrail')
    await expect(authedPage.getByText('Inactive', { exact: true }).first()).toBeVisible()
    await expect(authedPage.getByText('This experiment is inactive. Payments use your active routing setup.')).toBeVisible()
    expect(await route(api, m, paymentIn(0, 20))).toBe('stripe')
  })

  test('an experiment completes once its outcomes reach the sample target', async ({ api, merchant, authedPage }) => {
    test.setTimeout(120_000)
    const m = merchant.id
    const experimentId = await startExperiment(api, m, 'Sample target', {
      control: { rule_algorithm_id: await rule(api, m, 'checkout') },
      variant: { rule_algorithm_id: await rule(api, m, 'adyen') },
      variant_split_pct: 20,
      min_sample_size: 6,
    })
    // Every payment succeeds, so only the target can end the experiment: its sixth outcome does.
    await resolve(api, m, 4, [20, 100], 'checkout', 'AUTHORIZED')
    await resolve(api, m, 2, [0, 20], 'adyen', 'AUTHORIZED')
    await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000 }).toBe(false)

    await openShowing(authedPage, experimentId, 'Completed with 6 outcomes.')
    await expect(authedPage.getByText('Inactive', { exact: true }).first()).toBeVisible()
    expect(await route(api, m, paymentIn(0, 20))).toBe('stripe')
    expect(await route(api, m, paymentIn(20, 100))).toBe('stripe')
  })

  test('an experiment whose outcomes stop arriving completes without a verdict', async ({ api, merchant, authedPage }) => {
    test.setTimeout(120_000)
    const m = merchant.id
    const experimentId = await startExperiment(api, m, 'Missing outcomes', {
      control: { rule_algorithm_id: await rule(api, m, 'checkout') },
      variant: { rule_algorithm_id: await rule(api, m, 'adyen') },
      variant_split_pct: 20,
      min_sample_size: 5,
    })
    // Route twice the target of 5; only one payment's outcome arrives.
    const controls = Array.from({ length: 8 }, () => paymentIn(20, 100))
    const variants = Array.from({ length: 2 }, () => paymentIn(0, 20))
    for (const id of controls) expect(await route(api, m, id)).toBe('checkout')
    for (const id of variants) expect(await route(api, m, id)).toBe('adyen')
    await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId: m, paymentId: controls[0], gateway: 'checkout', status: 'AUTHORIZED' }))
    await expect.poll(() => experimentActive(api, m, experimentId), { timeout: 15_000 }).toBe(false)

    await openShowing(authedPage, experimentId, 'so there is no verdict')
    await expect(authedPage.getByText('of the 5 outcomes needed arrived')).toBeVisible()
    await expect(authedPage.getByText('Inactive', { exact: true }).first()).toBeVisible()
  })

  test('with several variants, only the one that breaches stops', async ({ api, merchant, authedPage }) => {
    test.setTimeout(120_000)
    const m = merchant.id
    // Buckets 0..20 and 20..40; control keeps 40..100.
    const experimentId = await startExperiment(api, m, 'Several variants', {
      control: { rule_algorithm_id: await rule(api, m, 'checkout') },
      variants: [
        { id: 'failing', name: 'Failing', split_pct: 20, rule_algorithm_id: await rule(api, m, 'adyen') },
        { id: 'healthy', name: 'Healthy', split_pct: 20, rule_algorithm_id: await rule(api, m, 'razorpay') },
      ],
      min_sample_size: 1000,
    })
    await resolve(api, m, 40, [40, 100], 'checkout', 'AUTHORIZED')
    await resolve(api, m, 40, [20, 40], 'razorpay', 'AUTHORIZED')
    await resolve(api, m, 40, [0, 20], 'adyen', 'FAILURE')

    await openShowing(authedPage, experimentId, 'Failing is stopped')
    await expect(authedPage.getByText('Active', { exact: true }).first()).toBeVisible()
    expect(await experimentActive(api, m, experimentId)).toBe(true)
    // The stopped variant's payments use the active rule; the other variant keeps its own.
    expect(await route(api, m, paymentIn(0, 20))).toBe('stripe')
    expect(await route(api, m, paymentIn(20, 40))).toBe('razorpay')
  })
})
