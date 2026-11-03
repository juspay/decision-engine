import { ABTestAlgorithmData, ExperimentArm, ExperimentEndpoint, SrConfigOverride } from '../../../types/api'

// Mirrors the backend's `ab_test::arms`: an arm is a bundle of layers, and each endpoint applies
// only the layers it supports. An experiment has one control and one or more variants. A variant's
// traffic is split on an endpoint only when it is in the experiment's scope, the variant is still
// running, and it differs from control there; elsewhere the control arm is served untracked.

export const SR_ROUTING_ARM_ID = 'sr_routing'
export const CONTROL_ARM_ID = 'control'
/** Arm id the single-variant format records its variant under. */
export const LEGACY_VARIANT_ARM_ID = 'variant'
/** Same limits the backend validates. */
export const MAX_VARIANTS = 5
export const MAX_TOTAL_VARIANT_PCT = 50

/** One variant with the single-variant format already read into the multi-variant shape. */
export interface ResolvedVariant {
  id: string
  name: string
  splitPct: number
  arm: ExperimentArm
  stopped: boolean
}

export const EXPERIMENT_ENDPOINTS: ExperimentEndpoint[] = ['hybrid_routing', 'decide_gateway', 'evaluate']

export const ENDPOINT_LABELS: Record<ExperimentEndpoint, string> = {
  hybrid_routing: 'Decide',
  decide_gateway: 'Decide gateway',
  evaluate: 'Rule evaluate',
}

export const ENDPOINT_PATHS: Record<ExperimentEndpoint, string> = {
  // `/routing/hybrid` runs the same evaluation and shares this endpoint's experiments.
  hybrid_routing: '/decide',
  decide_gateway: '/decide-gateway',
  evaluate: '/routing/evaluate',
}

export const ENDPOINT_LAYERS: Record<ExperimentEndpoint, string> = {
  hybrid_routing: 'Applies rules and SR',
  decide_gateway: 'Applies SR only',
  evaluate: 'Applies rules only',
}

function singleStrategyArm(id?: string, sr?: SrConfigOverride): ExperimentArm {
  if (id === SR_ROUTING_ARM_ID) return { sr: sr ?? {} }
  if (id) return { rule_algorithm_id: id }
  return {}
}

/** The control arm, reading the single-strategy format as an arm with just that one layer. */
export function resolvedControl(data: ABTestAlgorithmData): ExperimentArm {
  return data.control ?? singleStrategyArm(data.control_algorithm_id, data.control_sr_config)
}

/** The variants in order. The single-variant format reads as one variant with the id `variant`. */
export function resolvedVariants(data: ABTestAlgorithmData): ResolvedVariant[] {
  if (data.variants) {
    return data.variants.map(({ id, name, split_pct, stopped, rule_algorithm_id, sr }) => ({
      id,
      name: name || id,
      splitPct: split_pct,
      arm: { ...(rule_algorithm_id && { rule_algorithm_id }), ...(sr && { sr }) },
      stopped: stopped === true,
    }))
  }
  return [{
    id: LEGACY_VARIANT_ARM_ID,
    name: 'Variant',
    splitPct: data.variant_split_pct ?? 0,
    arm: data.variant ?? singleStrategyArm(data.variant_algorithm_id, data.variant_sr_config),
    stopped: false,
  }]
}

/** The arm recorded under `armId` (`control` or a variant id); `undefined` for an unknown id. */
export function armById(data: ABTestAlgorithmData, armId: string): ExperimentArm | undefined {
  if (armId === CONTROL_ARM_ID) return resolvedControl(data)
  return resolvedVariants(data).find(v => v.id === armId)?.arm
}

/** Display name of the arm recorded under `armId`. */
export function armName(data: ABTestAlgorithmData | undefined, armId: string): string {
  if (armId === CONTROL_ARM_ID) return 'Control'
  return (data && resolvedVariants(data).find(v => v.id === armId)?.name) || armId
}

/** Control's share of traffic: whatever the variants don't take. */
export function controlSplitPct(data: ABTestAlgorithmData): number {
  return 100 - resolvedVariants(data).reduce((sum, v) => sum + v.splitPct, 0)
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

export function routesDifferently(a: ExperimentArm, b: ExperimentArm, endpoint: ExperimentEndpoint): boolean {
  return !sameArm(routingBehaviour(a, endpoint), routingBehaviour(b, endpoint))
}

/** Whether this variant's traffic is split from control on `endpoint`. */
export function variantSplitsOn(data: ABTestAlgorithmData, variant: ResolvedVariant, endpoint: ExperimentEndpoint): boolean {
  return !variant.stopped
    && scopedEndpoints(data).includes(endpoint)
    && routesDifferently(resolvedControl(data), variant.arm, endpoint)
}

/** Whether some running variant is split from control on `endpoint`. */
export function splitsOn(data: ABTestAlgorithmData, endpoint: ExperimentEndpoint): boolean {
  return resolvedVariants(data).some(v => variantSplitsOn(data, v, endpoint))
}

/** Endpoints where traffic is actually split, in display order. */
export function splitEndpoints(data: ABTestAlgorithmData): ExperimentEndpoint[] {
  return EXPERIMENT_ENDPOINTS.filter(e => splitsOn(data, e))
}
