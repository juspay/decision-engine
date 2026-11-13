import { RoutingAlgorithm } from '../../../types/api'
import {
  VolumeSplitAlgorithmItem,
  VolumeSplitGatewayFormEntry,
  VolumeSplitRuleDetailsState,
} from './types'

export function toVolumeSplitGatewayFormEntries(
  items: VolumeSplitAlgorithmItem[]
): VolumeSplitGatewayFormEntry[] {
  return items.map((item, index) => ({
    id: `${item.output?.gateway_name || 'gateway'}-${index}`,
    gatewayName: item.output?.gateway_name || '',
    gatewayId: item.output?.gateway_id || '',
    split: item.split,
  }))
}

export function toVolumeSplitRuleDetailsState(
  rule: RoutingAlgorithm
): VolumeSplitRuleDetailsState | null {
  const algorithm = rule.algorithm_data || rule.algorithm
  if (!algorithm || algorithm.type !== 'volume_split') {
    return null
  }

  const items = (algorithm.data as VolumeSplitAlgorithmItem[]) || []

  return {
    id: rule.id,
    name: rule.name,
    description: rule.description,
    createdAt: rule.created_at,
    gateways: toVolumeSplitGatewayFormEntries(items),
  }
}
