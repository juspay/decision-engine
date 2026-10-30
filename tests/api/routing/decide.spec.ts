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

/** ISO codes the specs send, and the country names the rule configuration uses for them. */
const COUNTRY_NAMES: Record<string, string> = { US: 'UnitedStatesOfAmerica' }

/**
 * The `/routing/hybrid` request for the same payment: the rule parameters `/decide` derives from its
 * body, plus the preferred connector, which hybrid only reads from its SR request.
 */
function legacyRequest(request: DecideRequest) {
  const enumValue = (value?: string | null) => (value ? { type: 'enum_variant', value } : undefined)
  const strValue = (value?: string | null) => (value ? { type: 'str_value', value } : undefined)
  const country = (code?: string | null) => enumValue(code ? COUNTRY_NAMES[code.toUpperCase()] ?? code : null)
  const card = request.card ?? {}
  const parameters = Object.fromEntries(Object.entries({
    amount: { type: 'number', value: request.amount },
    payment_amount: { type: 'number', value: request.amount },
    currency: enumValue(request.currency),
    payment_method: enumValue(request.payment_method_type),
    payment_method_type: enumValue(request.payment_method_sub_type),
    payment_type: enumValue(request.payment_type),
    billing_country: country(request.billing_country),
    card_bin: strValue(card.card_isin),
    extended_card_bin: strValue(card.card_extended_bin),
    card_network: enumValue(card.card_network),
    card_type: enumValue(card.card_type),
    issuer_country: country(card.card_issuing_country_code),
    ...Object.fromEntries(Object.entries(request.metadata ?? {}).map(([key, value]) => [
      key, { type: 'metadata_variant', value: { key, value } },
    ])),
  }).filter(([, value]) => value !== undefined))
  const preferred = request.routing_config.preferred_connectors ?? []
  return {
    static_routing_request: {
      created_by: request.merchant_id,
      payment_id: request.payment_id,
      algorithm_for: request.routing_config.algorithm_for,
      fallback_output: request.routing_config.eligible_connectors.map(c => ({
        gateway_name: c.gateway_name,
        gateway_id: c.payment_merchant_connector_id ?? null,
      })),
      parameters,
    },
    ...(preferred.length ? {
      dynamic_routing_request: factory.srDecideGatewayRequest({
        merchantId: request.merchant_id,
        paymentInfo: { paymentId: request.payment_id, preferredConnectors: preferred.map(identifierOf) },
      }),
    } : {}),
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
    expect(decide.success_rate!.diagnostics).toMatchObject({
      head_before_downtime_check: legacy.gateway_before_evaluation ? fromIdentifier(legacy.gateway_before_evaluation) : null,
      reset_approach: legacy.reset_approach,
      dimension: { value: legacy.routing_dimension, level: legacy.routing_dimension_level },
      filter_wise_gateways: legacy.filter_wise_gateways,
      priority_logic: { tag: legacy.priority_logic_tag },
      dynamic_mga_enabled: legacy.is_dynamic_mga_enabled,
      gateway_mga_id_map: legacy.gateway_mga_id_map,
      rust_based_decider: legacy.is_rust_based_decider,
    })
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
      payment_id: body.payment_id, request_id: requestId,
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

  test('routes mandate payments by payment_type as hybrid does', async ({ api, merchant }) => {
    const tokenHolder = factory.gatewayConnector('checkout', 'mca_token_holder')
    const rules = [{
      name: 'Processor token', routing_type: 'priority', output: { priority: [tokenHolder] },
      statements: [{ condition: [
        { lhs: 'payment_type', comparison: 'equal', value: { type: 'enum_variant', value: 'ppt_mandate' }, metadata: {} },
      ] }],
    }]
    const created = await api.createRoutingAlgorithm(factory.advancedRoutingPayload(merchant.id, { rules }))
    await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors.push({
      gateway_name: tokenHolder.gateway_name, payment_merchant_connector_id: tokenHolder.gateway_id!,
    })

    body.payment_type = 'ppt_mandate'
    const mandate = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expectFieldMapParity(mandate.body, hybrid.body)
    expect(mandate.body.outcome).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_matched' })
    expect(mandate.body.connectors).toEqual([
      { gateway_name: tokenHolder.gateway_name, payment_merchant_connector_id: tokenHolder.gateway_id },
    ])

    // A rule that tests payment_type cannot match a payment that does not send it.
    body.payment_type = null
    const unsent = await api.raw<DecideResponse>('POST', '/decide', { body })
    expect(unsent.body.outcome).toMatchObject({ type: 'fallback', reason: 'no_rule_matched' })
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
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacy })
    expect(hybrid.body.static_routing).toBeUndefined()
    expectFieldMapParity(response.body, hybrid.body)
    expect(response.body.outcome).toMatchObject({ type: 'fallback', strategy: 'candidate_order', reason: 'rule_input_invalid' })
    // The rule error hybrid drops is reported, by the request's own field.
    expect(response.body.warnings).toEqual([{
      code: 'rule_input_invalid', param: 'payment_method_sub_type',
      message: "'unrecognized_subtype' is not a configured value for payment_method_sub_type.",
    }])
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

test.describe('Decide scenarios (API)', () => {
  const stripe = { gateway_name: 'stripe', payment_merchant_connector_id: 'mca_stripe' }
  const adyen = { gateway_name: 'adyen', payment_merchant_connector_id: 'mca_adyen' }
  const checkout = { gateway_name: 'checkout', payment_merchant_connector_id: 'mca_checkout' }

  async function activate(api: any, merchantId: string, payload: any) {
    const created = await api.createRoutingAlgorithm(payload)
    await api.activateRoutingAlgorithm(merchantId, created.body.rule_id)
  }

  /** The same payment through `/decide` and `/routing/hybrid`. */
  async function both(api: any, body: DecideRequest) {
    const decide: { status: number; body: DecideResponse } = await api.raw('POST', '/decide', { body })
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: legacyRequest(body) })
    expect(decide.status).toBe(200)
    return { decide: decide.body, hybrid: hybrid.body }
  }

  test('applies a card/credit sub-level override to credit payments only', async ({ api, merchant }) => {
    // No hedging by default; the credit-card override hedges every payment.
    await api.createSuccessRateConfig(merchant.id, {
      defaultHedgingPercent: 0,
      subLevelInputConfig: [{
        paymentMethod: 'card', paymentMethodType: 'credit', hedgingPercent: 100, bucketSize: null, latencyThreshold: null,
      }],
    })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    const payment = (subType: string) => {
      const body = decideRequest(merchant.id)
      body.payment_method_sub_type = subType
      body.card = { ...body.card, card_type: subType }
      body.routing_config.eligible_connectors = [stripe, adyen]
      return body
    }

    const credit = await both(api, payment('credit'))
    expect(credit.decide.success_rate?.approach).toBe('SR_V3_HEDGING')
    expect(credit.decide.outcome).toMatchObject({ type: 'routed', strategy: 'success_rate', reason: 'success_rate_exploration' })
    expect(credit.hybrid.dynamic_routing.decision.routing_approach).toBe('SR_V3_HEDGING')

    const debit = await both(api, payment('debit'))
    expect(debit.decide.success_rate?.approach).toBe('SR_SELECTION_V3_ROUTING')
    expect(debit.decide.outcome).toMatchObject({ type: 'routed', strategy: 'success_rate', reason: 'success_rate_ranked' })
    expect(debit.hybrid.dynamic_routing.decision.routing_approach).toBe('SR_SELECTION_V3_ROUTING')

    // Both endpoints score the payment under the same SR dimension.
    for (const { decide, hybrid } of [credit, debit]) {
      expect(decide.success_rate?.diagnostics).toMatchObject({
        dimension: { value: hybrid.dynamic_routing.decision.routing_dimension, level: hybrid.dynamic_routing.decision.routing_dimension_level },
      })
    }

    // The specific method sent as the category matches no override, and /decide says why.
    const misplaced = payment('credit')
    misplaced.payment_method_type = 'credit'
    misplaced.payment_method_sub_type = null
    const response = await api.raw<DecideResponse>('POST', '/decide', { body: misplaced })
    expect(response.body.success_rate?.approach).not.toBe('SR_V3_HEDGING')
    expect(response.body.warnings).toContainEqual(expect.objectContaining({ code: 'rule_input_invalid', param: 'payment_method_type' }))
  })

  test('writes each rule output type in the response connector shape', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [stripe, adyen]

    await activate(api, merchant.id, factory.singleRoutingPayload(merchant.id, { gateway: 'stripe', gateway_id: 'mca_stripe' }))
    let { decide, hybrid } = await both(api, body)
    expectFieldMapParity(decide, hybrid)
    expect(decide.rule?.output).toEqual({ type: 'straight_through', connector: stripe })
    expect(decide.connectors).toEqual([stripe])

    await activate(api, merchant.id, factory.volumeSplitRoutingPayload(merchant.id, {
      data: [{ split: 100, output: factory.gatewayConnector('adyen') }, { split: 0, output: factory.gatewayConnector('stripe') }],
    }))
    ;({ decide, hybrid } = await both(api, body))
    expectFieldMapParity(decide, hybrid)
    expect(decide.rule?.output).toEqual({ type: 'volume_split', splits: [{ connector: adyen, split: 100 }, { connector: stripe, split: 0 }] })
    expect(decide.connectors[0]).toEqual(adyen)
    expect(decide.outcome).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_matched' })
  })

  test('falls back to the candidates when no advanced rule matches', async ({ api, merchant }) => {
    await activate(api, merchant.id, factory.advancedRoutingPayload(merchant.id, {
      rules: [{
        name: 'Wallets', routing_type: 'priority', output: { priority: [factory.gatewayConnector('checkout')] },
        statements: [{ condition: [{ lhs: 'payment_method', comparison: 'equal', value: { type: 'enum_variant', value: 'wallet' }, metadata: {} }] }],
      }],
    }))
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [stripe, adyen, checkout]
    const { decide, hybrid } = await both(api, body)
    expectFieldMapParity(decide, hybrid)
    expect(decide.outcome).toMatchObject({ type: 'fallback', strategy: 'candidate_order', reason: 'no_rule_matched' })
    expect(decide.connectors).toEqual([stripe, adyen, checkout])
    expect(decide.warnings).toEqual([])
  })

  test('reports a rule output outside the candidates, wholly and partly', async ({ api, merchant }) => {
    await activate(api, merchant.id, factory.priorityRoutingPayload(merchant.id, {
      connectors: [factory.gatewayConnector('checkout'), factory.gatewayConnector('stripe')],
    }))
    const warning = (code: string) => ({ code, param: 'routing_config.eligible_connectors', message: expect.any(String) })

    const partly = decideRequest(merchant.id)
    partly.routing_config.eligible_connectors = [stripe, adyen]
    let { decide, hybrid } = await both(api, partly)
    expectFieldMapParity(decide, hybrid)
    expect(decide.outcome).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_output_partially_eligible' })
    expect(decide.connectors).toEqual([stripe])
    expect(decide.warnings).toEqual([warning('rule_output_partially_eligible')])

    const wholly = decideRequest(merchant.id)
    wholly.routing_config.eligible_connectors = [adyen]
    ;({ decide, hybrid } = await both(api, wholly))
    expectFieldMapParity(decide, hybrid)
    expect(decide.outcome).toMatchObject({ type: 'fallback', strategy: 'candidate_order', reason: 'rule_output_not_eligible' })
    expect(decide.connectors).toEqual([adyen])
    expect(decide.warnings).toEqual([warning('rule_output_not_eligible')])
  })

  test('names every invalid value by its own field', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    body.payment_method_sub_type = 'unrecognized_subtype'
    body.card = { ...body.card, card_network: 'VISAA' }
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    expect(response.body.outcome.reason).toBe('rule_input_invalid')
    expect(response.body.connectors).toEqual(body.routing_config.eligible_connectors)
    expect([...response.body.warnings].sort((a, b) => a.param!.localeCompare(b.param!))).toEqual([
      { code: 'rule_input_invalid', param: 'card.card_network', message: "'VISAA' is not a configured value for card.card_network." },
      { code: 'rule_input_invalid', param: 'payment_method_sub_type', message: "'unrecognized_subtype' is not a configured value for payment_method_sub_type." },
    ])
  })

  test('matches configured values in any case', async ({ api, merchant }) => {
    await activate(api, merchant.id, factory.advancedRoutingPayload(merchant.id, {
      rules: [{
        name: 'Credit cards', routing_type: 'priority', output: { priority: [factory.gatewayConnector('checkout')] },
        statements: [{ condition: [
          { lhs: 'payment_method', comparison: 'equal', value: { type: 'enum_variant', value: 'card' }, metadata: {} },
          { lhs: 'payment_method_type', comparison: 'equal', value: { type: 'enum_variant', value: 'credit' }, metadata: {} },
        ] }],
      }],
    }))
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [stripe, checkout]
    body.payment_method_type = 'CARD'
    body.payment_method_sub_type = 'Credit'
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    expect(response.body.warnings).toEqual([])
    expect(response.body.outcome).toMatchObject({ type: 'routed', reason: 'rule_matched' })
    expect(response.body.connectors).toEqual([checkout])
  })

  test('rejects a country that is not an ISO code', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    body.billing_country = 'UnitedStatesOfAmerica'
    const response = await api.raw('POST', '/decide', { body, failOnStatusCode: false })
    expect(response.status).toBe(400)
    expect(JSON.stringify(response.body)).toContain('ISO 3166 alpha-2')
  })

  test('routes to the preferred connector as hybrid does', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0 })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    const body = decideRequest(merchant.id)
    body.routing_config.eligible_connectors = [stripe, adyen]
    await trainScores(api, body, 'stripe:mca_stripe')
    body.routing_config.preferred_connectors = [adyen]
    const { decide, hybrid } = await both(api, body)
    expectFieldMapParity(decide, hybrid)
    expect(decide.connectors[0]).toEqual(adyen)
    expect(decide.outcome).toMatchObject({ type: 'routed', strategy: 'success_rate', reason: 'preferred_connector' })
  })

  test('mints a new id for every decision of the same payment', async ({ api, merchant }) => {
    const body = decideRequest(merchant.id)
    const first = await api.raw<DecideResponse>('POST', '/decide', { body })
    const second = await api.raw<DecideResponse>('POST', '/decide', { body })
    expect(first.body.payment_id).toBe(second.body.payment_id)
    expect(first.body.id).not.toBe(second.body.id)
    expect(first.body.id).toMatch(/^dec_[0-9a-f]{32}$/)
  })
})
