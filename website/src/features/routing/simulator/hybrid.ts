import { DecideGatewayResponse, GatewayConnector } from '../../../types/api'

// `/decide-gateway` takes the SR request as is. `/routing/hybrid` sends the same request as its
// dynamic half, plus a static half that evaluates the active rule on the payment's attributes.
export type SimulationEndpoint = 'decide_gateway' | 'hybrid_routing'

export const SIMULATION_ENDPOINTS: { value: SimulationEndpoint; label: string; path: string }[] = [
  { value: 'decide_gateway', label: 'Decide gateway', path: '/decide-gateway' },
  { value: 'hybrid_routing', label: 'Hybrid routing', path: '/routing/hybrid' },
]

export interface HybridRoutingResponse {
  static_routing?: unknown
  dynamic_routing?: { status: string; decision?: DecideGatewayResponse | null; error?: unknown }
  evaluated_connectors?: GatewayConnector[]
}

// Rule parameters for the static half of a hybrid request. Only keys the routing-key config knows
// are sent, and enum values only when the config lists them (matched case-insensitively), so an
// attribute the rule engine can't take is left out instead of failing the request.
export function hybridRuleParameters(
  routingKeys: Record<string, { type: string; values: string[] }>,
  attributes: Record<string, string | number | undefined>,
): Record<string, { type: string; value: string | number }> {
  const parameters: Record<string, { type: string; value: string | number }> = {}
  for (const [key, raw] of Object.entries(attributes)) {
    const config = routingKeys[key]
    if (!config || raw === undefined || raw === '') continue
    if (typeof raw === 'number') {
      if (config.type === 'integer' || config.type === 'number') parameters[key] = { type: 'number', value: Math.round(raw) }
      continue
    }
    const value = config.values.find(v => v === raw) ?? config.values.find(v => v.toLowerCase() === raw.toLowerCase())
    if (value !== undefined) parameters[key] = { type: 'enum_variant', value }
  }
  return parameters
}

// The routing decision a hybrid call made: the SR decision when the dynamic half ran, otherwise the
// head of the rule output (SR routing off for the merchant, or an A/B arm without an SR layer).
export function decisionFromHybrid(response: HybridRoutingResponse): DecideGatewayResponse {
  const decision = response.dynamic_routing?.decision
  if (decision) return decision
  const [head, ...rest] = response.evaluated_connectors ?? []
  if (!head) {
    const dynamicError = response.dynamic_routing?.error
    throw new Error(dynamicError ? `Hybrid routing failed: ${JSON.stringify(dynamicError)}` : 'Hybrid routing returned no connector')
  }
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
