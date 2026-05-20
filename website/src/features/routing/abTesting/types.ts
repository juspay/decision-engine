export interface ABTestFormValues {
  name: string
  controlAlgorithmId: string
  variantAlgorithmId: string
  variantSplitPct: number
  minSampleSize: number
  guardrailThresholdPp: number
}

export interface ABTestAlgorithmPayload {
  control_algorithm_id: string
  variant_algorithm_id: string
  variant_split_pct: number
  min_sample_size: number
  guardrail_threshold_pp: number
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
