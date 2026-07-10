import { ABTestFormValues, ABTestCreatePayload, ABTestAlgorithmPayload, SrConfigOverrideForm, SrConfigOverridePayload } from './types'

function toSrConfigPayload(form: SrConfigOverrideForm): SrConfigOverridePayload {
  const out: SrConfigOverridePayload = {}
  if (form.hedgingPercent !== null) out.hedging_percent = form.hedgingPercent
  if (form.eliminationThreshold !== null) out.elimination_threshold = form.eliminationThreshold
  return out
}

const DESCRIPTIONS: Record<ABTestFormValues['experimentType'], (pct: number) => string> = {
  sr_config_tuning: (pct) => `SR config tuning: ${pct}% variant traffic`,
  cost_on_off: (pct) => `Cost on/off: ${pct}% variant traffic`,
  autopilot_value: (pct) => `Autopilot value: ${pct}% variant traffic`,
  algorithm_comparison: (pct) => `A/B test: ${pct}% variant traffic`,
}

// Per-arm overrides for each experiment type. SR-arm experiments (all but algorithm_comparison)
// route both arms through `sr_routing` and differentiate them via control_sr_config / variant_sr_config.
function armConfigs(values: ABTestFormValues): {
  control?: SrConfigOverridePayload
  variant?: SrConfigOverridePayload
} {
  switch (values.experimentType) {
    case 'sr_config_tuning':
      // Control uses live SR config (no override); variant tweaks hedging/elimination.
      return { variant: toSrConfigPayload(values.variantSrConfig) }
    case 'cost_on_off':
      // Control: multi-objective off (auth-only). Variant: multi-objective on at the chosen margin.
      return {
        control: { enable_multi_objective: false },
        variant: {
          enable_multi_objective: true,
          ...(values.variantMargin !== null && { margin: values.variantMargin }),
        },
      }
    case 'autopilot_value':
      // Both arms cost-on; control ignores autopilot-tuned config (manual), variant uses it.
      return {
        control: { enable_multi_objective: true, use_autopilot: false },
        variant: { enable_multi_objective: true, use_autopilot: true },
      }
    default:
      return {}
  }
}

export function toABTestCreatePayload(
  values: ABTestFormValues,
  merchantId: string,
): ABTestCreatePayload {
  const isAlgoComparison = values.experimentType === 'algorithm_comparison'
  const { control, variant } = armConfigs(values)

  const data: ABTestAlgorithmPayload = {
    control_algorithm_id: isAlgoComparison ? values.controlAlgorithmId : 'sr_routing',
    variant_algorithm_id: isAlgoComparison ? values.variantAlgorithmId : 'sr_routing',
    variant_split_pct: values.variantSplitPct,
    min_sample_size: values.minSampleSize,
    guardrail_threshold_pp: values.guardrailThresholdPp,
    ...(variant && { variant_sr_config: variant }),
    ...(control && { control_sr_config: control }),
  }

  return {
    name: values.name.trim(),
    description: DESCRIPTIONS[values.experimentType](values.variantSplitPct),
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: { type: 'ab_test', data },
  }
}
