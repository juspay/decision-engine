import { RoutingAlgorithm, ABTestAlgorithmData } from '../../../types/api'
import { ABTestExperimentType, ABTestFormValues, DEFAULT_VARIANT_SR_CONFIG } from './types'

// Infer the experiment type from the persisted arm overrides (the backend doesn't store a "type").
function inferExperimentType(abData: ABTestAlgorithmData): ABTestExperimentType {
  const v = abData.variant_sr_config
  const c = abData.control_sr_config
  // Autopilot value: both arms toggle use_autopilot.
  if (v?.use_autopilot !== undefined || c?.use_autopilot !== undefined) return 'autopilot_value'
  // Cost on/off: arms toggle multi-objective.
  if (v?.enable_multi_objective !== undefined || c?.enable_multi_objective !== undefined) return 'cost_on_off'
  // SR config tuning: variant tweaks hedging/elimination.
  if (v && (v.hedging_percent !== undefined || v.elimination_threshold !== undefined)) return 'sr_config_tuning'
  return 'algorithm_comparison'
}

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  return {
    name: algorithm.name,
    experimentType: inferExperimentType(abData),
    controlAlgorithmId: abData.control_algorithm_id,
    variantAlgorithmId: abData.variant_algorithm_id,
    variantSplitPct: abData.variant_split_pct,
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
    variantSrConfig: {
      hedgingPercent: abData.variant_sr_config?.hedging_percent ?? null,
      eliminationThreshold: abData.variant_sr_config?.elimination_threshold ?? null,
    },
    variantMargin: abData.variant_sr_config?.margin ?? null,
  }
}

export { DEFAULT_VARIANT_SR_CONFIG }
