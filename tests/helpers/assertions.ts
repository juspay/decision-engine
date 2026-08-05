import { expect } from '@playwright/test'

/**
 * Ports of the custom chai assertions in cypress/support/e2e.js, expressed as plain
 * functions over Playwright's `expect`. Each takes the response body object.
 */

function expectObject(obj: any, label = 'response') {
  expect(obj, `${label} should be an object`).toBeTruthy()
  expect(typeof obj, `${label} should be an object`).toBe('object')
}

export function expectValidMerchantCreateResponse(obj: any) {
  expectObject(obj, 'merchant create')
  expect(obj.message).toBe('Merchant account created successfully')
  expect(typeof obj.merchant_id).toBe('string')
}

export function expectValidMerchantGetResponse(obj: any) {
  expectObject(obj, 'merchant get')
  expect(typeof obj.merchant_id).toBe('string')
  expect(obj).toHaveProperty('gateway_success_rate_based_decider_input')
}

export function expectValidMerchantDeleteResponse(obj: any) {
  expectObject(obj, 'merchant delete')
  expect(obj.message).toBe('Merchant account deleted successfully')
  expect(typeof obj.merchant_id).toBe('string')
}

export function expectValidGatewayResponse(obj: any) {
  expectObject(obj, 'gateway decision')
  expect(typeof obj.decided_gateway).toBe('string')
  expect(obj.gateway_priority_map, 'gateway_priority_map should be present').toBeTruthy()
  expect(typeof obj.gateway_priority_map).toBe('object')
  expect(typeof obj.routing_approach).toBe('string')
}

export function expectValidScoreUpdate(obj: any) {
  expectObject(obj, 'score update')
  expect(obj.message).toBe('Gateway score updated successfully')
  expect(typeof obj.merchant_id).toBe('string')
  expect(typeof obj.gateway).toBe('string')
  expect(typeof obj.payment_id).toBe('string')
}

export function expectValidRuleConfigResponse(obj: any, expectedType?: string) {
  expectObject(obj, 'rule config')
  expect(typeof obj.merchant_id).toBe('string')
  expect(obj.config, 'config should be present').toBeTruthy()
  expect(typeof obj.config).toBe('object')
  if (expectedType) expect(obj.config.type).toBe(expectedType)
}

export function expectValidRoutingAlgorithmCreateResponse(obj: any) {
  expectObject(obj, 'routing algorithm create')
  expect(typeof obj.rule_id).toBe('string')
  expect(typeof obj.name).toBe('string')
}

export function expectValidRoutingAlgorithmList(obj: any) {
  expect(Array.isArray(obj), 'routing algorithm list should be an array').toBe(true)
  for (const item of obj) {
    expect(typeof item.id).toBe('string')
    expect(typeof item.name).toBe('string')
    expect(typeof item.created_by).toBe('string')
  }
}

export function expectValidAnalyticsOverview(obj: any) {
  expectObject(obj, 'analytics overview')
  expect(typeof obj.merchant_id).toBe('string')
  expect(Array.isArray(obj.kpis)).toBe(true)
  expect(Array.isArray(obj.route_hits)).toBe(true)
}

export function expectValidRoutingStats(obj: any) {
  expectObject(obj, 'routing stats')
  expect(typeof obj.merchant_id).toBe('string')
  expect(Array.isArray(obj.gateway_share)).toBe(true)
  expect(Array.isArray(obj.sr_trend)).toBe(true)
}

export function expectValidPaymentAudit(obj: any) {
  expectObject(obj, 'payment audit')
  expect(Array.isArray(obj.results)).toBe(true)
  expect(obj).toHaveProperty('page')
  expect(obj).toHaveProperty('page_size')
  expect(obj).toHaveProperty('total_results')
}
