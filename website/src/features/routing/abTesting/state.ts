import { RoutingAlgorithm, ABTestAlgorithmData, ExperimentArm, SrConfigOverride } from '../../../types/api'
import { ABTestExperimentType, ABTestFormValues, ArmLayersForm, DEFAULT_VARIANT_SR_CONFIG, EMPTY_ARM, SrStrategy, VariantForm } from './types'
import { CONTROL_ARM_ID, resolvedControl, resolvedVariants, scopedEndpoints } from './arms'

const TUNING_KEYS: (keyof SrConfigOverride)[] = ['hedging_percent', 'elimination_threshold']

/** Whether the arm runs SR with no rule layer and sets no SR field outside `allowed`. */
function srOnlyWith(arm: ExperimentArm, allowed: (keyof SrConfigOverride)[]): boolean {
  if (arm.rule_algorithm_id || !arm.sr) return false
  const sr = arm.sr
  return (Object.keys(sr) as (keyof SrConfigOverride)[])
    .every(key => sr[key] === undefined || allowed.includes(key))
}

// Infer the experiment type from the persisted arm shape (the backend stores no "type"). SR config
// tuning is the shape that mode saves: control on the live SR config, and every variant on SR with
// only hedging/elimination overrides. Anything else, such as a rule-changing variant beside an SR
// tuning one, is an algorithm comparison, whose form keeps each arm's layers and overrides.
export function inferExperimentType(abData: ABTestAlgorithmData): ABTestExperimentType {
  const tunes = srOnlyWith(resolvedControl(abData), [])
    && resolvedVariants(abData).every(({ arm }) =>
      srOnlyWith(arm, TUNING_KEYS) && TUNING_KEYS.some(key => arm.sr?.[key] !== undefined))
  return tunes ? 'sr_config_tuning' : 'algorithm_comparison'
}

const LETTERS = 'abcdefghijklmnopqrstuvwxyz'

/**
 * A new variant whose id and name don't clash with `existing`: `variant_a` / "Variant A", then
 * `variant_b` / "Variant B", and so on.
 */
export function newVariant(existing: VariantForm[], layers: ArmLayersForm = EMPTY_ARM, splitPct = 10): VariantForm {
  const taken = new Set(existing.map(v => v.id))
  const letter = [...LETTERS].find(l => !taken.has(`variant_${l}`)) ?? String(existing.length + 1)
  return {
    id: `variant_${letter}`,
    name: `Variant ${letter.toUpperCase()}`,
    layers: { ...layers },
    srConfig: { ...DEFAULT_VARIANT_SR_CONFIG },
    splitPct,
  }
}

export { CONTROL_ARM_ID }

// Reverse of payload.ts `srStrategyConfig`. Cost off / absent reads as auth-based, split by autopilot.
export function srStrategyOf(config: SrConfigOverride): SrStrategy {
  if (config.enable_multi_objective === true) return config.use_autopilot === true ? 'sr_mo_autopilot' : 'sr_mo_manual'
  return config.use_autopilot === true ? 'sr_auth_autopilot' : 'sr_auth'
}

function armForm(arm: ExperimentArm): ArmLayersForm {
  return {
    ruleAlgorithmId: arm.rule_algorithm_id ?? '',
    srStrategy: arm.sr ? srStrategyOf(arm.sr) : '',
  }
}

/**
 * The control arm matching how live traffic routes today. With an experiment running, that is the
 * experiment's control arm. Otherwise it is the active payment routing config plus, when the
 * merchant has SR routing on, SR with its cost-savings setting and manual or autopilot tuning.
 */
export function currentSetupArm(
  activeAlgorithms: RoutingAlgorithm[],
  settings: { srRoutingOn: boolean; costSavingsOn: boolean; autopilotOn: boolean },
): ArmLayersForm {
  const { srRoutingOn, costSavingsOn, autopilotOn } = settings
  const payment = activeAlgorithms.filter(a => a.algorithm_for === 'payment')
  const running = payment.find(a => (a.algorithm_data || a.algorithm)?.type === 'ab_test')
  const runningData = running && ((running.algorithm_data || running.algorithm)?.data as ABTestAlgorithmData | undefined)
  const runningControl = runningData ? resolvedControl(runningData) : undefined
  if (runningControl) {
    return {
      ruleAlgorithmId: runningControl.rule_algorithm_id ?? '',
      // Unset dials follow the merchant settings.
      srStrategy: runningControl.sr
        ? srStrategyOf({
          enable_multi_objective: runningControl.sr.enable_multi_objective ?? costSavingsOn,
          use_autopilot: runningControl.sr.use_autopilot ?? autopilotOn,
        })
        : '',
    }
  }
  return {
    ruleAlgorithmId: payment.find(a => (a.algorithm_data || a.algorithm)?.type !== 'ab_test')?.id ?? '',
    srStrategy: srRoutingOn ? srStrategyOf({ enable_multi_objective: costSavingsOn, use_autopilot: autopilotOn }) : '',
  }
}

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  return {
    name: algorithm.name,
    experimentType: inferExperimentType(abData),
    controlSource: 'saved',
    control: armForm(resolvedControl(abData)),
    // Ids are kept, so an edited single-variant experiment still records its variant as `variant`.
    variants: resolvedVariants(abData).map(v => ({
      id: v.id,
      name: v.name,
      layers: armForm(v.arm),
      srConfig: {
        hedgingPercent: v.arm.sr?.hedging_percent ?? null,
        eliminationThreshold: v.arm.sr?.elimination_threshold ?? null,
      },
      splitPct: v.splitPct,
    })),
    endpoints: [...scopedEndpoints(abData)],
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
  }
}

export { DEFAULT_VARIANT_SR_CONFIG }
