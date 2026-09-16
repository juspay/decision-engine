import { ABTestFormValues, armHasRule, armRunsSr } from './types'

/** The form control a validation issue belongs to, so the form can put the cursor there. */
export type ABTestFormField =
  | 'name'
  | 'controlArm'
  | 'variantArm'
  | 'variantSrConfig'
  | 'variantSplitPct'
  | 'minSampleSize'
  | 'guardrailThresholdPp'

export interface ABTestFormIssue {
  field: ABTestFormField
  /**
   * Why the value is wrong. Omitted when the field being empty is the whole story — putting the
   * cursor in a blank required input says "fill this in" better than a sentence does, and a
   * message the reader has to connect back to a control is worse than no message.
   */
  message?: string
}

export function validateABTestForm(values: ABTestFormValues): ABTestFormIssue | null {
  if (!values.name.trim()) return { field: 'name' }

  if (values.experimentType === 'sr_config_tuning') {
    const v = values.variantSrConfig
    if (v.hedgingPercent !== null && (v.hedgingPercent < 0 || v.hedgingPercent > 100))
      return { field: 'variantSrConfig', message: 'Hedging % must be between 0 and 100' }
    if (v.eliminationThreshold !== null && (v.eliminationThreshold < 0 || v.eliminationThreshold > 1))
      return { field: 'variantSrConfig', message: 'Elimination threshold must be between 0 and 1' }
    if (v.hedgingPercent === null && v.eliminationThreshold === null)
      return { field: 'variantSrConfig', message: 'Set at least one parameter override for the variant arm' }
  } else {
    // algorithm_comparison — an arm needs at least one leg to route with. The control is checked
    // only when it is named outright; left as the merchant's live setup the backend resolves it,
    // and rejects the merchant having nothing to resolve.
    if (values.control && !armHasRule(values.control) && !armRunsSr(values.control))
      return { field: 'controlArm', message: 'Give the control a routing rule, success-rate routing, or both' }
    if (!armHasRule(values.variant) && !armRunsSr(values.variant))
      return { field: 'variantArm', message: 'Give the variant a routing rule, success-rate routing, or both' }
  }

  if (values.variantSplitPct < 5 || values.variantSplitPct > 30)
    return { field: 'variantSplitPct', message: 'Variant traffic must be between 5% and 30%' }
  if (values.minSampleSize < 100)
    return { field: 'minSampleSize', message: 'Minimum sample size must be at least 100 transactions' }
  if (values.guardrailThresholdPp <= 0 || values.guardrailThresholdPp > 20)
    return { field: 'guardrailThresholdPp', message: 'Guardrail threshold must be between 0.1 and 20 percentage points' }
  return null
}
