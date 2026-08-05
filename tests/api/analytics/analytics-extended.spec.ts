import { test, expect } from '../../fixtures/test'
import { seedRoutedTraffic } from '../../helpers/seed'

/**
 * The five analytics endpoints analytics.spec.ts does not reach. Each one backs a dashboard panel that
 * renders on page load, so the contract that matters is: authenticated, scoped to the session's
 * merchant, and returning a well-formed (possibly empty) payload rather than an error.
 *
 * As with analytics.spec.ts, these assert SHAPE not VALUES — ClickHouse ingestion is asynchronous and
 * pinning row counts here would buy flake without buying coverage.
 */
test.describe('Analytics endpoints (API)', () => {
  test('every analytics panel endpoint answers for a merchant with traffic', async ({ api, merchant }) => {
    await seedRoutedTraffic(api, merchant.id, { prefix: 'analytics_ext' })

    const endpoints = [
      '/analytics/gateway-scores',
      '/analytics/decisions',
      '/analytics/routing-stats',
      '/analytics/cost-savings',
      '/analytics/routing-events',
      '/analytics/log-summaries',
    ]

    for (const path of endpoints) {
      const r = await api.raw('GET', path, { failOnStatusCode: false, qs: { range: '1h' } })
      expect(r.status, `${path} should answer 200`).toBe(200)
      expect(r.body, `${path} should return a payload`).toBeTruthy()
    }
  })



})
