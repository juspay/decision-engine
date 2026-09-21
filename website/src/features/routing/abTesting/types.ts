import { ExperimentEndpoint } from '../../../types/api'

export type ABTestExperimentType =
  // Compare any two arms, each a bundle of layers: a saved routing config (rule-based / priority /
  // volume split / single) for the rule layer, and an SR strategy for the SR layer. Either layer
  // may be left out, so rule vs SR, rule + SR vs rule + SR, and SR vs SR all live here.
  | 'algorithm_comparison'
  | 'sr_config_tuning'

// SR layer strategies. Two independent dials — cost-awareness (multi-objective) and autopilot
// self-tuning — give four combinations, each resolving to a distinct SR override.
export type SrStrategy = 'sr_auth' | 'sr_auth_autopilot' | 'sr_mo_manual' | 'sr_mo_autopilot'
// Labels name the routing *goal* (approvals, or approvals + fee savings) rather than the internal
// algorithm (SR / multi-objective), with the autopilot-vs-manual tuning mode as a trailing
// qualifier. Single source of truth: the create-form dropdown and `armLabel` (which renders
// existing experiments) both resolve through this map, so a rename here propagates everywhere.
export const SR_STRATEGY_LABELS: Record<SrStrategy, string> = {
  sr_auth: 'Maximize approvals · manual tuning',
  sr_auth_autopilot: 'Maximize approvals · auto-tuned',
  sr_mo_manual: 'Approvals + save on fees · manual',
  sr_mo_autopilot: 'Approvals + save on fees · auto-tuned',
}

export interface SrConfigOverrideForm {
  hedgingPercent: number | null
  eliminationThreshold: number | null
}

export const DEFAULT_VARIANT_SR_CONFIG: SrConfigOverrideForm = {
  hedgingPercent: null,
  eliminationThreshold: null,
}

/** One arm in the form. '' means the layer is not applied by this arm. */
export interface ArmLayersForm {
  ruleAlgorithmId: string
  srStrategy: SrStrategy | ''
}

export const EMPTY_ARM: ArmLayersForm = { ruleAlgorithmId: '', srStrategy: '' }

/**
 * Where the control arm comes from:
 * - `current`: the merchant's current routing setup, read when the experiment is created
 *   (`control` is ignored until then);
 * - `saved`: the control of the experiment this form was cloned from;
 * - `custom`: picked by the user.
 */
export type ControlSource = 'current' | 'saved' | 'custom'

/**
 * What editing an experiment may change:
 * - `checking`: still reading whether the experiment has recorded payments;
 * - `full`: it has none, so every setting can change;
 * - `evaluation`: its results are read against its setup, so only the name, sample target and
 *   guardrail can change.
 */
export type EditScope = 'checking' | 'full' | 'evaluation'

export interface ABTestFormValues {
  name: string
  experimentType: ABTestExperimentType
  /** Only used in algorithm_comparison mode. */
  controlSource: ControlSource
  /** Only used in algorithm_comparison mode, when `controlSource` is not `current`. */
  control: ArmLayersForm
  variant: ArmLayersForm
  /** Endpoints the experiment splits traffic on. */
  endpoints: ExperimentEndpoint[]
  variantSplitPct: number
  minSampleSize: number
  guardrailThresholdPp: number
  /** Only used in sr_config_tuning mode. Control always uses the live SR config. */
  variantSrConfig: SrConfigOverrideForm
}

export interface SrConfigOverridePayload {
  hedging_percent?: number
  elimination_threshold?: number
  enable_multi_objective?: boolean
  margin?: number
  use_autopilot?: boolean
}

export interface ExperimentArmPayload {
  rule_algorithm_id?: string
  sr?: SrConfigOverridePayload
}

export interface ABTestAlgorithmPayload {
  control: ExperimentArmPayload
  variant: ExperimentArmPayload
  endpoints: ExperimentEndpoint[]
  variant_split_pct: number
  min_sample_size: number
  guardrail_threshold_pp: number
}

export interface ABTestCreatePayload {
  name: string
  description: string
  created_by: string
  algorithm_for: 'payment'
  metadata: Record<string, unknown>
  algorithm: {
    type: 'ab_test'
    data: ABTestAlgorithmPayload
  }
}
