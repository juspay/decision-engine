import { RoutingAlgorithm, ABTestAlgorithmData } from '../../../types/api'
import { ABTestFormValues, DEFAULT_VARIANT_SR_CONFIG } from './types'

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  const isTuning = Boolean(abData.variant_sr_config)
  return {
    name: algorithm.name,
    experimentType: isTuning ? 'sr_config_tuning' : 'algorithm_comparison',
    controlAlgorithmId: abData.control_algorithm_id,
    variantAlgorithmId: abData.variant_algorithm_id,
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
