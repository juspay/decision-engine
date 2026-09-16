export type ABTestExperimentType =
  // Compare any two routing strategies. The SR strategies (auth / multi-objective manual /
  // multi-objective autopilot) and rule-based/volume-split configs are all pickable arms here —
  // this is where auth-vs-cost and manual-vs-autopilot experiments now live.
  | 'algorithm_comparison'
  | 'sr_config_tuning'

// Synthetic arm values for the SR strategies. They all resolve to a `sr`/`hybrid` arm but carry
// different per-arm overrides (see payload.ts `srConfigFor`). Two independent dials —
// cost-awareness (multi-objective) and autopilot self-tuning — give four combinations. Kept
// distinct in the form so the arm dropdown can offer them separately.
export type SrStrategy = 'sr_auth' | 'sr_auth_autopilot' | 'sr_mo_manual' | 'sr_mo_autopilot'

// An SR leg that carries no overrides of its own: every dial falls back to the merchant's live
// settings — autopilot honored, cost-awareness from their feature flag. Deliberately NOT one of
// the four strategies above: those each pin `enable_multi_objective` and `use_autopilot` to a
// fixed value, whereas this one tracks whatever the merchant has configured today.
//
// Only stored experiments produce it now — control arms created before `resolve_current_arm`
// pinned its dials, and legacy flat-shaped rows. It is rendered but never offered as a new
// choice: a control arm that tracks live settings stops being a baseline the moment the merchant
// changes one, which is precisely what an experiment on that setting does.
export const LIVE_SR = 'sr_live'
export type ArmSrLeg = SrStrategy | typeof LIVE_SR
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

// Label for any SR leg, including the no-override one. Naming a pinned strategy here would claim
// settings the arm does not actually pin — the reason a control arm used to read as "manual
// tuning" while it was in fact running the merchant's autopilot.
export const LIVE_SR_LABEL = 'Success-rate routing · your live settings'
export const armSrLegLabel = (leg: ArmSrLeg): string =>
  leg === LIVE_SR ? LIVE_SR_LABEL : SR_STRATEGY_LABELS[leg]

/** Whether an SR leg honors autopilot-tuned segments. The two `_autopilot` strategies pin
 *  `use_autopilot: true`; the two manual ones pin it false (see SR_STRATEGY_CONFIG). A leg with no
 *  override honors it, matching `ab_use_autopilot ... .unwrap_or(true)` in gw_scoring.rs. */
export const legHonorsAutopilot = (leg: ArmSrLeg | null): boolean =>
  leg === LIVE_SR || leg === 'sr_auth_autopilot' || leg === 'sr_mo_autopilot'

// One arm, as the form holds it. The two legs are independent, which is what lets a merchant on
// hybrid routing describe an arm that runs both:
//   rule only    → the rule's first choice is the decision, SR never runs
//   SR only      → SR picks from every connector the engine's own filters allow
//   rule + SR    → the rule narrows the field, SR picks the winner among what's left
// An arm with neither leg is not a routing strategy and is rejected by validateABTestForm.
export interface ArmFormValue {
  /** Saved routing config this arm evaluates first. Empty means the arm has no rule leg. */
  algorithmId: string
  /** SR leg this arm ranks with. `null` means the rule's pick is final; `LIVE_SR` means it ranks
   *  with the merchant's live settings and pins nothing. */
  srStrategy: ArmSrLeg | null
}

export const EMPTY_ARM: ArmFormValue = { algorithmId: '', srStrategy: null }

export const armRunsSr = (arm: ArmFormValue): boolean => arm.srStrategy !== null
export const armHasRule = (arm: ArmFormValue): boolean => arm.algorithmId !== ''

export interface SrConfigOverrideForm {
  hedgingPercent: number | null
  eliminationThreshold: number | null
}

export const DEFAULT_VARIANT_SR_CONFIG: SrConfigOverrideForm = {
  hedgingPercent: null,
  eliminationThreshold: null,
}

export interface ABTestFormValues {
  name: string
  experimentType: ABTestExperimentType
  /**
   * The baseline. `null` — the default — means the merchant's live configuration, sent as
   * `{ kind: 'current' }` for the server to resolve and pin, which is what a control should be
   * nearly always: production, so the variant's numbers mean something.
   *
   * An explicit arm is for the case the live setup cannot express. Testing whether autopilot
   * beats a static rule needs SR configured for autopilot to have anything to tune, and the
   * calibration job creates that config itself — at which point the live setup reads as hybrid
   * and "my rule alone" stops being expressible as `current`. Naming the control arm outright is
   * the way to hold it at rule-only.
   */
  control: ArmFormValue | null
  variant: ArmFormValue
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
  enable_volume_commitment?: boolean
}

// Mirrors the backend `ArmStrategy`. `kind` names which legs the arm runs; a `hybrid` arm holds
// both, and its `sr_config` applies to the SR leg that ranks the rule's output.
export type ArmStrategyPayload =
  | { kind: 'current' }
  | { kind: 'rule'; algorithm_id: string }
  | { kind: 'sr'; sr_config?: SrConfigOverridePayload }
  | { kind: 'hybrid'; algorithm_id: string; sr_config?: SrConfigOverridePayload }

export interface ABTestAlgorithmPayload {
  control: ArmStrategyPayload
  variant: ArmStrategyPayload
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

/** What a merchant has to have switched on for an SR leg to route differently from a manual one.
 *  `null` when the leg needs nothing — its dials are per-arm overrides the decider reads directly.
 *
 *  Only autopilot has a prerequisite, and only because its values do not come from the arm: the
 *  calibration job writes them into the merchant's shared SR config tagged `source: "autopilot"`,
 *  and `use_autopilot` merely decides whether an arm reads entries carrying that tag. With the job
 *  off there are no such entries and the leg scores exactly like its manual twin. Multi-objective
 *  needs nothing — `enable_multi_objective` on the arm wins over the feature flag outright
 *  (flow_new.rs), which is what lets cost be tested before it is adopted. */
export const srLegPrerequisite = (leg: ArmSrLeg): 'autopilot' | null =>
  legHonorsAutopilot(leg) && leg !== LIVE_SR ? 'autopilot' : null
