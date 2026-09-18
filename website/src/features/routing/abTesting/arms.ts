import { ABTestAlgorithmData, ExperimentArm, ExperimentEndpoint, SrConfigOverride } from '../../../types/api'

// Mirrors the backend's `ab_test::arms`: an arm is a bundle of layers, and each endpoint applies
// only the layers it supports. Traffic splits on an endpoint only when it is in the experiment's
// scope and the two arms differ there; elsewhere the control arm is served untracked.

export type ArmSide = 'control' | 'variant'

export const SR_ROUTING_ARM_ID = 'sr_routing'

export const EXPERIMENT_ENDPOINTS: ExperimentEndpoint[] = ['hybrid_routing', 'decide_gateway', 'evaluate']

export const ENDPOINT_LABELS: Record<ExperimentEndpoint, string> = {
  hybrid_routing: 'Hybrid routing',
  decide_gateway: 'Decide gateway',
  evaluate: 'Rule evaluate',
}

export const ENDPOINT_PATHS: Record<ExperimentEndpoint, string> = {
  hybrid_routing: '/routing/hybrid',
  decide_gateway: '/decide-gateway',
  evaluate: '/routing/evaluate',
}

export const ENDPOINT_LAYERS: Record<ExperimentEndpoint, string> = {
  hybrid_routing: 'Applies rules and SR',
  decide_gateway: 'Applies SR only',
  evaluate: 'Applies rules only',
}

/** The arm for `side`, reading the single-strategy format as an arm with just that one layer. */
export function resolvedArm(data: ABTestAlgorithmData, side: ArmSide): ExperimentArm {
  const layered = side === 'control' ? data.control : data.variant
  if (layered) return layered
  const id = side === 'control' ? data.control_algorithm_id : data.variant_algorithm_id
  const sr = side === 'control' ? data.control_sr_config : data.variant_sr_config
  if (id === SR_ROUTING_ARM_ID) return { sr: sr ?? {} }
  if (id) return { rule_algorithm_id: id }
  return {}
}

export function projectArm(arm: ExperimentArm, endpoint: ExperimentEndpoint): ExperimentArm {
  switch (endpoint) {
    case 'hybrid_routing': return arm
    case 'decide_gateway': return arm.sr ? { sr: arm.sr } : {}
    case 'evaluate': return arm.rule_algorithm_id ? { rule_algorithm_id: arm.rule_algorithm_id } : {}
  }
}

function sameSr(a?: SrConfigOverride, b?: SrConfigOverride): boolean {
  if (!a || !b) return a === b
  const keys: (keyof SrConfigOverride)[] = ['hedging_percent', 'elimination_threshold', 'enable_multi_objective', 'margin', 'use_autopilot']
  return keys.every(k => a[k] === b[k])
}

export function sameArm(a: ExperimentArm, b: ExperimentArm): boolean {
  return a.rule_algorithm_id === b.rule_algorithm_id && sameSr(a.sr, b.sr)
}

export function scopedEndpoints(data: ABTestAlgorithmData): ExperimentEndpoint[] {
  return data.endpoints ?? EXPERIMENT_ENDPOINTS
}

// The projection reduced to how the endpoint routes: `/decide-gateway` always runs SR, and an arm
// without an SR layer runs it with the merchant's settings, the same as an SR layer with no overrides.
function routingBehaviour(arm: ExperimentArm, endpoint: ExperimentEndpoint): ExperimentArm {
  const projection = projectArm(arm, endpoint)
  return endpoint === 'decide_gateway' ? { sr: projection.sr ?? {} } : projection
}

export function splitsOn(data: ABTestAlgorithmData, endpoint: ExperimentEndpoint): boolean {
  return scopedEndpoints(data).includes(endpoint)
    && !sameArm(routingBehaviour(resolvedArm(data, 'control'), endpoint), routingBehaviour(resolvedArm(data, 'variant'), endpoint))
}

/** Endpoints where traffic is actually split, in display order. */
export function splitEndpoints(data: ABTestAlgorithmData): ExperimentEndpoint[] {
  return EXPERIMENT_ENDPOINTS.filter(e => splitsOn(data, e))
}
