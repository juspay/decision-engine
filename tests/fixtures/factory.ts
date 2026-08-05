/**
 * Test-data factory.
 *
 * The pure, runner-agnostic payload/data builders currently live alongside the (now frozen)
 * Cypress suite in `cypress/support/test-data-factory.js`. That file contains ZERO `cy.*` calls —
 * it is plain CommonJS — so we re-export it verbatim to keep a SINGLE source of truth for request
 * payloads across both the Playwright and Cypress suites during the transition.
 *
 * Do not fork this. If a builder needs to change, change it in the shared file.
 */
// @ts-ignore - pure JS CommonJS module without type declarations
import factory from '../../cypress/support/test-data-factory.js'

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
