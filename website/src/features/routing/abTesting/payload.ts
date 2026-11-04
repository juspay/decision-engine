import { ABTestAlgorithmData } from '../../../types/api'
import { ABTestFormValues, ABTestCreatePayload, ABTestAlgorithmPayload, ArmLayersForm, ExperimentArmPayload, ExperimentVariantPayload, SrConfigOverrideForm, SrConfigOverridePayload, SrStrategy, VariantForm } from './types'

function toSrConfigPayload(form: SrConfigOverrideForm): SrConfigOverridePayload {
  const out: SrConfigOverridePayload = {}
  if (form.hedgingPercent !== null) out.hedging_percent = form.hedgingPercent
  if (form.eliminationThreshold !== null) out.elimination_threshold = form.eliminationThreshold
  return out
}

function description(values: ABTestFormValues): string {
  const prefix = values.experimentType === 'sr_config_tuning' ? 'SR config tuning' : 'A/B test'
  const total = values.variants.reduce((sum, v) => sum + v.splitPct, 0)
  if (values.variants.length === 1) return `${prefix}: ${total}% variant traffic`
  return `${prefix}: ${values.variants.length} variants, ${total}% variant traffic`
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
  const tuning = values.experimentType === 'sr_config_tuning'
  // SR config tuning: every arm runs SR; control uses the live config, each variant tweaks
  // hedging/elimination.
  // In algorithm comparison, a variant's SR layer keeps any hedging/elimination overrides it was
  // read with, so duplicating or editing a mixed experiment saves them unchanged.
  const comparisonArm = (v: VariantForm): ExperimentArmPayload => {
    const arm = toArmPayload(v.layers)
    return arm.sr ? { ...arm, sr: { ...arm.sr, ...toSrConfigPayload(v.srConfig) } } : arm
  }
  const variant = (v: VariantForm): ExperimentVariantPayload => ({
    id: v.id,
    name: v.name.trim() || v.id,
    split_pct: v.splitPct,
    ...(tuning ? { sr: toSrConfigPayload(v.srConfig) } : comparisonArm(v)),
  })
  const data: ABTestAlgorithmPayload = {
    control: tuning ? { sr: {} } : toArmPayload(values.control),
    variants: values.variants.map(variant),
    endpoints: values.endpoints,
    min_sample_size: values.minSampleSize,
    guardrail_threshold_pp: values.guardrailThresholdPp,
  }

  return {
    name: values.name.trim(),
    description: description(values),
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
