import { ABTestAlgorithmData } from '../../../types/api'
import { ABTestFormValues } from './types'
import { toABTestCreatePayload } from './payload'
import { splitEndpoints } from './arms'

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

  if (values.experimentType === 'sr_config_tuning') {
    const v = values.variantSrConfig
    if (v.hedgingPercent !== null && (v.hedgingPercent < 0 || v.hedgingPercent > 100))
      return 'Hedging % must be between 0 and 100'
    if (v.eliminationThreshold !== null && (v.eliminationThreshold < 0 || v.eliminationThreshold > 1))
      return 'Elimination threshold must be between 0 and 1'
    if (v.hedgingPercent === null && v.eliminationThreshold === null)
      return 'Set at least one parameter override for the variant arm'
  } else {
    // algorithm_comparison — each arm needs at least one layer.
    if (!values.control.ruleAlgorithmId && !values.control.srStrategy)
      return 'Pick a routing rule, an SR strategy, or both for the control arm'
    if (!values.variant.ruleAlgorithmId && !values.variant.srStrategy)
      return 'Pick a routing rule, an SR strategy, or both for the variant arm'
  }

  if (values.endpoints.length === 0) return 'Select at least one endpoint'
  // Same check the backend runs: some selected endpoint must see different layers per arm, or
  // the experiment would split traffic between two identical setups.
  const data = toABTestCreatePayload(values, '').algorithm.data as ABTestAlgorithmData
  if (splitEndpoints(data).length === 0)
    return 'Control and variant apply the same layers on every selected endpoint'

  if (values.variantSplitPct < 5 || values.variantSplitPct > 30)
    return 'Variant traffic must be between 5% and 30%'
  return validateEvaluationSettings(values)
}
