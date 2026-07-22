export type ABTestExperimentType =
  // Compare any two routing strategies. The SR strategies (auth / multi-objective manual /
  // multi-objective autopilot) and rule-based/volume-split configs are all pickable arms here —
  // this is where auth-vs-cost and manual-vs-autopilot experiments now live.
  | 'algorithm_comparison'
  | 'sr_config_tuning'

// Synthetic arm values for the SR strategies. They all resolve to algorithm_id 'sr_routing' but
// carry different per-arm overrides (see payload.ts `resolveArm`). Two independent dials —
// cost-awareness (multi-objective) and autopilot self-tuning — give four combinations. Kept
// distinct in the form so the arm dropdown and the `control !== variant` check can tell them apart.
export type SrStrategy = 'sr_auth' | 'sr_auth_autopilot' | 'sr_mo_manual' | 'sr_mo_autopilot'
// Labels name the routing *goal* (approvals, or approvals + fee savings) rather than the internal
// algorithm (SR / multi-objective), with the autopilot-vs-manual tuning mode as a trailing
// qualifier. Single source of truth: the create-form dropdown and `armLabel` (which renders
// existing experiments) both resolve through this map, so a rename here propagates everywhere.
export const SR_STRATEGY_LABELS: Record<SrStrategy, string> = {
  sr_auth: 'Maximize approvals · manual tuning',
  sr_auth_autopilot: 'Maximize approvals · auto-tuned',
  sr_mo_manual: 'Approvals + save on fees · manual',
  sr_mo_autopilot: 'Approvals + save on fees · auto-tuned',
}

export interface SrConfigOverrideForm {
  hedgingPercent: number | null
  eliminationThreshold: number | null
}

export const DEFAULT_VARIANT_SR_CONFIG: SrConfigOverrideForm = {
  hedgingPercent: null,
  eliminationThreshold: null,
}

export interface ABTestFormValues {
  name: string
  experimentType: ABTestExperimentType
  controlAlgorithmId: string
  variantAlgorithmId: string
  variantSplitPct: number
  minSampleSize: number
  guardrailThresholdPp: number
  /** Only used in sr_config_tuning mode. Control always uses the live SR config. */
  variantSrConfig: SrConfigOverrideForm
}

export interface SrConfigOverridePayload {
  hedging_percent?: number
  elimination_threshold?: number
  enable_multi_objective?: boolean
  margin?: number
  use_autopilot?: boolean
}

export interface ABTestAlgorithmPayload {
  control_algorithm_id: string
  variant_algorithm_id: string
  variant_split_pct: number
  min_sample_size: number
  guardrail_threshold_pp: number
  variant_sr_config?: SrConfigOverridePayload
  control_sr_config?: SrConfigOverridePayload
}

export interface ABTestCreatePayload {
  name: string
  description: string
  created_by: string
  algorithm_for: 'payment'
  metadata: Record<string, unknown>
  algorithm: {
    type: 'ab_test'
    data: ABTestAlgorithmPayload
  }
}
