import { test, expect } from '../../fixtures/test'
import { seedRoutedTraffic, waitForOverviewRouteHits } from '../../helpers/seed'

/**
 * Port of cypress/e2e/ui/dashboard-overview.cy.js — the Overview page at `/`.
 *
 * The seed + poll dance is not incidental: the page renders analytics that only exist once ClickHouse
 * has ingested the generated traffic, so the test waits for the data to land before asserting on it
 * rather than racing the pipeline.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Dashboard Overview UI', () => {
  test('renders overview content and shows the refresh state on range change', async ({
    api,
    authedPage,
    merchant,
  }) => {
    // Seeding + two ClickHouse ingestion waits can outlast the 60s default.
    test.setTimeout(120_000)

    await seedRoutedTraffic(api, merchant.id, {
      withAdvancedRule: false,
      withPreviewEvaluation: false,
      gatewayLatency: 2200,
      prefix: 'overview',
    })
    await waitForOverviewRouteHits(api, ['/decide_gateway'])

    await authedPage.goto('/')

    await expect(authedPage.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
    await expect(authedPage.getByText('Setup', { exact: true }).first()).toBeVisible()
    await expect(authedPage.getByText('Gateway activity').first()).toBeVisible()
    await expect(authedPage.getByText(merchant.id).first()).toBeVisible()

    // Wait for the first analytics load to settle before triggering a refresh.
    await expect(authedPage.getByText('Top gateway').first()).toBeVisible()

    // Changing the range must re-query analytics rather than re-render stale numbers. The Cypress
    // original asserted a "Loading" text badge; the refresh affordance is now a 2px progress bar and a
    // dimmed KPI grid, so assert the refetch itself — that's the behaviour, the badge was the cosmetic.
    const refetch = authedPage.waitForResponse(
      (r) => r.url().includes('/analytics/overview') && r.url().includes('range='),
      { timeout: 30_000 },
    )
    await authedPage.getByRole('button', { name: '1 week' }).click()
    await refetch

    await expect(authedPage.getByText('Top gateway').first()).toBeVisible({ timeout: 30_000 })

    await authedPage.getByRole('button', { name: 'Analytics' }).click()
    await expect(authedPage).toHaveURL(/\/analytics/)
    await expect(authedPage.getByRole('heading', { level: 1, name: 'Analytics' })).toBeVisible()
  })
})
