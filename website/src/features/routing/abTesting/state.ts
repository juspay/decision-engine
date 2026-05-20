import { RoutingAlgorithm, ABTestAlgorithmData } from '../../../types/api'
import { ABTestFormValues } from './types'

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  return {
    name: algorithm.name,
    controlAlgorithmId: abData.control_algorithm_id,
    variantAlgorithmId: abData.variant_algorithm_id,
    variantSplitPct: abData.variant_split_pct,
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
  }
}
