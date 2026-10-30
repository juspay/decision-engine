import {
  BlockedCommitment,
  DecideConnector,
  DecideGatewayResponse,
  DecideRequest,
  DecideResponse,
  VolumeSteerInfo,
} from '../../../types/api'

// `/decide-gateway` takes the SR request as is. `/decide` takes the same payment as one flat
// request: it evaluates the active rule on the payment's attributes, then SR among the rule output.
export type SimulationEndpoint = 'decide_gateway' | 'decide'

export const SIMULATION_ENDPOINTS: { value: SimulationEndpoint; label: string; path: string }[] = [
  { value: 'decide_gateway', label: 'Decide gateway', path: '/decide-gateway' },
  { value: 'decide', label: 'Decide', path: '/decide' },
]

// A saved endpoint, or null when it names none. `hybrid_routing` (the `/routing/hybrid` option)
// reads as `/decide`, which runs the same evaluation.
export function savedSimulationEndpoint(value: unknown): SimulationEndpoint | null {
  if (value === 'hybrid_routing') return 'decide'
  return SIMULATION_ENDPOINTS.find(e => e.value === value)?.value ?? null
}

export interface SimulatedPayment {
  merchantId: string
  paymentId: string
  amount: number
  currency: string
  paymentMethod: string
  paymentMethodType: string
  authType?: string
  cardNetwork?: string
  cardProgram?: string
  cardIssuerCountry?: string
  gateways: string[]
  eliminationEnabled: boolean
}

// The `/decide` request for one simulated payment. Connectors are named by gateway alone, as the
// simulator's gateways carry no connector account. Card attributes are sent for card payments only.
export function decideRequestFor(payment: SimulatedPayment): DecideRequest {
  const isCard = payment.paymentMethod.toUpperCase() === 'CARD'
  return {
    payment_id: payment.paymentId,
    merchant_id: payment.merchantId,
    amount: Math.round(payment.amount),
    currency: payment.currency,
    payment_method_type: payment.paymentMethod || null,
    payment_method_sub_type: payment.paymentMethodType || null,
    authentication_type: payment.authType || null,
    card: isCard
      ? {
        card_network: payment.cardNetwork || null,
        card_type: payment.paymentMethodType || null,
        card_subtype: payment.cardProgram || null,
        card_issuing_country_code: payment.cardIssuerCountry || null,
      }
      : null,
    routing_config: {
      algorithm_for: 'payment',
      ranking_algorithm: 'SR_BASED_ROUTING',
      elimination_enabled: payment.eliminationEnabled,
      eligible_connectors: payment.gateways.map(gateway_name => ({ gateway_name })),
    },
  }
}

// The decider's identifier for a connector: "name:id", or the bare name without an account.
export function connectorIdentifier(connector: DecideConnector): string {
  return connector.payment_merchant_connector_id
    ? `${connector.gateway_name}:${connector.payment_merchant_connector_id}`
    : connector.gateway_name
}

// The approach a row records when the rule layer set the order and success rate did not run.
export const RULE_DECIDED_APPROACH = 'RULE_OUTPUT'

// What a simulated payment's row takes from its routing decision, whichever endpoint made it.
export interface SimulatedDecision {
  decidedGateway: string
  fallbackGateways: string[]
  routingApproach: string | null
  gatewayPriorityMap: Record<string, number> | null
  costSavedBps: number | null
  costWon: boolean
  authWon: boolean
  headAuthRate: number | null
  chosenAuthRate: number | null
  margin: number | null
  evGapTop2: number | null
  steerOutcome: VolumeSteerInfo['outcome'] | null
  steerSrHead: string | null
  steerBlocked: BlockedCommitment[] | null
}

export function simulatedDecisionFromDecideGateway(res: DecideGatewayResponse): SimulatedDecision {
  const cost = res.multi_objective_info ?? null
  const steer = res.volume_steer_info ?? null
  return {
    decidedGateway: res.decided_gateway,
    fallbackGateways: res.fallback_gateways,
    routingApproach: res.routing_approach ?? null,
    gatewayPriorityMap: res.gateway_priority_map ?? null,
    costSavedBps: cost?.costSavedBps ?? null,
    costWon: cost?.outcome === 'COST_WON',
    authWon: cost?.outcome === 'AUTH_WON',
    headAuthRate: cost?.ranked?.find(r => r.isSrHead)?.authRate ?? null,
    chosenAuthRate: cost?.ranked?.find(r => r.isChosen)?.authRate ?? null,
    margin: cost?.margin ?? null,
    evGapTop2: cost?.evGapTop2 ?? null,
    steerOutcome: steer?.outcome ?? null,
    steerSrHead: steer?.srHead ?? null,
    steerBlocked: steer?.blocked ?? null,
  }
}

export function simulatedDecisionFromDecide(res: DecideResponse): SimulatedDecision {
  const [head, ...rest] = res.connectors
  if (!head) throw new Error(`Decide returned no connector (${res.outcome.reason})`)
  const sr = res.success_rate?.status === 'applied' ? res.success_rate : null
  const { cost, volume_steering: steer } = res
  return {
    decidedGateway: connectorIdentifier(head),
    fallbackGateways: rest.map(connectorIdentifier),
    routingApproach: sr ? sr.approach : RULE_DECIDED_APPROACH,
    gatewayPriorityMap: sr
      ? Object.fromEntries(sr.scores.map(score => [connectorIdentifier(score), score.score]))
      : null,
    costSavedBps: cost?.cost_saved_bps ?? null,
    costWon: cost?.outcome === 'COST_WON',
    authWon: cost?.outcome === 'AUTH_WON',
    headAuthRate: cost?.ranked.find(r => r.is_sr_head)?.auth_rate ?? null,
    chosenAuthRate: cost?.ranked.find(r => r.is_chosen)?.auth_rate ?? null,
    margin: cost?.margin ?? null,
    evGapTop2: cost?.ev_gap_top2 ?? null,
    steerOutcome: steer?.outcome ?? null,
    steerSrHead: steer?.sr_head ? connectorIdentifier(steer.sr_head) : null,
    steerBlocked: steer
      ? steer.blocked.map(({ connector, gate }) => ({ connector: connectorIdentifier(connector), gate }))
      : null,
  }
}
