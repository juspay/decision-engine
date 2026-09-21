import { RoutingAlgorithm, ABTestAlgorithmData, ExperimentArm, SrConfigOverride } from '../../../types/api'
import { ABTestExperimentType, ABTestFormValues, ArmLayersForm, DEFAULT_VARIANT_SR_CONFIG, SrStrategy } from './types'
import { resolvedArm, scopedEndpoints } from './arms'

// Infer the experiment type from the persisted arm shape (the backend stores no "type").
export function inferExperimentType(abData: ABTestAlgorithmData): ABTestExperimentType {
  const v = resolvedArm(abData, 'variant').sr
  // SR config tuning: variant tweaks hedging/elimination.
  if (v && (v.hedging_percent !== undefined || v.elimination_threshold !== undefined)) return 'sr_config_tuning'
  return 'algorithm_comparison'
}

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
  const runningControl = runningData ? resolvedArm(runningData, 'control') : undefined
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
  const variantSr = resolvedArm(abData, 'variant').sr
  return {
    name: algorithm.name,
    experimentType: inferExperimentType(abData),
    controlSource: 'saved',
    control: armForm(resolvedArm(abData, 'control')),
    variant: armForm(resolvedArm(abData, 'variant')),
    endpoints: [...scopedEndpoints(abData)],
    variantSplitPct: abData.variant_split_pct,
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
    variantSrConfig: {
      hedgingPercent: variantSr?.hedging_percent ?? null,
      eliminationThreshold: variantSr?.elimination_threshold ?? null,
    },
  }
}

export { DEFAULT_VARIANT_SR_CONFIG }
