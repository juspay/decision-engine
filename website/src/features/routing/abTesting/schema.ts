import { ABTestFormValues } from './types'

export function validateABTestForm(values: ABTestFormValues): string | null {
  if (!values.name.trim()) return 'Enter an experiment name'

  if (values.experimentType === 'sr_config_tuning') {
    const v = values.variantSrConfig
    if (v.hedgingPercent !== null && (v.hedgingPercent < 0 || v.hedgingPercent > 100))
      return 'Hedging % must be between 0 and 100'
    if (v.eliminationThreshold !== null && (v.eliminationThreshold < 0 || v.eliminationThreshold > 1))
      return 'Elimination threshold must be between 0 and 1'
    if (v.hedgingPercent === null && v.eliminationThreshold === null)
      return 'Set at least one parameter override for the variant arm'
  } else if (values.experimentType === 'cost_on_off') {
    // The variant needs a realistic margin (< 1) for cost to actually win — the default 1.0 is
    // effectively auth-only, which would make the experiment meaningless.
    if (values.variantMargin === null)
      return 'Set the variant margin so cost routing has an effect'
    if (values.variantMargin <= 0 || values.variantMargin > 1)
      return 'Variant margin must be between 0 and 1 (fraction of ticket)'
  } else if (values.experimentType === 'autopilot_value') {
    // No extra fields — both arms are SR routing; the only difference is manual vs autopilot config.
  } else {
    if (!values.controlAlgorithmId) return 'Select a control routing algorithm'
    if (!values.variantAlgorithmId) return 'Select a variant routing algorithm'
    if (values.controlAlgorithmId === values.variantAlgorithmId)
      return 'Control and variant must be different algorithms'
  }

  if (values.variantSplitPct < 5 || values.variantSplitPct > 30)
    return 'Variant traffic must be between 5% and 30%'
  if (values.minSampleSize < 100)
    return 'Minimum sample size must be at least 100 transactions'
  if (values.guardrailThresholdPp <= 0 || values.guardrailThresholdPp > 20)
    return 'Guardrail threshold must be between 0.1 and 20 percentage points'
  return null
}
