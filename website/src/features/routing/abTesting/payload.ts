import { ABTestFormValues, ABTestCreatePayload, SrConfigOverrideForm, SrConfigOverridePayload } from './types'

function toSrConfigPayload(form: SrConfigOverrideForm): SrConfigOverridePayload {
  const out: SrConfigOverridePayload = {}
  if (form.hedgingPercent !== null) out.hedging_percent = form.hedgingPercent
  if (form.eliminationThreshold !== null) out.elimination_threshold = form.eliminationThreshold
  return out
}

export function toABTestCreatePayload(
  values: ABTestFormValues,
  merchantId: string,
): ABTestCreatePayload {
  const isTuning = values.experimentType === 'sr_config_tuning'

  return {
    name: values.name.trim(),
    description: isTuning
      ? `SR config tuning: ${values.variantSplitPct}% variant traffic`
      : `A/B test: ${values.variantSplitPct}% variant traffic`,
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: {
      type: 'ab_test',
      data: {
        control_algorithm_id: isTuning ? 'sr_routing' : values.controlAlgorithmId,
        variant_algorithm_id: isTuning ? 'sr_routing' : values.variantAlgorithmId,
        variant_split_pct: values.variantSplitPct,
        min_sample_size: values.minSampleSize,
        guardrail_threshold_pp: values.guardrailThresholdPp,
        // Control always uses live SR config — no override sent.
        ...(isTuning && {
          variant_sr_config: toSrConfigPayload(values.variantSrConfig),
        }),
      },
    },
  }
}
