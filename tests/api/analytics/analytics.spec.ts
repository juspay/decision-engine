import { test, expect, factory } from '../../fixtures/test'
import {
  HYBRID_AUDIT_TRAIL,
  seedHybridTraffic,
  seedRoutedTraffic,
  waitForAuditFlowType,
  waitForAuditFlowTypes,
  waitForAuditListing,
} from '../../helpers/seed'
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

    // The unified audit (default scope) shows the direct /routing/evaluate decision too, so an
    // operator no longer has to know which trail a payment lives in.
    const unified = await waitForAuditFlowType(api, seeded.previewPaymentId!, 'routing_evaluate_advanced')
    expect(unified.body.scope).toBe('all')

    // A payment id that carries both a rule evaluation and an SR decision is listed under
    // Multi-objective only, with its whole trail; the preview-only payment is rule based only.
    await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(
        merchant.id,
        { payment_method: { type: 'enum_variant', value: 'card' }, amount: { type: 'number', value: 250 } },
        { payment_id: seeded.decisionPaymentId },
      ),
    )
    await waitForAuditFlowTypes(api, seeded.decisionPaymentId, [
      'decide_gateway_decision',
      'update_gateway_score_update',
      'routing_evaluate_advanced',
    ])
    const multi = await waitForAuditListing(api, seeded.decisionPaymentId, { routing_kind: 'multi_objective' })
    expect(multi.body.results.find((row: any) => row.payment_id === seeded.decisionPaymentId).event_count).toBe(3)
    expect(multi.body.results.some((row: any) => row.payment_id === seeded.previewPaymentId)).toBe(false)
    const ruleBased = await waitForAuditListing(api, seeded.previewPaymentId!, { routing_kind: 'rule_based' })
    expect(ruleBased.body.results.some((row: any) => row.payment_id === seeded.decisionPaymentId)).toBe(false)
  })

  test('records a hybrid payment as one hybrid event plus its score update', async ({
    api,
    merchant,
  }) => {
    test.setTimeout(120_000)

    const seeded = await seedHybridTraffic(api, merchant.id, { prefix: 'analytics_hybrid' })

    // Exact lookup: one entry, two events, in trail order.
    const audit = await waitForAuditFlowTypes(api, seeded.paymentId, HYBRID_AUDIT_TRAIL)
    expect(audit.status).toBe(200)
    expectValidPaymentAudit(audit.body)
    expect(audit.body.scope).toBe('all')
    expect(audit.body.results).toHaveLength(1)
    expect(audit.body.results[0].payment_id).toBe(seeded.paymentId)
    expect(audit.body.results[0].event_count).toBe(2)

    const flowTypes: string[] = audit.body.timeline.map((event: any) => event.flow_type)
    expect(flowTypes.indexOf('routing_hybrid_decision')).toBeGreaterThanOrEqual(0)
    expect(flowTypes.indexOf('routing_hybrid_decision')).toBeLessThan(flowTypes.indexOf('update_gateway_score_update'))
    // Neither half is recorded on its own.
    expect(flowTypes.some((flowType) => flowType.startsWith('routing_evaluate_'))).toBe(false)
    expect(flowTypes.some((flowType) => flowType.startsWith('decide_gateway_'))).toBe(false)

    // The hybrid event carries the whole hybrid request and response.
    const byType = Object.fromEntries(audit.body.timeline.map((event: any) => [event.flow_type, event]))
    const hybridEvent = byType.routing_hybrid_decision
    expect(hybridEvent.route).toBe('routing_hybrid')
    expect(hybridEvent.event_stage).toBe('hybrid_routed')
    expect(hybridEvent.gateway).toBe(seeded.decidedGateway)
    expect(hybridEvent.request_id).toBeTruthy()
    expect(hybridEvent.details_json?.request?.static_routing_request?.created_by).toBe(merchant.id)
    expect(hybridEvent.details_json?.request?.dynamic_routing_request?.merchantId).toBe(merchant.id)
    expect(hybridEvent.details_json?.response?.static_routing?.status).toBe('success')
    expect(hybridEvent.details_json?.response?.dynamic_routing?.decision?.decided_gateway).toBe(seeded.decidedGateway)
    expect(hybridEvent.details_json?.selection_reason?.dynamic_status).toBe('success')
    expect(byType.update_gateway_score_update.event_stage).toBe('score_updated')

    // Routing-type filter over the list: hybrid lists it, the other kinds do not.
    const hybridListing = await waitForAuditListing(api, seeded.paymentId, { routing_kind: 'hybrid' })
    expect(hybridListing.body.routing_kind).toBe('hybrid')
    const row = hybridListing.body.results.find((entry: any) => entry.payment_id === seeded.paymentId)
    expect(row.event_count).toBe(2)
    for (const routingKind of ['multi_objective', 'rule_based', 'debit_routing']) {
      const listing = await api.raw('GET', '/analytics/payment-audit', {
        qs: { range: '1h', routing_kind: routingKind },
        failOnStatusCode: false,
      })
      expect(listing.status).toBe(200)
      expect(listing.body.results.some((entry: any) => entry.payment_id === seeded.paymentId)).toBe(false)
    }

    // The preview trail stays reserved for direct /routing/evaluate calls.
    const preview = await api.raw('GET', '/analytics/preview-trace', {
      qs: { range: '1h', payment_id: seeded.paymentId },
      failOnStatusCode: false,
    })
    expect(preview.status).toBe(200)
    expect(preview.body.results).toHaveLength(0)
    expect(preview.body.timeline).toHaveLength(0)
    const previewScope = await api.raw('GET', '/analytics/payment-audit', {
      qs: { range: '1h', payment_id: seeded.paymentId, scope: 'preview' },
      failOnStatusCode: false,
    })
    expect(previewScope.body.scope).toBe('preview')
    expect(previewScope.body.results).toHaveLength(0)
  })
})
