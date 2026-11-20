import { ABTestFormValues, MAX_VARIANT_PCT, MIN_VARIANT_PCT } from './types'
import { MAX_TOTAL_VARIANT_PCT, MAX_VARIANTS } from './arms'

/** Checks the fields that can still change on an experiment with recorded payments. */
export function validateEvaluationSettings(
  values: Pick<ABTestFormValues, 'name' | 'minSampleSize' | 'guardrailThresholdPp'>,
): string | null {
  if (!values.name.trim()) return 'Enter an experiment name'
  if (values.minSampleSize < 100)
    return 'Minimum sample size must be at least 100 transactions'
  if (values.guardrailThresholdPp <= 0 || values.guardrailThresholdPp > 20)
    return 'Guardrail threshold must be between 0.1 and 20 percentage points'
  return null
}

export function validateABTestForm(values: ABTestFormValues): string | null {
  if (!values.name.trim()) return 'Enter an experiment name'
  if (values.variants.length === 0) return 'Add at least one variant'
  if (values.variants.length > MAX_VARIANTS) return `An experiment can have at most ${MAX_VARIANTS} variants`
  const names = values.variants.map(v => v.name.trim().toLowerCase())
  if (names.some(n => !n)) return 'Give every variant a name'
  if (new Set(names).size !== names.length) return 'Give every variant a different name'

  for (const v of values.variants) {
    if (values.experimentType === 'sr_config_tuning') {
      const c = v.srConfig
      if (c.hedgingPercent !== null && (c.hedgingPercent < 0 || c.hedgingPercent > 100))
        return `${v.name}: hedging % must be between 0 and 100`
      if (c.eliminationThreshold !== null && (c.eliminationThreshold < 0 || c.eliminationThreshold > 1))
        return `${v.name}: elimination threshold must be between 0 and 1`
      if (c.hedgingPercent === null && c.eliminationThreshold === null)
        return `${v.name}: set at least one parameter override`
    } else if (!v.layers.ruleAlgorithmId && !v.layers.srStrategy) {
      return `Pick a routing rule, an SR strategy, or both for ${v.name}`
    }
  }
  if (values.experimentType === 'algorithm_comparison' && !values.control.ruleAlgorithmId && !values.control.srStrategy)
    return 'Pick a routing rule, an SR strategy, or both for the control arm'

  if (values.endpoints.length === 0) return 'Select at least one endpoint'
  // Variants may route the same as control or as each other (an A/A comparison), so arms are not
  // compared here.

  if (values.variants.some(v => v.splitPct < MIN_VARIANT_PCT || v.splitPct > MAX_VARIANT_PCT))
    return `Each variant's traffic must be between ${MIN_VARIANT_PCT}% and ${MAX_VARIANT_PCT}%`
  const total = values.variants.reduce((sum, v) => sum + v.splitPct, 0)
  if (total > MAX_TOTAL_VARIANT_PCT)
    return `Variants together can take at most ${MAX_TOTAL_VARIANT_PCT}% of traffic, so control keeps the majority`
  return validateEvaluationSettings(values)
}
