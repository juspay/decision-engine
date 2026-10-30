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
        gateway_id: c.payment_merchant_connector_id ?? null,
      })),
      parameters: {
        amount: { type: 'number', value: request.amount },
        payment_amount: { type: 'number', value: request.amount },
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

const connector = (name: string) => {
  const { gateway_name, gateway_id } = factory.gatewayConnector(name)
  return { gateway_name, payment_merchant_connector_id: gateway_id! }
}

/** A hybrid `{gateway_name, gateway_id}` connector in the `/decide` shape. */
const fromLegacy = (c: any) => ({ gateway_name: c.gateway_name, payment_merchant_connector_id: c.gateway_id ?? null })

/** A decider `"name:id"` (or bare `"name"`) identifier in the `/decide` shape. */
const fromIdentifier = (identifier: string) => {
  const split = identifier.indexOf(':')
  return split < 0
    ? { gateway_name: identifier, payment_merchant_connector_id: null }
    : { gateway_name: identifier.slice(0, split), payment_merchant_connector_id: identifier.slice(split + 1) }
}

const identifierOf = (c: { gateway_name: string; payment_merchant_connector_id?: string | null }) =>
  c.payment_merchant_connector_id ? `${c.gateway_name}:${c.payment_merchant_connector_id}` : c.gateway_name

/** The identifiers a `/decide` answer names, as a set: fallbacks tied on score come back in either order. */
const candidates = (response: DecideResponse) => response.connectors.map(identifierOf).sort()

/** Renames the rule output's `gateway_id` keys, as `/decide` writes its `rule.output`. */
const renameIds = (value: any): any => Array.isArray(value)
  ? value.map(renameIds)
  : value && typeof value === 'object'
    ? Object.fromEntries(Object.entries(value).map(([key, v]) => [
      'gateway_name' in value && key === 'gateway_id' ? 'payment_merchant_connector_id' : key, renameIds(v),
    ]))
    : value

const RULE_STATUS: Record<string, string> = {
  success: 'matched', default_selection: 'default_selection', no_active_algorithm: 'no_active_rule',
}

/**
 * Routes payments through `/decide`, charging those that land on `winner` and failing the rest, so
 * `winner` scores highest. Fresh connectors all score the same and SR breaks the tie at random.
 */
async function trainScores(api: any, template: DecideRequest, winner: string) {
  for (let i = 0; i < 12; i++) {
    const body = { ...structuredClone(template), payment_id: factory.paymentId('train') }
    const decision: DecideResponse = (await api.raw('POST', '/decide', { body })).body
    if (decision.success_rate?.status !== 'applied') return
    const decided = identifierOf(decision.connectors[0])
    await api.raw('POST', '/update-gateway-score', { body: {
      merchantId: template.merchant_id, gateway: decided, gatewayReferenceId: null, paymentId: body.payment_id,
      status: decided === winner ? 'CHARGED' : 'FAILURE', enforceDynamicRoutingFailure: null,
    } })
  }
}

/**
 * The field map between the `/routing/hybrid` record and the `/decide` answer for the same
 * payment: every fact the record carries is where `/decide` puts it. SR configs here hedge 0% and
 * scores are trained to one winner, so the SR pick is deterministic.
 */
function expectFieldMapParity(decide: DecideResponse, hybrid: any) {
  const rule = hybrid.static_routing
  if (rule) {
    expect(decide.rule).toEqual({
      status: RULE_STATUS[rule.status],
      output: renameIds(rule.output),
      selected: rule.evaluated_output.map(fromLegacy),
      eligible: rule.eligible_connectors.map(fromLegacy),
    })
  } else {
    expect(decide.rule).toBeNull()
  }

  const sr = hybrid.dynamic_routing
  if (sr?.status === 'success') {
    const legacy = sr.decision
    expect(decide.connectors[0]).toEqual(fromIdentifier(legacy.decided_gateway))
    expect(candidates(decide)).toEqual([legacy.decided_gateway, ...legacy.fallback_gateways].sort())
    expect(decide.success_rate).toMatchObject({
      status: 'applied', approach: legacy.routing_approach, scheduled_outage: legacy.is_scheduled_outage,
    })
    // Scores are live: the call in between can move them slightly, so they compare per connector
    // within a tolerance.
    const scores = Object.fromEntries(decide.success_rate!.scores.map(s => [identifierOf(s), s.score]))
    const legacyScores: Record<string, number> = legacy.gateway_priority_map ?? {}
    expect(Object.keys(scores).sort()).toEqual(Object.keys(legacyScores).sort())
    for (const [identifier, score] of Object.entries(legacyScores)) {
      expect(Math.abs(scores[identifier] - score)).toBeLessThan(0.05)
    }
    const cost = legacy.multi_objective_info
    if (cost) {
      expect(decide.cost).toMatchObject({
        outcome: cost.outcome, cost_saved_bps: cost.costSavedBps,
        qualified_count: cost.qualifiedCount, margin: cost.margin,
      })
      // The reason quotes live auth rates, which can move between the two calls.
      const masked = (reason: string) => reason.replace(/[0-9.]+/g, '#')
      expect(masked(decide.cost!.reason)).toBe(masked(cost.reason))
      expect(decide.cost!.ranked.map(r => [identifierOf(r), r.cost_bps, r.is_sr_head, r.is_chosen]))
        .toEqual((cost.ranked ?? []).map((r: any) => [r.psp, r.costBps, r.isSrHead, r.isChosen]))
    } else {
      expect(decide.cost).toBeNull()
    }
    expect(Boolean(decide.volume_steering)).toBe(Boolean(legacy.volume_steer_info))
    expect(Boolean(decide.debit_routing)).toBe(Boolean(legacy.debit_routing_output))
  } else if (sr?.status === 'fallback') {
    expect(decide.success_rate?.status).toBe('failed')
    expect(decide.connectors).toEqual(sr.fallback_connectors.map(fromLegacy))
  } else {
    expect(decide.success_rate).toBeNull()
    expect(decide.connectors).toEqual((hybrid.evaluated_connectors ?? []).map(fromLegacy))
  }
}

test.describe('Decide parity with hybrid (API)', () => {
  const scenarios = [
    { name: 'rule only', rule: true, sr: false, cost: false },
    { name: 'SR only', rule: false, sr: true, cost: false },
    { name: 'rule + SR', rule: true, sr: true, cost: false },
    { name: 'rule + SR + cost', rule: true, sr: true, cost: true },
  ]
  for (const scenario of scenarios) {
    test(`carries every hybrid fact: ${scenario.name}`, async ({ api, merchant }) => {
      await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0 })
      if (scenario.rule) {
        const created = await api.createRoutingAlgorithm(factory.priorityRoutingPayload(merchant.id, {
          connectors: [factory.gatewayConnector('checkout'), factory.gatewayConnector('stripe')],
        }))
        await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
      }
      await api.setMerchantFeature(merchant.id, 'sr-routing', scenario.sr)
      await api.setMerchantFeature(merchant.id, 'cost-savings', scenario.cost)

      const body = decideRequest(merchant.id)
      // adyen is a candidate the rule leaves out, so a rule-narrowed SR decision never names it.
      body.routing_config.eligible_connectors = ['stripe', 'checkout', 'adyen'].map(connector)
      if (scenario.sr) await trainScores(api, body, 'stripe:mca_stripe')
      const response = await api.raw<DecideResponse>('POST', '/decide', { body })
      const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
      expect(response.status).toBe(hybrid.status)
      expectFieldMapParity(response.body, hybrid.body)
      expect(response.body.warnings).toEqual([])

      if (scenario.sr) {
        expect(response.body.outcome).toMatchObject({ type: 'routed', strategy: 'success_rate', reason: 'success_rate_ranked' })
        expect(identifierOf(response.body.connectors[0])).toBe('stripe:mca_stripe')
        const expected = scenario.rule ? ['checkout:mca_checkout', 'stripe:mca_stripe'] : ['adyen:mca_adyen', 'checkout:mca_checkout', 'stripe:mca_stripe']
        expect(candidates(response.body)).toEqual(expected)
        expect(response.body.rule?.status).toBe(scenario.rule ? 'matched' : 'no_active_rule')
      } else {
        expect(response.body.outcome).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_matched' })
        expect(response.body.connectors.map(c => c.gateway_name)).toEqual(['checkout', 'stripe'])
      }
    })
  }

  test('carries every hybrid fact when cost overrides the SR head', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0 })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    await api.setMerchantFeature(merchant.id, 'cost-savings', true)
    // Contract rates price PSPs by gateway name, so the candidates are named without an account.
    const rate = (psp: string, markup_bps: number) => ({
      psp, interchange_bps: 0, scheme_bps: 0, markup_bps, fixed: 0, is_default: true,
    })
    await api.raw('PUT', `/merchant-account/${merchant.id}/seed-costs`, {
      body: { rows: [rate('stripe', 1500), rate('adyen', 20)] },
    })
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [{ gateway_name: 'stripe' }, { gateway_name: 'adyen' }]
    await trainScores(api, body, 'stripe')
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.outcome).toMatchObject({
      type: 'routed', strategy: 'cost', reason: 'cost_optimized', message: response.body.cost!.reason,
    })
    expect(response.body.cost!.outcome).toBe('COST_WON')
    expect(response.body.connectors).toEqual([
      { gateway_name: 'adyen', payment_merchant_connector_id: null },
      { gateway_name: 'stripe', payment_merchant_connector_id: null },
    ])
  })

  test('carries every hybrid fact for connectors named without an account', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0 })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [{ gateway_name: 'stripe' }, { gateway_name: 'adyen' }]
    await trainScores(api, body, 'stripe')
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.connectors).toEqual([
      { gateway_name: 'stripe', payment_merchant_connector_id: null },
      { gateway_name: 'adyen', payment_merchant_connector_id: null },
    ])
  })
})

test.describe('Decide contract (API)', () => {
  test('accepts the full payload and preserves the no-rule fallback', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    const requestId = factory.paymentId('request')
    const response = await api.raw<DecideResponse>('POST', '/decide', {
      body, headers: { 'x-request-id': requestId },
    })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expect(response.status).toBe(200)
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body).toMatchObject({
      object: 'routing_decision', payment_id: body.payment_id, request_id: requestId,
      outcome: { type: 'fallback', strategy: 'candidate_order', reason: 'no_active_rule' },
      success_rate: null, cost: null, volume_steering: null, debit_routing: null, warnings: [],
    })
    expect(response.body.id).toMatch(/^dec_[0-9a-f]{32}$/)
    expect(Number.isNaN(Date.parse(response.body.created_at))).toBe(false)
    expect(response.headers['x-request-id']).toBe(requestId)
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
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.connectors).toEqual([
      { gateway_name: selected.gateway_name, payment_merchant_connector_id: selected.gateway_id },
    ])
    expect(response.body.outcome).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_matched' })
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
      expectFieldMapParity(response.body, hybrid.body)
      // With one candidate the decider has nothing to rank and keeps the default order.
      expect(response.body.outcome).toMatchObject(enabled
        ? { type: 'routed', strategy: 'candidate_order', reason: 'decider_default' }
        : { type: 'routed', strategy: 'rule', reason: 'rule_matched' })
      expect(response.body.success_rate?.status ?? null).toBe(enabled ? 'applied' : null)
    })
  }

  test('rejects removed fields, legacy connector IDs and missing required data', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    for (const invalid of [
      { ...body, payment_type: 'normal' },
      { ...body, context: {} },
      { ...body, amount: -1 },
      { ...body, amount: undefined },
      { ...body, amount: undefined, order_amount: body.amount },
      { ...body, order_amount: body.amount },
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
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.outcome).toMatchObject({ type: 'fallback', strategy: 'candidate_order', reason: 'rule_input_invalid' })
    // The rule error hybrid drops is reported.
    expect(response.body.warnings).toEqual([expect.objectContaining({ code: 'rule_evaluation_failed', param: null })])
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
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.connectors).toEqual([])
    expect(response.body.outcome).toMatchObject({ type: 'no_connector', strategy: 'none', reason: 'no_eligible_connectors' })
    // Without candidates, a missing rule is a rule-layer error, which hybrid drops and /decide reports.
    expect(response.body.warnings).toEqual([expect.objectContaining({ code: 'rule_evaluation_failed' })])
  })
})
