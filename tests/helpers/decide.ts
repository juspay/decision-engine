import { expect } from '@playwright/test'
import factory from '../fixtures/factory'
import type { ApiClient } from '../fixtures/api-client'
import decideContract from '../fixtures/contracts/decide-request.json'

/**
 * `/decide` requests for specs that route payments. `/decide` replaces `/routing/hybrid`: it runs the
 * merchant's rule and, where enabled, success-rate routing, and an A/B experiment's
 * `hybrid_routing` split.
 */

/** The connectors the specs' rules and experiment arms route to. */
export const DECIDE_CONNECTORS = ['stripe', 'checkout', 'adyen', 'razorpay']

/** A `/decide` body for `paymentId`, the contract's credit-card payment, routable to `connectors`. */
export function decideBody(paymentId: string, connectors: string[] = DECIDE_CONNECTORS) {
  return {
    ...structuredClone(decideContract),
    payment_id: paymentId,
    routing_config: {
      ...structuredClone(decideContract.routing_config),
      eligible_connectors: connectors.map((name) => {
        const { gateway_name, gateway_id } = factory.gatewayConnector(name)
        return { connector: gateway_name, merchant_connector_id: gateway_id! }
      }),
    },
  }
}

/**
 * Routes `paymentId` through `/decide` on `baseUrl` (the suite's server when empty) and answers with
 * the response body.
 */
export async function decide(api: ApiClient, paymentId: string, connectors: string[] = DECIDE_CONNECTORS, baseUrl = '') {
  const r = await api.raw('POST', `${baseUrl}/decide`, { body: decideBody(paymentId, connectors) })
  expect(r.status, JSON.stringify(r.body).slice(0, 500)).toBe(200)
  return r.body
}

/** The connector `/decide` chose for `paymentId`. */
export async function decidedConnector(api: ApiClient, paymentId: string, connectors: string[] = DECIDE_CONNECTORS, baseUrl = ''): Promise<string> {
  return (await decide(api, paymentId, connectors, baseUrl)).connectors[0].connector
}
