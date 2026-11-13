import { VolumeSplitAlgorithmData, VolumeSplitGatewayFormEntry, VolumeSplitRuleCreatePayload, VolumeSplitRuleFormValues } from './types'

/**
 * The `algorithm` body a volume-split rule is stored as. Shared by create and update so the two
 * paths cannot diverge in how they encode splits.
 */
export function toVolumeSplitAlgorithm(
  gateways: VolumeSplitGatewayFormEntry[]
): VolumeSplitAlgorithmData {
  return {
    type: 'volume_split',
    data: gateways.map((gateway) => ({
      split: gateway.split,
      output: {
        gateway_name: gateway.gatewayName.trim(),
        gateway_id: gateway.gatewayId.trim() || null,
      },
    })),
  }
}

export function toVolumeSplitCreatePayload(
  formValues: VolumeSplitRuleFormValues,
  merchantId: string
): VolumeSplitRuleCreatePayload {
  return {
    name: formValues.ruleName.trim(),
    description: formValues.description?.trim() ?? '',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: {},
    algorithm: toVolumeSplitAlgorithm(formValues.gateways),
  }
}
