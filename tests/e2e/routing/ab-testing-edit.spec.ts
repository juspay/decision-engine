import { test, expect, factory } from '../../fixtures/test'
import type { Page } from '@playwright/test'
import { expectApiCall } from '../../helpers/network'
import type { ApiClient } from '../../fixtures/api-client'

/**
 * Editing a saved A/B experiment. Until the experiment records a payment every setting can change;
 * after that its results are read against its setup, so only the name, sample target and guardrail
 * can. The page decides which by reading the experiment's results (across every endpoint).
 */
test.use({ viewport: { width: 1600, height: 1200 } })

async function createExperiment(api: ApiClient, merchantId: string) {
  const rule = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, { name: factory.ruleName('ab_edit_rule'), gateway: 'stripe' }),
  )
  expect(rule.status).toBe(200)
  const name = factory.ruleName('ab_edit_exp')
  const experiment = await api.createRoutingAlgorithm({
    name,
    description: 'A/B test: 10% variant traffic',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: {
        control: { rule_algorithm_id: rule.body.rule_id },
        variant: { sr: { enable_multi_objective: false, use_autopilot: true } },
        variant_split_pct: 10,
        min_sample_size: 1000,
        guardrail_threshold_pp: 3,
      },
    },
  })
  expect(experiment.status, JSON.stringify(experiment.body)).toBe(200)
  return { id: experiment.body.rule_id as string, name }
}

/** Answers the edit check (results with no endpoint filter) as if the experiment recorded payments. */
async function mockRecordedPayments(page: Page, count: number) {
  await page.route(
    (url) => url.pathname.endsWith('/results') && url.pathname.includes('/analytics/experiment/') && !url.searchParams.has('endpoint'),
    (route) => route.fulfill({
      json: { control: { transaction_count: count }, variant: { transaction_count: 0 } },
    }),
  )
}

async function openEdit(page: Page, experimentId: string) {
  await page.goto(`/routing/ab-testing?experiment=${experimentId}`)
  await page.getByRole('button', { name: 'Edit', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Edit experiment' })).toBeVisible()
}

test.describe('A/B experiment edit', () => {
  test('an experiment with no recorded payments can change its whole setup', async ({ authedPage, api, merchant }) => {
    const experiment = await createExperiment(api, merchant.id)
    await mockRecordedPayments(authedPage, 0)
    await openEdit(authedPage, experiment.id)

    await expect(authedPage.getByText('No payments recorded yet, so every setting can change.')).toBeVisible()
    await expect(authedPage.getByText('Traffic allocation')).toBeVisible()
    await expect(authedPage.getByLabel('Control traffic percentage')).toBeVisible()

    await authedPage.getByRole('slider', { name: 'Control traffic percentage' }).fill('80')
    const saved = expectApiCall(authedPage, '/routing/update')
    await authedPage.getByRole('button', { name: 'Save changes' }).click()
    const { status, requestBody, body } = await saved
    expect(status, JSON.stringify(body)).toBe(200)
    expect(requestBody.routing_algorithm_id).toBe(experiment.id)
    expect(requestBody.algorithm.data.variant_split_pct).toBe(20)
  })

  test('an experiment with recorded payments keeps its routing setup', async ({ authedPage, api, merchant }) => {
    const experiment = await createExperiment(api, merchant.id)
    await mockRecordedPayments(authedPage, 6)
    await openEdit(authedPage, experiment.id)

    await expect(authedPage.getByText(/its arms, traffic split and endpoints are locked/)).toBeVisible()
    await expect(authedPage.getByText('Traffic allocation')).toHaveCount(0)
    await expect(authedPage.getByLabel('Control traffic percentage')).toHaveCount(0)

    await authedPage.getByRole('button', { name: '5,000', exact: true }).click()
    const saved = expectApiCall(authedPage, '/routing/update')
    await authedPage.getByRole('button', { name: 'Save changes' }).click()
    const { status, requestBody, body } = await saved
    expect(status, JSON.stringify(body)).toBe(200)
    expect(requestBody.name).toBe(experiment.name)
    expect(requestBody.algorithm.data.min_sample_size).toBe(5000)
    expect(requestBody.algorithm.data.variant_split_pct).toBe(10)
    expect(requestBody.algorithm.data.control).toBeTruthy()
  })
})
