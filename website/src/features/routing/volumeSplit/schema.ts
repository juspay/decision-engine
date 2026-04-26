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
