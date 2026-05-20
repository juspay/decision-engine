import { ABTestFormValues } from './types'

export function validateABTestForm(values: ABTestFormValues): string | null {
  if (!values.name.trim()) return 'Enter an experiment name'
  if (!values.controlAlgorithmId) return 'Select a control routing algorithm'
  if (!values.variantAlgorithmId) return 'Select a variant routing algorithm'
  if (values.controlAlgorithmId === values.variantAlgorithmId)
    return 'Control and variant must be different algorithms'
  if (values.variantSplitPct < 5 || values.variantSplitPct > 30)
    return 'Variant traffic must be between 5% and 30%'
  if (values.minSampleSize < 100)
    return 'Minimum sample size must be at least 100 transactions'
  if (values.guardrailThresholdPp <= 0 || values.guardrailThresholdPp > 20)
    return 'Guardrail threshold must be between 0.1 and 20 percentage points'
  return null
}
