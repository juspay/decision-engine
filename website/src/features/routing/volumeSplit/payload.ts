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

/**
 * Metadata for a saved rule: the source rule's metadata (so a fork or edit never drops keys
 * written elsewhere) with sticky_routing set from the toggle.
 */
export function toRuleMetadata(
  stickyRouting: boolean,
  baseMetadata?: Record<string, unknown> | null
): Record<string, unknown> {
  return { ...(baseMetadata ?? {}), sticky_routing: { enabled: stickyRouting } }
}

export function toVolumeSplitCreatePayload(
  formValues: VolumeSplitRuleFormValues,
  merchantId: string,
  baseMetadata?: Record<string, unknown> | null
): VolumeSplitRuleCreatePayload {
  return {
    name: formValues.ruleName.trim(),
    description: formValues.description?.trim() ?? '',
    created_by: merchantId,
    algorithm_for: 'payment',
    metadata: toRuleMetadata(formValues.stickyRouting, baseMetadata),
    algorithm: toVolumeSplitAlgorithm(formValues.gateways),
  }
}
