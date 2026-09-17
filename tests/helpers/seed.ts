import type { ApiClient, ApiResponse } from '../fixtures/api-client'
import factory from '../fixtures/factory'
import { poll } from './poll'

/**
 * Shared analytics seeding.
 *
 * Four places grew a near-identical copy of this sequence: cypress/e2e/ui/dashboard-overview.cy.js,
 * analytics-page.cy.js, payment-audit.cy.js, and inline in tests/api/analytics.spec.ts. They differ in
 * exactly three parameters — whether an advanced rule is created, whether a preview evaluation runs,
 * and whether the score feedback is a success or a failure — so they collapse into one function with
 * an options bag.
 *
 * The `waitFor*` helpers below are the other half of the duplication: every consumer follows the seed
 * with the same poll-until-ClickHouse-catches-up loop.
 */

export interface SeedOptions {
  /** Create + activate an advanced routing algorithm. Default true. */
  withAdvancedRule?: boolean
  /** Run a /routing/evaluate preview so preview-trace has something to find. Default true. */
  withPreviewEvaluation?: boolean
  /** Feedback status posted to /update-gateway-score. Default 'AUTHORIZED'. */
  scoreStatus?: 'AUTHORIZED' | 'FAILURE'
  /** Reported gateway latency on the score update. */
  gatewayLatency?: number
  /** Prefix for generated payment ids, to keep failures traceable to a spec. */
  prefix?: string
}

export interface SeededTraffic {
  decisionPaymentId: string
  previewPaymentId?: string
  decidedGateway: string
  ruleId?: string
  /** Body of the preview /routing/evaluate call, when one ran — lets callers assert on the output. */
  previewEvaluation?: any
}

export interface SeedHybridOptions {
  /** Feedback status posted to /update-gateway-score. Default 'AUTHORIZED'. */
  scoreStatus?: 'AUTHORIZED' | 'FAILURE'
  /** Prefix for generated ids, to keep failures traceable to a spec. */
  prefix?: string
}

export interface SeededHybridTraffic {
  paymentId: string
  decidedGateway: string
  ruleId: string
  /** Body of the /routing/hybrid call. */
  hybridResponse: any
}

/** The audit events one hybrid-routed payment leaves, in trail order: the hybrid call, then feedback. */
export const HYBRID_AUDIT_TRAIL = ['routing_hybrid_decision', 'update_gateway_score_update'] as const

/** Generate a decision + score-update (and optionally a rule + preview evaluation) for a merchant. */
export async function seedRoutedTraffic(
  api: ApiClient,
  merchantId: string,
  options: SeedOptions = {},
): Promise<SeededTraffic> {
  const {
    withAdvancedRule = true,
    withPreviewEvaluation = true,
    scoreStatus = 'AUTHORIZED',
    gatewayLatency,
    prefix = 'seed',
  } = options

  await api.createSuccessRateConfig(merchantId)

  let ruleId: string | undefined
  if (withAdvancedRule) {
    const created = await api.createRoutingAlgorithm(
      factory.advancedRoutingPayload(merchantId, { name: factory.ruleName(`${prefix}_adv`) }),
    )
    ruleId = created.body.rule_id
    await api.activateRoutingAlgorithm(merchantId, ruleId!)
  }

  const decisionPaymentId = factory.paymentId(`${prefix}_decision`)
  const decide = await api.decideGateway(
    factory.srDecideGatewayRequest({
      merchantId,
      paymentInfo: { paymentId: decisionPaymentId },
    }),
  )
  const decidedGateway: string = decide.body.decided_gateway

  await api.updateGatewayScore(
    factory.updateGatewayScoreRequest({
      merchantId,
      gateway: decidedGateway,
      paymentId: decisionPaymentId,
      status: scoreStatus,
      ...(gatewayLatency === undefined ? {} : { txnLatency: { gatewayLatency } }),
    }),
  )

  let previewPaymentId: string | undefined
  let previewEvaluation: any
  if (withPreviewEvaluation && withAdvancedRule) {
    previewPaymentId = factory.paymentId(`${prefix}_preview`)
    const evaluated = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(
        merchantId,
        {
          payment_method: { type: 'enum_variant', value: 'card' },
          amount: { type: 'number', value: 250 },
        },
        { payment_id: previewPaymentId },
      ),
    )
    previewEvaluation = evaluated.body
  }

  return { decisionPaymentId, previewPaymentId, decidedGateway, ruleId, previewEvaluation }
}

/**
 * Poll /analytics/overview until every named route has been recorded. Routes are the internal names
 * the API reports, e.g. '/decide_gateway', '/update_gateway', '/rule_evaluate'.
 */
export function waitForOverviewRouteHits(api: ApiClient, routes: string[]): Promise<ApiResponse> {
  return poll(
    () => api.raw('GET', '/analytics/overview', { failOnStatusCode: false, qs: { range: '1h' } }),
    ({ body }) =>
      Array.isArray(body?.route_hits) &&
      routes.every((route) => body.route_hits.some((hit: any) => hit.route === route)),
    { message: `Expected analytics overview to record route hits: ${routes.join(', ')}` },
  )
}

export function waitForHybridDecisions(api: ApiClient, minimum = 1): Promise<ApiResponse> {
  return poll(
    () =>
      api.raw('GET', '/analytics/overview', {
        failOnStatusCode: false,
        qs: { range: '1h', routing_kind: 'hybrid' },
      }),
    ({ body }) => (body?.hybrid_split?.decisions ?? 0) >= minimum,
    { message: `Expected analytics overview to count at least ${minimum} hybrid decision(s)` },
  )
}

/** Poll /analytics/payment-audit until the payment's timeline contains a given flow type. */
export function waitForAuditFlowType(
  api: ApiClient,
  paymentId: string,
  flowType: string,
): Promise<ApiResponse> {
  return poll(
    () =>
      api.raw('GET', '/analytics/payment-audit', {
        failOnStatusCode: false,
        qs: { range: '1h', payment_id: paymentId },
      }),
    ({ body }) =>
      Array.isArray(body?.timeline) && body.timeline.some((e: any) => e.flow_type === flowType),
    { message: `Expected payment audit timeline for ${paymentId} to contain ${flowType}` },
  )
}

/** Poll /analytics/preview-trace until the preview payment's timeline contains a given flow type. */
export function waitForPreviewFlowType(
  api: ApiClient,
  paymentId: string,
  flowType: string,
): Promise<ApiResponse> {
  return poll(
    () =>
      api.raw('GET', '/analytics/preview-trace', {
        failOnStatusCode: false,
        qs: { range: '1h', payment_id: paymentId },
      }),
    ({ body }) =>
      Array.isArray(body?.timeline) && body.timeline.some((e: any) => e.flow_type === flowType),
    { message: `Expected preview trace for ${paymentId} to contain ${flowType}` },
  )
}

/**
 * One payment routed the way Hyperswitch routes under DE cutover: a single POST /routing/hybrid
 * carrying both the static rule request and the SR request, followed by the score feedback. Its
 * audit trail is `HYBRID_AUDIT_TRAIL`: one event for the whole hybrid call, then the score update.
 */
export async function seedHybridTraffic(
  api: ApiClient,
  merchantId: string,
  options: SeedHybridOptions = {},
): Promise<SeededHybridTraffic> {
  const { scoreStatus = 'AUTHORIZED', prefix = 'hybrid' } = options

  await api.createSuccessRateConfig(merchantId)
  const created = await api.createRoutingAlgorithm(
    factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName(`${prefix}_single`),
      gateway: 'stripe',
    }),
  )
  const ruleId: string = created.body.rule_id
  await api.activateRoutingAlgorithm(merchantId, ruleId)

  const paymentId = factory.paymentId(`${prefix}_payment`)
  const hybrid = await api.raw('POST', '/routing/hybrid', {
    body: {
      static_routing_request: {
        created_by: merchantId,
        payment_id: paymentId,
        parameters: {
          payment_method: { type: 'enum_variant', value: 'card' },
          amount: { type: 'number', value: 100 },
        },
        fallback_output: [factory.gatewayConnector('stripe')],
      },
      dynamic_routing_request: factory.srDecideGatewayRequest({
        merchantId,
        paymentInfo: { paymentId },
      }),
    },
  })
  const decidedGateway: string | undefined = hybrid.body?.dynamic_routing?.decision?.decided_gateway
  if (!decidedGateway) {
    throw new Error(
      `hybrid routing returned no dynamic decision: ${JSON.stringify(hybrid.body).slice(0, 800)}`,
    )
  }

  await api.updateGatewayScore(
    factory.updateGatewayScoreRequest({
      merchantId,
      gateway: decidedGateway,
      paymentId,
      status: scoreStatus,
    }),
  )

  return { paymentId, decidedGateway, ruleId, hybridResponse: hybrid.body }
}

/**
 * Poll /analytics/payment-audit until the payment's timeline contains every named flow type AND its
 * summary entry counts them. The handler runs the summary query before the timeline query, so a poll
 * that lands between a Kafka flush and those two reads can see the timeline populated while the entry
 * is still empty; requiring both makes the settled response safe to assert on.
 */
export function waitForAuditFlowTypes(
  api: ApiClient,
  paymentId: string,
  flowTypes: readonly string[],
  qs: Record<string, unknown> = {},
): Promise<ApiResponse> {
  return poll(
    () =>
      api.raw('GET', '/analytics/payment-audit', {
        failOnStatusCode: false,
        qs: { range: '1h', payment_id: paymentId, ...qs },
      }),
    ({ body }) =>
      Array.isArray(body?.timeline) &&
      flowTypes.every((flowType) => body.timeline.some((e: any) => e.flow_type === flowType)) &&
      Array.isArray(body?.results) &&
      body.results.some((row: any) => row.payment_id === paymentId && row.event_count >= flowTypes.length),
    { message: `Expected payment audit for ${paymentId} to list it with ${flowTypes.join(', ')}` },
  )
}

/**
 * Poll the payment-audit LIST (no payment id) until a payment appears under a routing-type filter.
 * The list reads the pre-aggregated summaries, which are fed by their own Kafka consumer and can
 * lag the raw timeline by a moment.
 */
export function waitForAuditListing(
  api: ApiClient,
  paymentId: string,
  qs: Record<string, unknown> = {},
): Promise<ApiResponse> {
  return poll(
    () =>
      api.raw('GET', '/analytics/payment-audit', {
        failOnStatusCode: false,
        qs: { range: '1h', page: 1, page_size: 50, ...qs },
      }),
    ({ body }) =>
      Array.isArray(body?.results) && body.results.some((row: any) => row.payment_id === paymentId),
    { message: `Expected payment audit list ${JSON.stringify(qs)} to include ${paymentId}` },
  )
}
