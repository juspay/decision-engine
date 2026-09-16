import { RoutingAlgorithm, ABTestAlgorithmData, ArmStrategy, SrConfigOverride } from '../../../types/api'
import {
  ABTestExperimentType,
  ABTestFormValues,
  ArmFormValue,
  ArmSrLeg,
  DEFAULT_VARIANT_SR_CONFIG,
  EMPTY_ARM,
  LIVE_SR,
} from './types'

export type ArmSide = 'control' | 'variant'

// Which SR leg a stored override describes — the reverse of payload.ts SR_STRATEGY_CONFIG.
//
// An override that pins neither dial is NOT one of the four strategies: it is an arm that runs the
// merchant's live settings, which is how control arms were stored before `resolve_current_arm`
// pinned them. Collapsing it into `sr_auth` (the both-off strategy) is what made such an arm read
// as "manual tuning" while the merchant had autopilot switched on — it pins nothing, so the
// backend defaults `use_autopilot` to true and cost to the feature flag.
function srStrategyOf(config?: SrConfigOverride): ArmSrLeg {
  if (config?.enable_multi_objective === undefined && config?.use_autopilot === undefined) {
    return LIVE_SR
  }
  if (config?.enable_multi_objective === true) {
    return config.use_autopilot === true ? 'sr_mo_autopilot' : 'sr_mo_manual'
  }
  // Auth-based (cost explicitly off) — split by autopilot.
  return config?.use_autopilot === true ? 'sr_auth_autopilot' : 'sr_auth'
}

/**
 * One stored arm, normalized to the form's two-leg shape. Reads both the current per-arm form and
 * the older flat one, where an arm was a single id and 'sr_routing' was the reserved value meaning
 * "no rule, run SR" — so an experiment created before hybrid arms existed still renders correctly.
 */
export function parseStoredArm(data: ABTestAlgorithmData, side: ArmSide): ArmFormValue {
  const arm: ArmStrategy | undefined = side === 'control' ? data.control : data.variant
  if (arm) {
    switch (arm.kind) {
      case 'rule':
        return { algorithmId: arm.algorithm_id, srStrategy: null }
      case 'sr':
        return { algorithmId: '', srStrategy: srStrategyOf(arm.sr_config) }
      case 'hybrid':
        return { algorithmId: arm.algorithm_id, srStrategy: srStrategyOf(arm.sr_config) }
      case 'current':
        // Names nothing of its own — it is resolved into a concrete arm when the experiment is
        // created, so a stored arm is only ever 'current' if some other path wrote the row.
        return EMPTY_ARM
      default:
        return EMPTY_ARM
    }
  }

  const legacyId = side === 'control' ? data.control_algorithm_id : data.variant_algorithm_id
  const legacyConfig = side === 'control' ? data.control_sr_config : data.variant_sr_config
  if (!legacyId) return EMPTY_ARM
  return legacyId === 'sr_routing'
    ? { algorithmId: '', srStrategy: srStrategyOf(legacyConfig) }
    : { algorithmId: legacyId, srStrategy: null }
}

/** The arm's SR overrides, whichever shape it was stored in. */
export function storedSrConfig(data: ABTestAlgorithmData, side: ArmSide): SrConfigOverride | undefined {
  const arm: ArmStrategy | undefined = side === 'control' ? data.control : data.variant
  if (arm) return arm.kind === 'sr' || arm.kind === 'hybrid' ? arm.sr_config : undefined
  return side === 'control' ? data.control_sr_config : data.variant_sr_config
}

// Infer the experiment type from the persisted arm shape (the backend stores no "type").
// SR config tuning: both arms run SR alone and the variant tweaks hedging/elimination.
function inferExperimentType(abData: ABTestAlgorithmData): ABTestExperimentType {
  const variantConfig = storedSrConfig(abData, 'variant')
  const tweaksParams =
    variantConfig?.hedging_percent !== undefined || variantConfig?.elimination_threshold !== undefined
  const bothPureSr =
    parseStoredArm(abData, 'control').algorithmId === '' &&
    parseStoredArm(abData, 'variant').algorithmId === ''
  return tweaksParams && bothPureSr ? 'sr_config_tuning' : 'algorithm_comparison'
}

export function toABTestFormValues(algorithm: RoutingAlgorithm): ABTestFormValues | null {
  const data = (algorithm.algorithm_data || algorithm.algorithm)
  if (!data || data.type !== 'ab_test') return null
  const abData = data.data as ABTestAlgorithmData
  const variantConfig = storedSrConfig(abData, 'variant')
  return {
    name: algorithm.name,
    experimentType: inferExperimentType(abData),
    // The stored control arm is carried back verbatim rather than re-resolved: it was pinned to
    // the merchant's setup when the experiment was created, and re-reading it on save would move
    // the baseline out from under data already collected against it.
    control: parseStoredArm(abData, 'control'),
    variant: parseStoredArm(abData, 'variant'),
    variantSplitPct: abData.variant_split_pct,
    minSampleSize: abData.min_sample_size,
    guardrailThresholdPp: abData.guardrail_threshold_pp,
    variantSrConfig: {
      hedgingPercent: variantConfig?.hedging_percent ?? null,
      eliminationThreshold: variantConfig?.elimination_threshold ?? null,
    },
  }
}

export { DEFAULT_VARIANT_SR_CONFIG }
