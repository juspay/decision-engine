export type ABTestExperimentType =
  | 'algorithm_comparison'
  | 'sr_config_tuning'
  // Control = multi-objective OFF (auth-only SR) vs Variant = multi-objective ON (cost-aware).
  | 'cost_on_off'
  // Control = multi-objective ON with manual SR config vs Variant = ON with autopilot config.
  | 'autopilot_value'

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
  /** Variant-arm margin (fraction of ticket, 0–1) for the cost_on_off experiment. Lower margin
   * lets cost win more often. Null = use the merchant's configured/default margin. */
  variantMargin: number | null
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
