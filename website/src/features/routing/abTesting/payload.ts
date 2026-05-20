import { ABTestFormValues, ABTestCreatePayload } from './types'

export function toABTestCreatePayload(
  values: ABTestFormValues,
  merchantId: string,
): ABTestCreatePayload {
  return {
    name: values.name.trim(),
    description: `A/B test: ${values.variantSplitPct}% variant traffic`,
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: {
        control_algorithm_id: values.controlAlgorithmId,
        variant_algorithm_id: values.variantAlgorithmId,
        variant_split_pct: values.variantSplitPct,
        min_sample_size: values.minSampleSize,
        guardrail_threshold_pp: values.guardrailThresholdPp,
      },
    },
  }
}
