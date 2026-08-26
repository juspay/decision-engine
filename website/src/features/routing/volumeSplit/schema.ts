import { z } from 'zod'
import { VolumeSplitRuleFormValues } from './types'

export const volumeSplitGatewaySchema = z.object({
  id: z.string().min(1),
  gatewayName: z.string().trim().min(1, 'Gateway name is required'),
  gatewayId: z.string().trim(),
  split: z.number().min(0).max(100),
})

export const volumeSplitRuleFormSchema = z.object({
  ruleName: z.string().trim().min(1, 'Enter a rule name'),
  gateways: z.array(volumeSplitGatewaySchema).min(1, 'Add at least one gateway'),
})

/** Every problem in the form, keyed to the control that owns it. */
export interface VolumeSplitFieldErrors {
  ruleName?: string
  /** Keyed by gateway row id. */
  gateways: Record<string, string>
  /** Splits that do not sum to 100; belongs with the running total, not with a single row. */
  total?: string
}

export function hasVolumeSplitErrors(errors: VolumeSplitFieldErrors) {
  return Boolean(errors.ruleName || errors.total || Object.keys(errors.gateways).length)
}

/**
 * The same rules as {@link validateVolumeSplitRule}, reported per field so each message can be
 * rendered against its own control instead of collapsing to whichever issue happened to be first.
 */
export function collectVolumeSplitErrors(values: VolumeSplitRuleFormValues): VolumeSplitFieldErrors {
  const errors: VolumeSplitFieldErrors = { gateways: {} }

  if (!values.ruleName.trim()) {
    errors.ruleName = 'Enter a name for this rule.'
  }
  if (!values.gateways.length) {
    errors.total = 'Add at least one gateway.'
    return errors
  }

  values.gateways.forEach((gateway) => {
    if (!gateway.gatewayName.trim()) {
      errors.gateways[gateway.id] = 'Choose a gateway.'
    }
  })

  const total = values.gateways.reduce((sum, gateway) => sum + gateway.split, 0)
  if (total !== 100) {
    errors.total = `Splits must sum to 100 (currently ${total}).`
  }

  return errors
}

export function validateVolumeSplitRule(values: VolumeSplitRuleFormValues): string | null {
  const parsed = volumeSplitRuleFormSchema.safeParse(values)
  if (!parsed.success) {
    return parsed.error.issues[0]?.message || 'Invalid volume split rule configuration'
  }

  const total = parsed.data.gateways.reduce((sum, gateway) => sum + gateway.split, 0)
  if (total !== 100) {
    return `Splits must sum to 100 (currently ${total})`
  }

  return null
}
