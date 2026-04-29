export interface VolumeSplitGatewayFormEntry {
  id: string
  gatewayName: string
  gatewayId: string
  split: number
}

export interface VolumeSplitRuleFormValues {
  ruleName: string
  gateways: VolumeSplitGatewayFormEntry[]
}

export interface VolumeSplitGatewayOutput {
  gateway_name: string
  gateway_id: string | null
}

export interface VolumeSplitAlgorithmItem {
  split: number
  output: VolumeSplitGatewayOutput
}

export interface VolumeSplitAlgorithmData {
  type: 'volume_split'
  data: VolumeSplitAlgorithmItem[]
}

export interface VolumeSplitRuleCreatePayload {
  name: string
  description: string
  created_by: string
  algorithm_for: 'payment'
  metadata: Record<string, unknown>
  algorithm: VolumeSplitAlgorithmData
}

export interface VolumeSplitRuleDetailsState {
  id: string
  name: string
  description: string
  createdAt?: string
  gateways: VolumeSplitGatewayFormEntry[]
}
