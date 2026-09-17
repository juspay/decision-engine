import { test, expect } from '../../fixtures/test'
import {
  seedHybridTraffic,
  seedRoutedTraffic,
  waitForHybridDecisions,
  waitForOverviewRouteHits,
} from '../../helpers/seed'

/**
 * Port of cypress/e2e/ui/analytics-page.cy.js.
 *
 * The Analytics page has independent views — hybrid, multi-objective
 * (success-rate routing), and rule/volume based — and the toggle between them is the thing worth
 * guarding: each reads a different set of endpoints (or the same ones over a different flow type),
 * so a regression in one is invisible while looking at the other.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Analytics UI', () => {
  test('renders transaction and rule-based analytics with refresh', async ({
    api,
    authedPage,
    merchant,
  }) => {
    test.setTimeout(120_000)

    await seedRoutedTraffic(api, merchant.id, { prefix: 'analytics_ui' })
    await waitForOverviewRouteHits(api, ['/decide_gateway', '/update_gateway'])

    await authedPage.goto('/analytics')

    await expect(authedPage.getByRole('heading', { level: 1, name: 'Analytics' })).toBeVisible()
    await expect(authedPage.getByRole('button', { name: 'Multi-objective', exact: true })).toBeVisible()
    await expect(
      authedPage.getByRole('button', { name: 'Rule based / Volume based', exact: true }),
    ).toBeVisible()

    await authedPage.getByRole('button', { name: 'Multi-objective', exact: true }).click({ force: true })
    await expect(authedPage).toHaveURL(/view=multi_objective/)

    // Change the window and force a reload of both panels.
    await authedPage.getByRole('button', { name: '1w', exact: true }).click()
    const overview = authedPage.waitForResponse((r) => r.url().includes('/analytics/overview'))
    const routingStats = authedPage.waitForResponse((r) => r.url().includes('/analytics/routing-stats'))
    await authedPage.getByRole('button', { name: 'Refresh' }).click()
    await overview
    await routingStats

    await expect(authedPage.getByText('Decide Gateway')).toBeVisible({ timeout: 30_000 })

    // Switching views must swap in the rule-based panel.
    await authedPage
      .getByRole('button', { name: 'Rule based / Volume based', exact: true })
      .click({ force: true })
    await expect(authedPage.getByText('Latest decisions from')).toBeVisible({ timeout: 30_000 })
  })

  /**
   * The hybrid tab reads the same metrics as multi-objective, only over `routing_hybrid_*`
   * events — so the regression it guards is the query never switching flow type, which looks
   * like an empty tab rather than an error.
   */
  test('hybrid tab reports hybrid decisions the multi-objective tab does not count', async ({
    api,
    authedPage,
    merchant,
  }) => {
    test.setTimeout(120_000)

    await seedHybridTraffic(api, merchant.id, { prefix: 'analytics_hybrid_ui' })
    await waitForHybridDecisions(api)


    const hybridOverview = authedPage.waitForResponse(
      (r) => r.url().includes('/analytics/overview') && r.url().includes('routing_kind=hybrid'),
    )
    await authedPage.goto('/analytics')
    await hybridOverview

    // The rule-based half of the call, which the multi-objective tab has no card for.
    await expect(authedPage.getByText('Static vs dynamic outcome')).toBeVisible({ timeout: 30_000 })
    await expect(authedPage.getByText('Rule-shortlisted connectors')).toBeVisible()
    // The decision card counts hybrid calls rather than /decide_gateway ones.
    await expect(authedPage.getByText('Decide Gateway')).toHaveCount(0)
    await expect(authedPage).not.toHaveURL(/view=/)
    await authedPage.getByRole('button', { name: 'Multi-objective', exact: true }).click({ force: true })
    await expect(authedPage).toHaveURL(/view=multi_objective/)
  })
})
