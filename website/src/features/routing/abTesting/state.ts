import { RoutingAlgorithm, ABTestAlgorithmData, SrConfigOverride } from '../../../types/api'
import { ABTestExperimentType, ABTestFormValues, DEFAULT_VARIANT_SR_CONFIG } from './types'

// Infer the experiment type from the persisted arm shape (the backend stores no "type").
function inferExperimentType(abData: ABTestAlgorithmData): ABTestExperimentType {
  const v = abData.variant_sr_config
  // SR config tuning: variant tweaks hedging/elimination.
  if (v && (v.hedging_percent !== undefined || v.elimination_threshold !== undefined)) return 'sr_config_tuning'
  // Everything else (including SR auth / multi-objective strategy pairs, and rule-based configs
  // picked as arms) is an algorithm comparison.
  return 'algorithm_comparison'
}

// Reverse of payload.ts `resolveArm`: turn a stored (algorithm_id, sr_config) pair back into the
// synthetic form arm value the ArmSelector uses.
function armFormValue(id: string, config?: SrConfigOverride): string {
  if (id !== 'sr_routing') return id
  if (config?.enable_multi_objective === true) return config.use_autopilot === true ? 'sr_mo_autopilot' : 'sr_mo_manual'
  return 'sr_auth' // enable_multi_objective false or absent → auth-based SR
}

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  return {
    name: algorithm.name,
    experimentType: inferExperimentType(abData),
    controlAlgorithmId: armFormValue(abData.control_algorithm_id, abData.control_sr_config),
    variantAlgorithmId: armFormValue(abData.variant_algorithm_id, abData.variant_sr_config),
    variantSplitPct: abData.variant_split_pct,
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
    variantSrConfig: {
      hedgingPercent: abData.variant_sr_config?.hedging_percent ?? null,
      eliminationThreshold: abData.variant_sr_config?.elimination_threshold ?? null,
    },
  }
}

export { DEFAULT_VARIANT_SR_CONFIG }
