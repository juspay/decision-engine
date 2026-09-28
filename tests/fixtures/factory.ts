/**
 * Test-data factory — the single source of truth for request payloads across the suite.
 *
 * The builders live in `test-data-factory.js`: plain CommonJS, no runner dependency, so they
 * stay readable as data. This file is the typed face of that module — the shape is asserted
 * once here rather than repeated at every call site.
 */
// @ts-ignore - pure JS CommonJS module without type declarations
import factory from './test-data-factory.js'

export default factory as {
  CONNECTORS: Record<string, { gateway_name: string; gateway_id: string }>
  merchantId: (suite?: string) => string
  paymentId: (prefix?: string) => string
  customerId: (prefix?: string) => string
  ruleName: (prefix?: string) => string
  gatewayConnector: (name: string, gatewayId?: string | null) => { gateway_name: string; gateway_id: string | null }
  connectorNames: (...names: string[]) => string[]
  srConfigData: (overrides?: Record<string, unknown>) => any
  eliminationConfigData: (overrides?: Record<string, unknown>) => any
  debitRoutingConfigData: (overrides?: Record<string, unknown>) => any
  paymentInfo: (overrides?: Record<string, unknown>) => any
  srDecideGatewayRequest: (overrides?: Record<string, unknown>) => any
  updateGatewayScoreRequest: (overrides?: Record<string, unknown>) => any
  singleRoutingPayload: (createdBy: string, overrides?: Record<string, unknown>) => any
  priorityRoutingPayload: (createdBy: string, overrides?: Record<string, unknown>) => any
  advancedRoutingPayload: (createdBy: string, overrides?: Record<string, unknown>) => any
  advancedNestedAndOrRoutingPayload: (createdBy: string, overrides?: Record<string, unknown>) => any
  volumeSplitRoutingPayload: (createdBy: string, overrides?: Record<string, unknown>) => any
  ruleEvaluatePayload: (createdBy: string, parameters?: Record<string, unknown>, overrides?: Record<string, unknown>) => any
}
