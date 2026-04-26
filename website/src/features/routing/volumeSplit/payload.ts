import { VolumeSplitRuleCreatePayload, VolumeSplitRuleFormValues } from './types'

export function toVolumeSplitCreatePayload(
  formValues: VolumeSplitRuleFormValues,
  merchantId: string
): VolumeSplitRuleCreatePayload {
  return {
    rule_id: null,
    name: formValues.ruleName.trim(),
    description: '',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: null,
    algorithm: {
      type: 'volume_split',
      data: formValues.gateways.map((gateway) => ({
        split: gateway.split,
        output: {
          gateway_name: gateway.gatewayName.trim(),
          gateway_id: gateway.gatewayId.trim() || null,
        },
      })),
    },
  }
}
