import { test, expect } from '../../fixtures/test'
import { seedRoutedTraffic, waitForOverviewRouteHits } from '../../helpers/seed'

/**
 * Port of cypress/e2e/ui/analytics-page.cy.js.
 *
 * The Analytics page has two independent views — multi-objective (success-rate routing) and
 * rule/volume based — and the toggle between them is the thing worth guarding: each reads a different
 * set of endpoints, so a regression in one is invisible while looking at the other.
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
})
