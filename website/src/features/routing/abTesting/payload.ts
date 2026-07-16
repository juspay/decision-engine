import { ABTestFormValues, ABTestCreatePayload, ABTestAlgorithmPayload, SrConfigOverrideForm, SrConfigOverridePayload } from './types'

function toSrConfigPayload(form: SrConfigOverrideForm): SrConfigOverridePayload {
  const out: SrConfigOverridePayload = {}
  if (form.hedgingPercent !== null) out.hedging_percent = form.hedgingPercent
  if (form.eliminationThreshold !== null) out.elimination_threshold = form.eliminationThreshold
  return out
}

const DESCRIPTIONS: Record<ABTestFormValues['experimentType'], (pct: number) => string> = {
  sr_config_tuning: (pct) => `SR config tuning: ${pct}% variant traffic`,
  algorithm_comparison: (pct) => `A/B test: ${pct}% variant traffic`,
}

// Resolve a form arm value into the stored (algorithm_id, sr_config) pair. The three SR strategies
// all map to 'sr_routing' with a distinguishing per-arm override; a real algorithm id maps to itself.
// Two independent dials — cost-awareness (enable_multi_objective) and autopilot (use_autopilot):
//  - sr_auth          → auth-only, manual config (cost off, autopilot off) — static auth baseline
//  - sr_auth_autopilot→ auth-only, autopilot on (cost off, but hedging/bucket self-tuned)
//  - sr_mo_manual     → cost-aware, manual config
//  - sr_mo_autopilot  → cost-aware, autopilot on
function resolveArm(value: string): { id: string; config?: SrConfigOverridePayload } {
  switch (value) {
    case 'sr_auth':
      return { id: 'sr_routing', config: { enable_multi_objective: false, use_autopilot: false } }
    case 'sr_auth_autopilot':
      return { id: 'sr_routing', config: { enable_multi_objective: false, use_autopilot: true } }
    case 'sr_mo_manual':
      return { id: 'sr_routing', config: { enable_multi_objective: true, use_autopilot: false } }
    case 'sr_mo_autopilot':
      return { id: 'sr_routing', config: { enable_multi_objective: true, use_autopilot: true } }
    default:
      return { id: value }
  }
}

export function toABTestCreatePayload(
  values: ABTestFormValues,
  merchantId: string,
): ABTestCreatePayload {
  const base = {
    variant_split_pct: values.variantSplitPct,
    min_sample_size: values.minSampleSize,
    guardrail_threshold_pp: values.guardrailThresholdPp,
  }

  let data: ABTestAlgorithmPayload
  if (values.experimentType === 'sr_config_tuning') {
    // Both arms SR; control uses live config, variant tweaks hedging/elimination.
    data = {
      ...base,
      control_algorithm_id: 'sr_routing',
      variant_algorithm_id: 'sr_routing',
      variant_sr_config: toSrConfigPayload(values.variantSrConfig),
    }
  } else {
    // algorithm_comparison — each arm is a resolved strategy (SR variants carry an override).
    const c = resolveArm(values.controlAlgorithmId)
    const v = resolveArm(values.variantAlgorithmId)
    data = {
      ...base,
      control_algorithm_id: c.id,
      variant_algorithm_id: v.id,
      ...(c.config && { control_sr_config: c.config }),
      ...(v.config && { variant_sr_config: v.config }),
    }
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
