// All types use snake_case to match Rust backend responses

export interface DecideGatewayResponse {
  decided_gateway: string
  fallback_gateways: string[]
  routing_approach: string
  gateway_priority_map: Record<string, number> | null
  routing_dimension: string | null
  routing_dimension_level: string | null
  filter_wise_gateways: Record<string, string[]> | null
  reset_approach: string
  is_scheduled_outage: boolean
  debit_routing_output?: DebitRoutingOutput | null
  multi_objective_info?: MultiObjectiveInfo | null
  latency: number | null
}

export type MultiObjectiveOutcome = 'COST_WON' | 'AUTH_WON'

// Which source priced a PSP's cost: our own ingested data, the config seed table, or live Hypersense.
export type CostSource = 'IN_HOUSE' | 'SEED' | 'HYPERSENSE'

export interface PspSummary {
  psp: string
  authRate: number
  costBps: number | null
  // Where costBps came from; null when the PSP had no cost data.
  costSource: CostSource | null
}

export interface MultiObjectiveInfo {
  outcome: MultiObjectiveOutcome
  reason: string
  srHead: PspSummary | null
  chosen: PspSummary | null
  costSavedBps: number | null
  /// Number of PSPs ranked on expected value (i.e. those that had cost data).
  qualifiedCount: number
  /// Merchant margin (fraction of ticket) the decider applied. Used to value the
  /// auth-rate a cost override risked and net it against the fee saved.
  margin: number
  /// EV gap between the top-two EV-ranked PSPs (`EV(#1) − EV(#2)`, a fraction of
  /// ticket) — the margin of victory of the winning pick. Null when fewer than two
  /// PSPs had the cost data needed to rank on EV.
  evGapTop2?: number | null
}

export type RoutingAlgorithmName =
  | 'SR_BASED_ROUTING'
  | 'PL_BASED_ROUTING'
  | 'NTW_BASED_ROUTING'
  | 'NTW_SR_HYBRID_ROUTING'

export interface DebitRoutingNetworkSavingInfo {
  network: string
  saving_percentage: number
}

export interface DebitRoutingOutput {
  co_badged_card_networks_info: DebitRoutingNetworkSavingInfo[]
  issuer_country: string
  is_regulated: boolean
  regulated_name: string | null
  card_type: string
}

export interface GatewayConnector {
  gateway_name: string
  gateway_id: string | null
}

export interface VolumeSplitItem {
  split: number
  output: GatewayConnector
}

export interface SrConfigOverride {
  hedging_percent?: number
  elimination_threshold?: number
  enable_multi_objective?: boolean
  margin?: number
  use_autopilot?: boolean
}

export interface ABTestAlgorithmData {
  control_algorithm_id: string
  variant_algorithm_id: string
  variant_split_pct: number
  min_sample_size: number
  guardrail_threshold_pp: number
  variant_sr_config?: SrConfigOverride
  control_sr_config?: SrConfigOverride
}

export type RoutingAlgorithmData =
  | GatewayConnector[]              // priority
  | VolumeSplitItem[]              // volume_split
  | GatewayConnector               // single
  | EuclidAlgorithmData            // advanced
  | ABTestAlgorithmData            // ab_test

export interface EuclidRule {
  name: string
  routing_type?: 'priority' | 'volume_split' | 'volume_split_priority'
  output?: EuclidOutput
  statements: EuclidStatement[]
}

export interface EuclidStatement {
  condition: EuclidCondition[]
  nested?: EuclidStatement[]
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
  default_selection?: EuclidOutput
  defaultSelection?: EuclidOutput
  rules: EuclidRule[]
}

export interface RoutingAlgorithm {
  id: string
  rule_id?: string  // create endpoint returns rule_id; list endpoint returns id
  name: string
  description: string
  created_by: string
  algorithm_for: string
  created_at?: string
  modified_at?: string
  // Backend returns algorithm_data, not algorithm
  algorithm_data?: {
    type: 'priority' | 'volume_split' | 'single' | 'advanced' | 'ab_test'
    data: RoutingAlgorithmData
  }
  // For convenience, map algorithm_data to algorithm in the component
  algorithm?: {
    type: 'priority' | 'volume_split' | 'single' | 'advanced' | 'ab_test'
    data: RoutingAlgorithmData
  }
}

export type ExperimentVerdict =
  | 'collecting_data'
  | 'not_significant'
  | 'variant_wins'
  | 'variant_loses'
  | 'guardrail_breached'

export interface ExperimentArmMetrics {
  arm: string
  transaction_count: number
  success_count: number
  failure_count: number
  auth_rate: number
  avg_latency_ms: number | null
  avg_chosen_cost_bps: number | null
  avg_cost_saved_bps: number | null
  net_ev_bps: number | null
}

export interface ExperimentResultsResponse {
  experiment_id: string
  merchant_id: string
  control: ExperimentArmMetrics
  variant: ExperimentArmMetrics
  delta_pp: number
  p_value: number | null
  confidence_interval: [number, number] | null
  verdict: ExperimentVerdict
  min_sample_size: number
  net_delta_bps: number | null
  evaluation_margin: number
}

export interface ExperimentTransaction {
  payment_id: string
  variant_arm: string
  gateway: string | null
  status: string | null
  created_at_ms: number
}

export interface ExperimentTransactionsResponse {
  experiment_id: string
  total: number
  transactions: ExperimentTransaction[]
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

export interface DeactivateRoutingRequest {
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
  margin: number | null
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

export interface DebitRoutingFlagRequest {
  enabled: boolean
}

export interface DebitRoutingFlagResponse {
  merchant_id: string
  debit_routing_enabled: boolean
}

export interface MerchantFeatureEntry {
  feature: string
  enabled: boolean
}

export interface MerchantFeaturesResponse {
  merchant_id: string
  features: MerchantFeatureEntry[]
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

export type AnalyticsRange = '15m' | '1h' | '12h' | '1d' | '1w'
export type AnalyticsRangeValue = AnalyticsRange | 'custom'

export interface AnalyticsQuery {
  range?: AnalyticsRange
  start_ms?: number
  end_ms?: number
  page?: number
  page_size?: number
  payment_method_type?: string
  payment_method?: string
  card_network?: string
  card_is_in?: string
  currency?: string
  country?: string
  auth_type?: string
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

export interface SmartRetryTrigger {
  gateway: string
  error_code: string | null
  count: number
}

export interface SmartRetryFallback {
  gateway: string
  retried: number
  recovered: number
}

export interface SmartRetryStats {
  retried_count: number
  recovered_count: number
  by_trigger: SmartRetryTrigger[]
  by_fallback: SmartRetryFallback[]
}

export interface AnalyticsOverviewResponse {
  merchant_id: string
  kpis: AnalyticsKpi[]
  route_hits: AnalyticsRouteHit[]
  top_scores: GatewayScoreSnapshot[]
  top_errors: AnalyticsErrorSummary[]
  top_rules: AnalyticsRuleHit[]
  smart_retry_stats: SmartRetryStats
}

export interface AnalyticsRouteHit {
  route: string
  count: number
}

export interface AnalyticsGatewayScoresResponse {
  merchant_id: string
  range: AnalyticsRangeValue
  snapshots: GatewayScoreSnapshot[]
  series: GatewayScoreSeriesPoint[]
}

export interface AnalyticsDecisionPoint {
  bucket_ms: number
  routing_approach: string
  count: number
}

export interface AnalyticsDecisionResponse {
  merchant_id: string
  range: AnalyticsRangeValue
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
  merchant_id: string
  range: AnalyticsRangeValue
  gateway_share: AnalyticsGatewaySharePoint[]
  top_rules: AnalyticsRuleHit[]
  sr_trend: GatewayScoreSeriesPoint[]
  available_filters: RoutingFilterOptions
}

export interface AnalyticsAvailableCurrency {
  currency: string
  decision_count: number
}

export interface AnalyticsCostSavingsTrendPoint {
  bucket_ms: number
  saved_value: number
}

export interface AnalyticsCostSavingsTotals {
  saved_value: number
  cost_won_count: number
  total_decisions: number
}

export interface AnalyticsCostSavingsResponse {
  merchant_id: string
  range: AnalyticsRangeValue
  currency: string | null
  available_currencies: AnalyticsAvailableCurrency[]
  trend: AnalyticsCostSavingsTrendPoint[]
  totals: AnalyticsCostSavingsTotals
}

export interface RoutingFilterOptions {
  dimensions: RoutingFilterDimension[]
  missing_dimensions: RoutingFilterDimensionHint[]
  gateways: string[]
}

export interface RoutingFilterDimension {
  key: string
  label: string
  values: string[]
}

export interface RoutingFilterDimensionHint {
  key: string
  label: string
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
  global_request_id?: string | null
  trace_id?: string | null
  gateway?: string | null
  routing_approach?: string | null
  status?: string | null
  error_code?: string | null
  error_message?: string | null
  flow_type?: string | null
  created_at_ms: number
}

export interface AnalyticsLogSummariesResponse {
  merchant_id: string
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
  id: string
  flow_type: string
  event_stage?: string | null
  route?: string | null
  merchant_id?: string | null
  payment_id?: string | null
  request_id?: string | null
  global_request_id?: string | null
  trace_id?: string | null
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

export interface GsmInfo {
  decision: string
  stepUpPossible: boolean
  clearPanPossible: boolean
  alternateNetworkPossible: boolean
  unifiedCode: string | null
  unifiedMessage: string | null
  errorCategory: string | null
  standardisedCode: string | null
  description: string | null
  userGuidanceMessage: string | null
}

export interface UpdateScoreResponse {
  message: string
  merchant_id: string
  gateway: string
  payment_id: string
  gsm_info: GsmInfo | null
}

export type RoutingEventType =
  | 'leader_changed'
  | 'gateway_entered_auth_band'
  | 'gateway_exited_auth_band'
  | 'calibration_applied'

export interface RoutingEvent {
  id: string
  event_type: RoutingEventType
  merchant_id: string
  payment_method_type: string | null
  payment_method: string | null
  bucket_ms: number
  gateway: string
  previous_gateway: string | null
  score: number | null
  previous_score: number | null
  transaction_count: number | null
  // Present only on `calibration_applied` events: the autopilot's new/previous knobs
  // and the full cluster grain the retune applied to.
  bucket_size?: number | null
  previous_bucket_size?: number | null
  hedging_percent?: number | null
  previous_hedging_percent?: number | null
  card_network?: string | null
  currency?: string | null
  country?: string | null
  auth_type?: string | null
}

export interface RoutingEventsResponse {
  merchant_id: string
  range: AnalyticsRangeValue
  events: RoutingEvent[]
  generated_at_ms: number
}

export interface PaymentAuditResponse {
  merchant_id: string
  range: AnalyticsRangeValue
  payment_id?: string | null
  request_id?: string | null
  gateway?: string | null
  route?: string | null
  status?: string | null
  flow_type?: string | null
  routing_approach?: string | null
  error_code?: string | null
  page: number
  page_size: number
  total_results: number
  total_success: number
  total_failure: number
  results: PaymentAuditSummary[]
  timeline: PaymentAuditEvent[]
}
