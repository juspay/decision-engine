// Mock data for Analytics page — used when API is unavailable
// Toggle USE_MOCK_DATA to switch between mock and real API calls

// Set to false to use real API endpoints instead of mock data
export const USE_MOCK_DATA = true

// ─── Types ──────────────────────────────────────────────────────────────────

export interface GatewayScore {
  gateway: string
  sr_score: number
  elimination_score: number
  latency_score: number
  decisions: number
  feedbacks: number
  last_updated: string
}

export interface TimeSeriesPoint {
  timestamp: string
  [key: string]: string | number
}

export interface DecisionSeries {
  timestamp: string
  SR_SELECTION_V3_ROUTING: number
  PRIORITY_LOGIC: number
  NTW_BASED_ROUTING: number
  DEFAULT: number
}

export interface GatewaySharePoint {
  timestamp: string
  [gateway: string]: string | number
}

export interface FeedbackDecisionPoint {
  timestamp: string
  decisions: number
  feedbacks: number
}

export interface PriorityRule {
  rule_name: string
  hits: number
  last_hit: string
  gateway: string
}

export interface FeedbackError {
  id: string
  timestamp: string
  error_type: string
  message: string
  gateway: string
}

export interface AnalyticsKPI {
  decisions_per_sec: number
  decisions_5m: number
  decisions_1h: number
  decisions_24h: number
  feedbacks_per_sec: number
  avg_sr: number
  error_rate: number
  sparkline: number[]
}

// ─── Helpers ────────────────────────────────────────────────────────────────

const GATEWAYS = ['stripe', 'adyen', 'braintree', 'checkout_com', 'razorpay', 'worldpay']
function ts(minutesAgo: number): string {
  return new Date(Date.now() - minutesAgo * 60_000).toISOString()
}

function rand(min: number, max: number): number {
  return Math.round((Math.random() * (max - min) + min) * 100) / 100
}

function genSparkline(len = 20, base = 50, variance = 15): number[] {
  return Array.from({ length: len }, () => Math.round(base + (Math.random() - 0.5) * variance * 2))
}

// ─── Mock generators ────────────────────────────────────────────────────────

export function mockGatewayScores(): GatewayScore[] {
  return GATEWAYS.map((gw) => ({
    gateway: gw,
    sr_score: rand(0.6, 0.98),
    elimination_score: rand(0, 0.3),
    latency_score: rand(0.5, 1),
    decisions: Math.round(rand(200, 5000)),
    feedbacks: Math.round(rand(100, 4000)),
    last_updated: ts(rand(0, 5)),
  }))
}

export function mockSRTimeSeries(points = 30): TimeSeriesPoint[] {
  return Array.from({ length: points }, (_, i) => {
    const row: TimeSeriesPoint = { timestamp: ts(points - i) }
    GATEWAYS.forEach((gw) => {
      row[gw] = rand(0.55, 0.98)
    })
    return row
  })
}

export function mockDecisionsByApproach(points = 30): DecisionSeries[] {
  return Array.from({ length: points }, (_, i) => ({
    timestamp: ts(points - i),
    SR_SELECTION_V3_ROUTING: Math.round(rand(40, 120)),
    PRIORITY_LOGIC: Math.round(rand(10, 50)),
    NTW_BASED_ROUTING: Math.round(rand(5, 30)),
    DEFAULT: Math.round(rand(2, 15)),
  }))
}

export function mockGatewayShare(points = 30): GatewaySharePoint[] {
  return Array.from({ length: points }, (_, i) => {
    const vals = GATEWAYS.map(() => rand(5, 40))
    const total = vals.reduce((a, b) => a + b, 0)
    const row: GatewaySharePoint = { timestamp: ts(points - i) }
    GATEWAYS.forEach((gw, j) => {
      row[gw] = Math.round((vals[j] / total) * 10000) / 100
    })
    return row
  })
}

export function mockFeedbackDecisions(points = 30): FeedbackDecisionPoint[] {
  return Array.from({ length: points }, (_, i) => ({
    timestamp: ts(points - i),
    decisions: Math.round(rand(80, 200)),
    feedbacks: Math.round(rand(40, 160)),
  }))
}

export function mockKPI(): AnalyticsKPI {
  return {
    decisions_per_sec: rand(12, 45),
    decisions_5m: Math.round(rand(3000, 8000)),
    decisions_1h: Math.round(rand(30000, 90000)),
    decisions_24h: Math.round(rand(500000, 2000000)),
    feedbacks_per_sec: rand(8, 35),
    avg_sr: rand(0.72, 0.95),
    error_rate: rand(0.1, 3.5),
    sparkline: genSparkline(),
  }
}

export function mockPriorityRules(): PriorityRule[] {
  return [
    { rule_name: 'high_value_card_stripe', hits: 1247, last_hit: ts(0.5), gateway: 'stripe' },
    { rule_name: 'upi_razorpay_preferred', hits: 983, last_hit: ts(1), gateway: 'razorpay' },
    { rule_name: 'eu_adyen_fallback', hits: 712, last_hit: ts(2), gateway: 'adyen' },
    { rule_name: 'wallet_checkout_com', hits: 456, last_hit: ts(3), gateway: 'checkout_com' },
    { rule_name: 'low_amount_braintree', hits: 321, last_hit: ts(5), gateway: 'braintree' },
    { rule_name: 'recurring_worldpay', hits: 198, last_hit: ts(8), gateway: 'worldpay' },
  ]
}

export function mockFeedbackErrors(): FeedbackError[] {
  return [
    { id: 'err_01', timestamp: ts(2), error_type: 'PARSE_ERROR', message: 'Invalid JSON in feedback payload from stripe webhook', gateway: 'stripe' },
    { id: 'err_02', timestamp: ts(5), error_type: 'DEAD_LETTER', message: 'Feedback for unknown transaction ID txn_abc123', gateway: 'adyen' },
    { id: 'err_03', timestamp: ts(12), error_type: 'TIMEOUT', message: 'Gateway feedback callback timed out after 30s', gateway: 'braintree' },
    { id: 'err_04', timestamp: ts(18), error_type: 'PARSE_ERROR', message: 'Missing required field "status" in feedback', gateway: 'razorpay' },
    { id: 'err_05', timestamp: ts(25), error_type: 'DEAD_LETTER', message: 'Duplicate feedback for txn_def456, ignoring', gateway: 'checkout_com' },
  ]
}

export const GATEWAY_COLORS: Record<string, string> = {
  stripe: '#635bff',
  adyen: '#0abf53',
  braintree: '#4b8bbe',
  checkout_com: '#ff6b35',
  razorpay: '#2d8cff',
  worldpay: '#e91e63',
}

export const APPROACH_COLORS: Record<string, string> = {
  SR_SELECTION_V3_ROUTING: '#3b82f6',
  PRIORITY_LOGIC: '#8b5cf6',
  NTW_BASED_ROUTING: '#10b981',
  DEFAULT: '#6b7280',
}
