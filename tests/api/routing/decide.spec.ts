import { test, expect, factory } from '../../fixtures/test'
import contract from '../../fixtures/contracts/decide-request.json'
import type { DecideRequest, DecideResponse } from '../../../website/src/types/api'

function decideRequest(merchantId: string): DecideRequest {
  return {
    ...structuredClone(contract),
    payment_id: factory.paymentId('decide'),
    merchant_id: merchantId,
  }
}

function legacyRequest(request: DecideRequest) {
  return {
    static_routing_request: {
      created_by: request.merchant_id,
      payment_id: request.payment_id,
      algorithm_for: request.routing_config.algorithm_for,
      fallback_output: request.routing_config.eligible_connectors.map(c => ({
        gateway_name: c.gateway_name,
        gateway_id: c.payment_merchant_connector_id,
      })),
      parameters: {
        amount: { type: 'number', value: request.order_amount },
        payment_amount: { type: 'number', value: request.order_amount },
        currency: { type: 'enum_variant', value: 'USD' },
        payment_method: { type: 'enum_variant', value: 'card' },
        payment_method_type: { type: 'enum_variant', value: 'credit' },
        card_bin: { type: 'str_value', value: '001234' },
        extended_card_bin: { type: 'str_value', value: '00123456' },
        card_network: { type: 'enum_variant', value: 'Visa' },
        card_type: { type: 'enum_variant', value: 'credit' },
        billing_country: { type: 'enum_variant', value: 'UnitedStatesOfAmerica' },
        issuer_country: { type: 'enum_variant', value: 'UnitedStatesOfAmerica' },
        segment: { type: 'metadata_variant', value: { key: 'segment', value: 'premium' } },
      },
    },
  }
}

test.describe('Decide contract (API)', () => {
  test('accepts the full payload and preserves the no-rule fallback', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    const requestId = factory.paymentId('request')
    const response = await api.raw<DecideResponse>('POST', '/decide', {
      body, headers: { 'x-request-id': requestId },
    })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expect(response.status).toBe(200)
    expect(response.body.evaluated_connectors).toEqual(hybrid.body.evaluated_connectors.map((c: any) => ({
      gateway_name: c.gateway_name, payment_merchant_connector_id: c.gateway_id,
    })))
    expect(response.body.decision).toEqual({ outcome: 'fallback', source: 'no_active_algorithm', decision_id: requestId })
    expect(response.headers['x-request-id']).toBe(requestId)
    expect(Object.keys(response.body).sort()).toEqual(['decision', 'evaluated_connectors'])
  })

  test('matches existing rules using renamed fields and nested card data', async ({ api, merchant }) => {
    const selected = factory.gatewayConnector('checkout', 'mca_decide_checkout')
    const rules = [{
      name: 'Renamed fields', routing_type: 'priority', output: { priority: [selected] },
      statements: [{ condition: [
        { lhs: 'payment_method', comparison: 'equal', value: { type: 'enum_variant', value: 'card' } },
        { lhs: 'payment_method_type', comparison: 'equal', value: { type: 'enum_variant', value: 'credit' } },
        { lhs: 'payment_amount', comparison: 'equal', value: { type: 'number', value: 1000 } },
        { lhs: 'card_bin', comparison: 'equal', value: { type: 'str_value', value: '001234' } },
        { lhs: 'billing_country', comparison: 'equal', value: { type: 'enum_variant', value: 'UnitedStatesOfAmerica' } },
        { lhs: 'metadata', comparison: 'equal', value: { type: 'metadata_variant', value: { key: 'segment', value: 'premium' } } },
      ].map(c => ({ ...c, metadata: {} })) }],
    }]
    const created = await api.createRoutingAlgorithm(factory.advancedRoutingPayload(merchant.id, { rules }))
    await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors.push({
      gateway_name: selected.gateway_name, payment_merchant_connector_id: selected.gateway_id!,
    })
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expect(response.body.evaluated_connectors).toEqual([
      { gateway_name: selected.gateway_name, payment_merchant_connector_id: selected.gateway_id },
    ])
    expect(hybrid.body.evaluated_connectors).toEqual([selected])
    expect(response.body.decision.outcome).toBe('routed')
  })

  for (const enabled of [false, true]) {
    test(`preserves the hybrid SR gate when enabled=${enabled}`, async ({ api, merchant }) => {
      await api.createSuccessRateConfig(merchant.id)
      const connector = factory.gatewayConnector('stripe')
      const created = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id, {
        gateway: connector.gateway_name, gateway_id: connector.gateway_id,
      }))
      await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
      await api.setMerchantFeature(merchant.id, 'sr-routing', enabled)
      const body = decideRequest(merchant.id)
      body.routing_config.eligible_connectors = [{
        gateway_name: connector.gateway_name, payment_merchant_connector_id: connector.gateway_id!,
      }]
      const response = await api.raw<DecideResponse>('POST', '/decide', { body })
      const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
      expect(response.body.evaluated_connectors).toEqual(hybrid.body.evaluated_connectors.map((c: any) => ({
        gateway_name: c.gateway_name, payment_merchant_connector_id: c.gateway_id,
      })))
      expect(response.body.decision.outcome).toBe('routed')
      if (enabled) {
        expect(hybrid.body.dynamic_routing?.status).toBe('success')
        expect(response.body.decision.source).toBe(hybrid.body.dynamic_routing.decision.routing_approach)
      } else {
        expect(hybrid.body.dynamic_routing).toBeUndefined()
        expect(response.body.decision.source).toBe(hybrid.body.static_routing.output.type)
      }
    })
  }

  test('rejects removed fields, legacy connector IDs and missing required data', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    for (const invalid of [
      { ...body, payment_type: 'normal' },
      { ...body, context: {} },
      { ...body, order_amount: -1 },
      { ...body, payment_id: undefined },
      { ...body, merchant_id: undefined },
      { ...body, routing_config: { ...body.routing_config, created_by: merchant.id } },
      { ...body, routing_config: { ...body.routing_config, algorithm_for: undefined } },
      ...['payment_amount', 'udfs', 'payment_metadata', 'payment_source', 'auth_type', 'channel'].map(key => ({ ...body, [key]: null })),
      { ...body, routing_config: { ...body.routing_config, eligible_connectors: [{ gateway_name: 'stripe', gateway_id: 'mca_1' }] } },
    ]) {
      const response = await api.raw('POST', '/decide', { body: invalid, failOnStatusCode: false })
      expect(response.status).toBe(400)
    }
  })

  test('preserves the candidate fallback when static validation fails', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    body.payment_method_sub_type = 'unrecognized_subtype'
    const legacy = legacyRequest(body)
    legacy.static_routing_request.parameters.payment_method_type.value = 'unrecognized_subtype'
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacy })
    expect(hybrid.body.static_routing).toBeUndefined()
    expect(response.body.evaluated_connectors).toEqual(hybrid.body.evaluated_connectors.map((c: any) => ({
      gateway_name: c.gateway_name, payment_merchant_connector_id: c.gateway_id,
    })))
    expect(response.body.decision.outcome).toBe('fallback')
    expect(response.body.decision.source).toBe('eligible_connectors')
  })

  test('requires the same authentication as hybrid', async ({ api }) => {
    const response = await api.anonymous().raw('POST', '/decide', {
      body: decideRequest('pro_unauthenticated'), failOnStatusCode: false,
    })
    expect(response.status).toBe(401)
  })

  test('enforces merchant ownership for sessions and API keys', async ({ api, merchant }) => {
    const foreign = decideRequest(factory.merchantId('foreign'))
    const sessionResponse = await api.raw('POST', '/decide', { body: foreign, failOnStatusCode: false })
    expect(sessionResponse.status).toBe(403)
    const key = (await api.createApiKey(merchant.id, 'decide-scope')).body.api_key
    const anonymous = api.anonymous()
    for (const [body, status] of [[decideRequest(merchant.id), 200], [foreign, 403]] as const) {
      const response = await anonymous.raw('POST', '/decide', {
        body, headers: { 'x-api-key': key }, failOnStatusCode: false,
      })
      expect(response.status).toBe(status)
    }
    const serviceResponse = await api.asService().raw('POST', '/decide', { body: foreign })
    expect(serviceResponse.status).toBe(200)
  })

  test('preserves the empty result when no rule and no candidate exist', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = []
    const response = await api.raw('POST', '/decide', { body, failOnStatusCode: false })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body), failOnStatusCode: false })
    expect(response.status).toBe(hybrid.status)
    expect(response.status).toBe(200)
    expect(response.body.evaluated_connectors).toEqual(hybrid.body.evaluated_connectors)
    expect(response.body.evaluated_connectors).toEqual([])
    expect(response.body.decision.outcome).toBe('no_eligible_connector')
  })
})
