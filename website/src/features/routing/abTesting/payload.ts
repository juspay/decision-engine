import { ABTestAlgorithmData } from '../../../types/api'
import { ABTestFormValues, ABTestCreatePayload, ABTestAlgorithmPayload, ArmLayersForm, ExperimentArmPayload, SrConfigOverrideForm, SrConfigOverridePayload, SrStrategy } from './types'

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

// The SR override each strategy stores. Two independent dials — cost-awareness
// (enable_multi_objective) and autopilot (use_autopilot):
//  - sr_auth          → auth-only, manual config (cost off, autopilot off) — static auth baseline
//  - sr_auth_autopilot→ auth-only, autopilot on (cost off, but hedging/bucket self-tuned)
//  - sr_mo_manual     → cost-aware, manual config
//  - sr_mo_autopilot  → cost-aware, autopilot on
export function srStrategyConfig(strategy: SrStrategy): SrConfigOverridePayload {
  switch (strategy) {
    case 'sr_auth': return { enable_multi_objective: false, use_autopilot: false }
    case 'sr_auth_autopilot': return { enable_multi_objective: false, use_autopilot: true }
    case 'sr_mo_manual': return { enable_multi_objective: true, use_autopilot: false }
    case 'sr_mo_autopilot': return { enable_multi_objective: true, use_autopilot: true }
  }
}

function toArmPayload(arm: ArmLayersForm): ExperimentArmPayload {
  return {
    ...(arm.ruleAlgorithmId && { rule_algorithm_id: arm.ruleAlgorithmId }),
    ...(arm.srStrategy && { sr: srStrategyConfig(arm.srStrategy) }),
  }
}

export function toABTestCreatePayload(
  values: ABTestFormValues,
  merchantId: string,
): ABTestCreatePayload {
  const base = {
    endpoints: values.endpoints,
    variant_split_pct: values.variantSplitPct,
    min_sample_size: values.minSampleSize,
    guardrail_threshold_pp: values.guardrailThresholdPp,
  }

  const data: ABTestAlgorithmPayload = values.experimentType === 'sr_config_tuning'
    // Both arms SR; control uses live config, variant tweaks hedging/elimination.
    ? { ...base, control: { sr: {} }, variant: { sr: toSrConfigPayload(values.variantSrConfig) } }
    : { ...base, control: toArmPayload(values.control), variant: toArmPayload(values.variant) }

  return {
    name: values.name.trim(),
    description: DESCRIPTIONS[values.experimentType](values.variantSplitPct),
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: { type: 'ab_test', data },
  }
}

/**
 * A saved experiment's algorithm with only the sample target and guardrail replaced. Everything
 * else is sent back as stored, so the routing setup is unchanged.
 */
export function withEvaluationSettings<A extends { type: string; data: unknown }>(
  algorithm: A,
  values: Pick<ABTestFormValues, 'minSampleSize' | 'guardrailThresholdPp'>,
): A {
  return {
    ...algorithm,
    data: {
      ...(algorithm.data as ABTestAlgorithmData),
      min_sample_size: values.minSampleSize,
      guardrail_threshold_pp: values.guardrailThresholdPp,
    },
  }
}
