// All types use snake_case to match Rust backend responses

export interface DecideGatewayResponse {
  decided_gateway: string
  routing_approach: string
  gateway_priority_map: Record<string, number>
  routing_dimension: string
  routing_dimension_level: string
  filter_wise_gateways: Record<string, string[]> | null
  reset_approach: string
  is_scheduled_outage: boolean
  latency: number
}

export interface GatewayConnector {
  gateway_name: string
  gateway_id: string | null
}

export interface VolumeSplitItem {
  split: number
  connectors: GatewayConnector[]
}

export type RoutingAlgorithmData =
  | GatewayConnector[]              // priority
  | VolumeSplitItem[]              // volume_split
  | GatewayConnector               // single
  | EuclidAlgorithmData            // advanced

export interface EuclidRule {
  name: string
  connectorSelection: EuclidOutput
  statements: EuclidStatement[]
}

export interface EuclidStatement {
  condition: EuclidCondition[]
}

export interface EuclidCondition {
  lhs: string
  comparison: string
  value: { type: string; value: string | number }
  metadata: Record<string, unknown>
}

export interface EuclidOutput {
  type: 'priority' | 'volume_split'
  data: GatewayConnector[] | VolumeSplitItem[]
}

export interface EuclidAlgorithmData {
  globals: Record<string, unknown>
  defaultSelection: EuclidOutput
  rules: EuclidRule[]
}

export interface RoutingAlgorithm {
  id: string
  name: string
  description: string
  created_by: string
  algorithm_for: string
  created_at?: string
  algorithm: {
    type: 'priority' | 'volume_split' | 'single' | 'advanced'
    data: RoutingAlgorithmData
  }
}

export interface CreateRoutingRequest {
  name: string
  description: string
  created_by: string
  algorithm_for: string
  algorithm: {
    type: string
    data: RoutingAlgorithmData
  }
}

export interface ActivateRoutingRequest {
  created_by: string
  routing_algorithm_id: string
}

export interface SRConfigData {
  defaultBucketSize: number
  defaultLatencyThreshold: number | null
  defaultHedgingPercent: number | null
  defaultLowerResetFactor: number | null
  defaultUpperResetFactor: number | null
  defaultGatewayExtraScore: number | null
  subLevelInputConfig: SubLevelConfig[] | null
}

export interface SubLevelConfig {
  paymentMethodType: string
  paymentMethod: string
  bucketSize: number
  hedgingPercent: number | null
  latencyThreshold: number | null
}

export interface EliminationData {
  threshold: number
  txnLatency: {
    gatewayLatency: number
  }
}

export interface RuleConfig {
  type: 'successRate' | 'elimination' | 'debitRouting'
  data?: SRConfigData | EliminationData | DebitRoutingData
}

export interface CreateRuleRequest {
  merchant_id: string
  config: RuleConfig
}

export interface DebitRoutingData {
  merchant_category_code: string
  acquirer_country: string
}

export interface CreateMerchantRequest {
  merchant_id: string
  gateway_success_rate_based_decider_input: null
}

export interface SRDimensionRequest {
  merchant_id: string
  paymentInfo: {
    fields: string[]
  }
}
