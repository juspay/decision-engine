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
  output: GatewayConnector
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
  modified_at?: string
  // Backend returns algorithm_data, not algorithm
  algorithm_data?: {
    type: 'priority' | 'volume_split' | 'single' | 'advanced'
    data: RoutingAlgorithmData
  }
  // For convenience, map algorithm_data to algorithm in the component
  algorithm?: {
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

export type AnalyticsScope = 'current' | 'all'
export type AnalyticsRange = '15m' | '1h' | '24h'

export interface AnalyticsQuery {
  merchant_id?: string
  scope?: AnalyticsScope
  range?: AnalyticsRange
  page?: number
  page_size?: number
  payment_method_type?: string
  payment_method?: string
  gateway?: string
}

export interface AnalyticsKpi {
  label: string
  value: string
  subtitle?: string | null
}

export interface GatewayScoreSnapshot {
  merchant_id: string
  payment_method_type: string
  payment_method: string
  gateway: string
  score_value: number
  sigma_factor: number
  average_latency: number
  tp99_latency: number
  transaction_count: number
  last_updated_ms: number
}

export interface GatewayScoreSeriesPoint {
  bucket_ms: number
  merchant_id: string
  payment_method_type: string
  payment_method: string
  gateway: string
  score_value: number
}

export interface AnalyticsOverviewResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  kpis: AnalyticsKpi[]
  top_scores: GatewayScoreSnapshot[]
  top_errors: AnalyticsErrorSummary[]
  top_rules: AnalyticsRuleHit[]
}

export interface AnalyticsGatewayScoresResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  range: AnalyticsRange
  snapshots: GatewayScoreSnapshot[]
  series: GatewayScoreSeriesPoint[]
}

export interface AnalyticsDecisionPoint {
  bucket_ms: number
  routing_approach: string
  count: number
}

export interface AnalyticsDecisionResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  range: AnalyticsRange
  tiles: AnalyticsKpi[]
  series: AnalyticsDecisionPoint[]
  approaches: AnalyticsRuleHit[]
}

export interface AnalyticsGatewaySharePoint {
  bucket_ms: number
  gateway: string
  count: number
}

export interface AnalyticsRoutingStatsResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  range: AnalyticsRange
  gateway_share: AnalyticsGatewaySharePoint[]
  top_rules: AnalyticsRuleHit[]
  sr_trend: GatewayScoreSeriesPoint[]
  available_filters: RoutingFilterOptions
}

export interface RoutingFilterOptions {
  payment_method_types: string[]
  payment_methods: string[]
  gateways: string[]
}

export interface AnalyticsErrorSummary {
  route: string
  error_code: string
  error_message: string
  count: number
  last_seen_ms: number
}

export interface AnalyticsLogSample {
  route: string
  merchant_id?: string | null
  payment_id?: string | null
  request_id?: string | null
  gateway?: string | null
  routing_approach?: string | null
  status?: string | null
  error_code?: string | null
  error_message?: string | null
  event_type?: string | null
  created_at_ms: number
}

export interface AnalyticsLogSummariesResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  range: AnalyticsRange
  total_errors: number
  errors: AnalyticsErrorSummary[]
  samples: AnalyticsLogSample[]
  page: number
  page_size: number
}

export interface AnalyticsRuleHit {
  rule_name: string
  count: number
}

export interface PaymentAuditSummary {
  lookup_key: string
  payment_id?: string | null
  request_id?: string | null
  merchant_id?: string | null
  first_seen_ms: number
  last_seen_ms: number
  event_count: number
  latest_status?: string | null
  latest_gateway?: string | null
  latest_stage?: string | null
  gateways: string[]
  routes: string[]
}

export interface PaymentAuditEvent {
  id: number
  event_type: string
  event_stage?: string | null
  route?: string | null
  merchant_id?: string | null
  payment_id?: string | null
  request_id?: string | null
  payment_method_type?: string | null
  payment_method?: string | null
  gateway?: string | null
  routing_approach?: string | null
  rule_name?: string | null
  status?: string | null
  error_code?: string | null
  error_message?: string | null
  score_value?: number | null
  sigma_factor?: number | null
  average_latency?: number | null
  tp99_latency?: number | null
  transaction_count?: number | null
  details?: string | null
  details_json?: Record<string, unknown> | null
  created_at_ms: number
}

export interface PaymentAuditResponse {
  generated_at_ms: number
  scope: AnalyticsScope
  merchant_id?: string | null
  range: AnalyticsRange
  payment_id?: string | null
  request_id?: string | null
  gateway?: string | null
  route?: string | null
  status?: string | null
  event_type?: string | null
  error_code?: string | null
  page: number
  page_size: number
  total_results: number
  results: PaymentAuditSummary[]
  timeline: PaymentAuditEvent[]
}
