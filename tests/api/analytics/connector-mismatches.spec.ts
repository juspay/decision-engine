import { test, expect, factory, poll } from '../../fixtures/test'

test('audit filters connector mismatches across both routing paths and retains complete trails', async ({ api, merchant }) => {
  test.setTimeout(120_000)
  await api.createSuccessRateConfig(merchant.id)
  await api.setMerchantFeature(merchant.id, 'sr-routing', true)
  const rule = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, { gateway: 'adyen' }))
  await api.activateRoutingAlgorithm(merchant.id, rule.body.rule_id)

  const paymentIds: string[] = []
  for (const route of ['decide', 'hybrid']) {
    const paymentId = factory.paymentId(`mismatch_${route}`)
    paymentIds.push(paymentId)
    const dynamic = factory.srDecideGatewayRequest({ merchantId: merchant.id, eligibleGatewayList: ['adyen'], paymentInfo: { paymentId } })
    const response = route === 'decide'
      ? await api.decideGateway(dynamic)
      : await api.raw('POST', '/routing/hybrid', { body: {
        static_routing_request: {
          created_by: merchant.id, payment_id: paymentId,
          parameters: { payment_method: { type: 'enum_variant', value: 'card' } },
          fallback_output: [factory.gatewayConnector('adyen')],
        },
        dynamic_routing_request: dynamic,
      } })
    const decided = route === 'decide' ? response.body.decided_gateway : response.body.dynamic_routing.decision.decided_gateway
    expect(decided.split(':')[0]).toBe('adyen')
    // Keep both the matching connector and every different connector, without duplicates.
    for (const gateway of ['adyen', 'stripe', 'cybersource', 'stripe']) {
      await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId: merchant.id, paymentId, gateway }))
    }
  }

  const matchingId = factory.paymentId('matching')
  await api.decideGateway(factory.srDecideGatewayRequest({ merchantId: merchant.id, eligibleGatewayList: ['adyen'], paymentInfo: { paymentId: matchingId } }))
  await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId: merchant.id, paymentId: matchingId, gateway: 'adyen' }))
  await api.updateGatewayScore(factory.updateGatewayScoreRequest({ merchantId: merchant.id, paymentId: factory.paymentId('no_decision'), gateway: 'stripe' }))

  const audit = await poll(
    () => api.raw('GET', '/analytics/payment-audit', { qs: { range: '1h', connector_mismatch: true } }),
    (response) => response.body.results?.length === 2 && response.body.results.every((row: any) => row.event_count === 5),
    { message: 'Both routing families should match without dropping any update events', timeout: 60_000 },
  )
  expect(audit.body.total_results).toBe(2)
  expect(audit.body.results.map((row: any) => row.payment_id).sort()).toEqual(paymentIds.sort())
  for (const row of audit.body.results) {
    const providers = row.gateways.map((gateway: string) => gateway.split(':')[0].toLowerCase())
    expect(providers).toEqual(expect.arrayContaining(['adyen', 'cybersource', 'stripe']))
  }
  expect(audit.body.timeline).toHaveLength(5)
  const timelineProviders = audit.body.timeline.map((event: any) => event.gateway.split(':')[0].toLowerCase())
  expect(timelineProviders).toEqual(expect.arrayContaining(['adyen', 'cybersource', 'stripe']))

  // Filter before pagination, retaining the full count even on an empty page.
  const qs = { range: '1h', connector_mismatch: true, page_size: 1 }
  const first = await api.raw('GET', '/analytics/payment-audit', { qs })
  const second = await api.raw('GET', '/analytics/payment-audit', { qs: { ...qs, page: 2 } })
  const empty = await api.raw('GET', '/analytics/payment-audit', { qs: { ...qs, page: 3 } })
  expect(first.body.total_results).toBe(2)
  expect(second.body.results).toHaveLength(1)
  expect(second.body.results[0].payment_id).not.toBe(first.body.results[0].payment_id)
  expect(empty.body.total_results).toBe(2)
  expect(empty.body.results).toEqual([])

  // Compose with the existing routing and payment-attribute filters (summary and raw paths).
  for (const filters of [{ routing_kind: 'hybrid' }, { currency: 'USD', routing_kind: 'hybrid' }]) {
    const filtered = await api.raw('GET', '/analytics/payment-audit', { qs: { ...qs, ...filters } })
    expect(filtered.body.total_results).toBe(1)
    expect(filtered.body.results[0].payment_id).toBe(paymentIds.find((id) => id.startsWith('mismatch_hybrid')))
    expect(filtered.body.results[0].event_count).toBe(5)
    expect(filtered.body.timeline).toHaveLength(5)
  }
  const unfiltered = await api.raw('GET', '/analytics/payment-audit', { qs: { range: '1h', connector_mismatch: false } })
  expect(unfiltered.body.results.some((row: any) => row.payment_id === matchingId)).toBe(true)
})
