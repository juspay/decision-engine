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
