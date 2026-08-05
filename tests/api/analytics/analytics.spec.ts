import { test, expect } from '../../fixtures/test'
import { seedRoutedTraffic } from '../../helpers/seed'
import {
  expectValidAnalyticsOverview,
  expectValidRoutingStats,
  expectValidPaymentAudit,
} from '../../helpers/assertions'

/**
 * API-contract port of cypress/e2e/api/analytics-api.cy.js.
 *
 * The `merchant` fixture stands in for the Cypress `ensureMerchantAccount` beforeEach (fresh merchant
 * + dashboard session, auto-cleaned). The analytics endpoints derive the merchant from the session
 * bearer token that the fixture sets on `api`, so merchant_id/scope are never sent as query params —
 * matching commands.js `normalizeAnalyticsRequest`, which strips them.
 *
 * The source spec polls each analytics endpoint until ClickHouse ingestion populates specific rows.
 * Analytics data may be empty in a fresh run, so per the port contract we generate the traffic and
 * then assert response SHAPE/status (the expectValid* contracts), not specific data values.
 *
 * The seed sequence itself lives in tests/helpers/seed.ts — the three analytics UI specs need the
 * identical setup. The remaining analytics endpoints are covered in analytics-extended.spec.ts.
 */
test.describe('Analytics API', () => {
  test('returns populated overview, routing stats, payment audit, and preview trace after traffic is generated', async ({
    api,
    merchant,
  }) => {
    const seeded = await seedRoutedTraffic(api, merchant.id, {
      scoreStatus: 'AUTHORIZED',
      gatewayLatency: 2500,
      prefix: 'analytics',
    })

    // The card/250 preview must resolve through the advanced rule to a priority output.
    expect(seeded.previewEvaluation.output.type).toBe('priority')

    // Analytics overview — merchant_id/scope are derived from the session token, not the query.
    const overview = await api.raw('GET', '/analytics/overview', {
      qs: { range: '1h' },
      failOnStatusCode: false,
    })
    expect(overview.status).toBe(200)
    expectValidAnalyticsOverview(overview.body)

    // Routing stats.
    const routingStats = await api.raw('GET', '/analytics/routing-stats', {
      qs: { range: '1h' },
      failOnStatusCode: false,
    })
    expect(routingStats.status).toBe(200)
    expectValidRoutingStats(routingStats.body)

    // Payment audit for the decisioned payment.
    const paymentAudit = await api.raw('GET', '/analytics/payment-audit', {
      qs: { range: '1h', payment_id: seeded.decisionPaymentId },
      failOnStatusCode: false,
    })
    expect(paymentAudit.status).toBe(200)
    expectValidPaymentAudit(paymentAudit.body)

    // Preview trace for the evaluated (preview) payment — same audit shape.
    const previewTrace = await api.raw('GET', '/analytics/preview-trace', {
      qs: { range: '1h', payment_id: seeded.previewPaymentId },
      failOnStatusCode: false,
    })
    expect(previewTrace.status).toBe(200)
    expectValidPaymentAudit(previewTrace.body)
  })
})
