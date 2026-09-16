import {
  ABTestFormValues,
  ABTestCreatePayload,
  ABTestAlgorithmPayload,
  ArmFormValue,
  ArmStrategyPayload,
  LIVE_SR,
  SrConfigOverrideForm,
  SrConfigOverridePayload,
  SrStrategy,
} from './types'

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

// The per-arm overrides behind each SR strategy. Two independent dials — cost-awareness
// (enable_multi_objective) and autopilot self-tuning (use_autopilot):
//  - sr_auth           → auth-only, manual config (cost off, autopilot off) — static auth baseline
//  - sr_auth_autopilot → auth-only, autopilot on (cost off, but hedging/bucket self-tuned)
//  - sr_mo_manual      → cost-aware, manual config
//  - sr_mo_autopilot   → cost-aware, autopilot on
const SR_STRATEGY_CONFIG: Record<SrStrategy, SrConfigOverridePayload> = {
  sr_auth: { enable_multi_objective: false, use_autopilot: false },
  sr_auth_autopilot: { enable_multi_objective: false, use_autopilot: true },
  sr_mo_manual: { enable_multi_objective: true, use_autopilot: false },
  sr_mo_autopilot: { enable_multi_objective: true, use_autopilot: true },
}

// Turn the form's two independent legs into the arm shape the backend stores. An arm with both
// legs is `hybrid`: the rule narrows the candidate set and the SR leg picks among what survives.
// `extraSrConfig` merges on top of the strategy's own dials — used by SR config tuning, where the
// variant additionally overrides hedging/elimination.
export function toArmStrategy(
  arm: ArmFormValue,
  extraSrConfig?: SrConfigOverridePayload,
): ArmStrategyPayload {
  // LIVE_SR pins nothing, so it contributes no dials — only an explicit tuning override can give
  // it an `sr_config` at all, and even then it stays unpinned on cost and autopilot.
  const strategyDials = arm.srStrategy && arm.srStrategy !== LIVE_SR
    ? SR_STRATEGY_CONFIG[arm.srStrategy]
    : {}
  const merged = { ...strategyDials, ...extraSrConfig }
  const srConfig = arm.srStrategy
    ? (Object.keys(merged).length > 0 ? merged : undefined)
    : undefined

  if (!arm.srStrategy) return { kind: 'rule', algorithm_id: arm.algorithmId }
  if (!arm.algorithmId) return { kind: 'sr', sr_config: srConfig }
  return { kind: 'hybrid', algorithm_id: arm.algorithmId, sr_config: srConfig }
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
    // Both arms SR; control uses live config, variant tweaks hedging/elimination on top of
    // whatever strategy it runs.
    data = {
      ...base,
      control: { kind: 'sr' },
      variant: toArmStrategy(
        { algorithmId: '', srStrategy: values.variant.srStrategy ?? LIVE_SR },
        toSrConfigPayload(values.variantSrConfig),
      ),
    }
  } else {
    data = {
      ...base,
      // `current` asks the backend to read the control off the merchant's active rule and SR
      // config and pin it. An explicit arm is sent as-is, for the baselines `current` cannot
      // name — chiefly "my rule alone" once autopilot has created an SR config alongside it.
      control: values.control ? toArmStrategy(values.control) : { kind: 'current' },
      variant: toArmStrategy(values.variant),
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
