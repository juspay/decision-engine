import { DecideGatewayResponse, DecideRequest, DecideResponse } from '../../../types/api'

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

// The routing decision a `/decide` call made: the SR decision when SR ran, otherwise the head of
// the rule output (SR routing off for the merchant, or an A/B arm without an SR layer).
export function decisionFromDecide(response: DecideResponse): DecideGatewayResponse {
  const decision = response.dynamic_routing?.decision
  if (decision) return decision
  const [head, ...rest] = response.evaluated_connectors
  if (!head) throw new Error(`Decide returned no connector (${response.decision.outcome})`)
  return {
    decided_gateway: head.gateway_name,
    fallback_gateways: rest.map(c => c.gateway_name),
    routing_approach: 'RULE_OUTPUT',
    gateway_priority_map: null,
    routing_dimension: null,
    routing_dimension_level: null,
    filter_wise_gateways: null,
    reset_approach: 'NO_RESET',
    is_scheduled_outage: false,
    latency: null,
  }
}
