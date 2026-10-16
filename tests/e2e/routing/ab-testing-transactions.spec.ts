import { test, expect, factory } from '../../fixtures/test'
import type { Page } from '@playwright/test'
import type { ApiClient } from '../../fixtures/api-client'

/**
 * An experiment's results page: the transactions table and the counts around it.
 *
 * - The table runs its full height (a page is 20 rows), so it scrolls with the page instead of
 *   inside a short box of its own.
 * - The sample target counts resolved payments only, while the table lists every routed payment,
 *   so the page names both and the payments still awaiting an outcome that separate them.
 */
test.use({ viewport: { width: 1600, height: 900 } })

const ROUTED = 48
const CONTROL = { routed: 30, success: 18, failure: 4 }
const VARIANT = { routed: 18, success: 10, failure: 4 }
const RESOLVED = CONTROL.success + CONTROL.failure + VARIANT.success + VARIANT.failure
const AWAITING = ROUTED - RESOLVED

async function createExperiment(api: ApiClient, merchantId: string) {
  const rule = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, { name: factory.ruleName('ab_txn_rule'), gateway: 'stripe' }),
  )
  expect(rule.status).toBe(200)
  const experiment = await api.createRoutingAlgorithm({
    name: factory.ruleName('ab_txn_exp'),
    description: 'A/B test: transactions table',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: {
        control: { rule_algorithm_id: rule.body.rule_id },
        variant: { sr: { enable_multi_objective: false, use_autopilot: true } },
        variant_split_pct: 40,
        min_sample_size: 1000,
        guardrail_threshold_pp: 3,
      },
    },
  })
  expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
  return experiment.body.rule_id as string
}

function arm(name: string, counts: typeof CONTROL) {
  const resolved = counts.success + counts.failure
  return {
    arm: name,
    transaction_count: counts.routed,
    success_count: counts.success,
    failure_count: counts.failure,
    auth_rate: counts.success / resolved,
    first_attempt_auth_rate: counts.success / resolved,
    total_cost_saved: null,
    avg_latency_ms: null,
    avg_chosen_cost_bps: null,
    avg_cost_saved_bps: null,
    net_ev_bps: null,
  }
}

/** Answers the experiment's analytics with `ROUTED` payments, `RESOLVED` of them with an outcome. */
async function mockExperimentAnalytics(page: Page, experimentId: string) {
  await page.route(
    (url) => url.pathname.endsWith(`/analytics/experiment/${experimentId}/results`),
    (route) =>
      route.fulfill({
        json: {
          experiment_id: experimentId,
          merchant_id: 'm',
          control: arm('control', CONTROL),
          variant: arm('variant', VARIANT),
          delta_pp: 0,
          p_value: null,
          confidence_interval: null,
          verdict: 'collecting_data',
          min_sample_size: 1000,
          net_delta_bps: null,
          evaluation_margin: 0,
        },
      }),
  )
  await page.route(
    (url) => url.pathname.endsWith(`/analytics/experiment/${experimentId}/transactions`),
    (route) => {
      const page = Number(new URL(route.request().url()).searchParams.get('page') || '1')
      const pageSize = Number(new URL(route.request().url()).searchParams.get('page_size') || '20')
      const from = (page - 1) * pageSize
      const count = Math.max(0, Math.min(pageSize, ROUTED - from))
      return route.fulfill({
        json: {
          experiment_id: experimentId,
          total: ROUTED,
          transactions: Array.from({ length: count }, (_, i) => ({
            payment_id: `pay_txn_${from + i}`,
            variant_arm: (from + i) % 3 === 0 ? 'variant' : 'control',
            gateway: 'stripe',
            status: from + i < RESOLVED ? 'success' : 'ab_test_decision',
            created_at_ms: Date.now() - (from + i) * 1000,
          })),
        },
      })
    },
  )
}

test.describe('A/B experiment transactions', () => {
  test('the table runs its full height and the counts add up', async ({ authedPage, api, merchant }) => {
    const experimentId = await createExperiment(api, merchant.id)
    await mockExperimentAnalytics(authedPage, experimentId)
    await authedPage.goto(`/routing/ab-testing?experiment=${experimentId}`)

    const table = authedPage.getByRole('table').filter({ hasText: 'pay_txn_0' })
    await expect(table.getByRole('row')).toHaveCount(1 + 20)

    // No scroller of its own: the box holding the table is as tall as the table.
    const box = await table.evaluate((element) => {
      const parent = element.parentElement!
      return { scroll: parent.scrollHeight, client: parent.clientHeight }
    })
    expect(box.scroll).toBe(box.client)

    // The table lists every routed payment; the sample target counts the resolved ones.
    await expect(
      authedPage.getByText(`${ROUTED} routed · ${RESOLVED} resolved · ${AWAITING} awaiting an outcome`),
    ).toBeVisible()
    await expect(authedPage.getByText(`Page 1 of 3 · ${ROUTED} routed`)).toBeVisible()
    await expect(authedPage.getByText(`${RESOLVED} / 1,000`)).toBeVisible()
    await expect(
      authedPage.getByText(`of the 1,000 needed for a verdict · ${AWAITING} more awaiting an outcome`),
    ).toBeVisible()
  })
})
