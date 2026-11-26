import { test, expect, factory, poll } from '../../fixtures/test'
import contract from '../../fixtures/contracts/decide-request.json'
import type { ApiClient } from '../../fixtures/api-client'
import type { DecideRequest, DecideResponse } from '../../../website/src/types/api'

/**
 * `/update-gateway-score` with `attemptedConnector`: the connector account the payment was
 * attempted on, sent back in the shape `/decide` returned it. Such an update is scored only for a
 * connector `/decide` offered the payment; an update that sends only `gateway` behaves as before.
 */

type Connector = { connector: string; merchant_connector_id: string | null }

const stripe: Connector = { connector: 'stripe', merchant_connector_id: 'mca_stripe' }
const adyen: Connector = { connector: 'adyen', merchant_connector_id: 'mca_adyen' }
const checkout: Connector = { connector: 'checkout', merchant_connector_id: 'mca_checkout' }
const otherStripeAccount: Connector = { connector: 'stripe', merchant_connector_id: 'mca_stripe_other' }

const identifierOf = (c: { connector: string; merchant_connector_id?: string | null }) => (c.merchant_connector_id ? `${c.connector}:${c.merchant_connector_id}` : c.connector)

function payment(eligible: Connector[], paymentId = factory.paymentId('attempt')): DecideRequest {
  const body: DecideRequest = { ...structuredClone(contract), payment_id: paymentId }
  body.routing_config.eligible_connectors = eligible
  return body
}

test.describe('Feedback with attemptedConnector (API)', () => {
  test.beforeEach(async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0, defaultBucketSize: 10, subLevelInputConfig: [] })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
  })

  const decide = async (api: ApiClient, body: DecideRequest) => {
    const response = await api.raw<DecideResponse>('POST', '/decide', { body })
    expect(response.status).toBe(200)
    expect(response.body.success_rate?.status).toBe('applied')
    return response.body
  }

  const feedback = (api: ApiClient, merchantId: string, paymentId: string, fields: Record<string, unknown>, status = 'FAILURE') =>
    api.raw('POST', '/update-gateway-score', { failOnStatusCode: false, body: {
      merchantId, paymentId, status, gatewayReferenceId: null, enforceDynamicRoutingFailure: null, ...fields,
    } })

  test('scores the connector it names, copied from the /decide response', async ({ api, merchant }) => {
    // Each failure names the connector exactly as /decide returned it.
    for (let i = 0; i < 3; i++) {
      const body = payment([stripe, adyen])
      const decision = await decide(api, body)
      const attempted = decision.connectors.find(c => c.connector === 'stripe')!
      const response = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: attempted })
      expect(response.status).toBe(200)
      expect(response.body.gateway).toBe(identifierOf(stripe))
    }
    // The score update is asynchronous.
    await poll(() => api.raw<DecideResponse>('POST', '/decide', { body: payment([stripe, adyen]) }), response => {
      const scores = Object.fromEntries(response.body.success_rate!.scores.map(s => [identifierOf(s), s.score]))
      return scores[identifierOf(stripe)] < scores[identifierOf(adyen)]
    }, { message: 'stripe never scored below adyen after failures reported with attemptedConnector' })
  })

  test('accepts any connector the decision offered, not only the first', async ({ api, merchant }) => {
    const body = payment([stripe, adyen])
    await decide(api, body)
    for (const attempted of [stripe, adyen]) {
      const response = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: attempted })
      expect(response.status, identifierOf(attempted)).toBe(200)
    }
  })

  test('refuses another account, a bare name, or a connector that was not offered', async ({ api, merchant }) => {
    const body = payment([stripe, adyen])
    await decide(api, body)
    for (const attempted of [otherStripeAccount, { connector: 'stripe', merchant_connector_id: null }, checkout]) {
      const response = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: attempted })
      expect(response.status, identifierOf(attempted)).toBe(422)
      expect(response.body.error_code).toBe('CONNECTOR_NOT_OFFERED')
      // The answer lists what was offered, so the caller can see the mismatch.
      expect(response.body.error_message).toContain(identifierOf(stripe))
      expect(response.body.error_message).toContain(identifierOf(adyen))
    }
  })

  test('refuses a payment /decide has no decision for', async ({ api, merchant }) => {
    const response = await feedback(api, merchant.id, factory.paymentId('never_decided'), { attemptedConnector: stripe })
    expect(response.status).toBe(404)
    expect(response.body.error_code).toBe('DECISION_NOT_FOUND')
  })

  test('checks against the payment\'s latest decision', async ({ api, merchant }) => {
    const paymentId = factory.paymentId('redecided')
    await decide(api, payment([stripe, adyen], paymentId))
    await decide(api, payment([checkout], paymentId))
    expect((await feedback(api, merchant.id, paymentId, { attemptedConnector: checkout })).status).toBe(200)
    expect((await feedback(api, merchant.id, paymentId, { attemptedConnector: stripe })).status).toBe(422)
  })

  test('takes gateway alongside it only when both name the same connector', async ({ api, merchant }) => {
    const body = payment([stripe, adyen])
    await decide(api, body)
    const same = await feedback(api, merchant.id, body.payment_id, { gateway: identifierOf(stripe), attemptedConnector: stripe })
    expect(same.status).toBe(200)
    for (const gateway of ['stripe', identifierOf(adyen)]) {
      const response = await feedback(api, merchant.id, body.payment_id, { gateway, attemptedConnector: stripe })
      expect(response.status, gateway).toBe(400)
      expect(response.body.error_code).toBe('CONNECTOR_MISMATCH')
      expect(response.body.error_message).toContain('name different connectors')
    }
  })

  test('rejects an attemptedConnector with unknown keys', async ({ api, merchant }) => {
    const body = payment([stripe, adyen])
    await decide(api, body)
    const response = await feedback(api, merchant.id, body.payment_id, {
      attemptedConnector: { connector: 'stripe', account_id: 'mca_stripe' },
    })
    expect(response.status).toBe(400)
  })

  test('leaves updates that send only gateway as they were', async ({ api, merchant }) => {
    const body = payment([stripe, adyen])
    await decide(api, body)
    // Neither is checked against the decision.
    for (const gateway of [identifierOf(otherStripeAccount), 'stripe', identifierOf(stripe)]) {
      const response = await feedback(api, merchant.id, body.payment_id, { gateway })
      expect(response.status, gateway).toBe(200)
      expect(response.body.gateway).toBe(gateway)
    }
    const unknownPayment = await feedback(api, merchant.id, factory.paymentId('never_decided'), { gateway: identifierOf(stripe) })
    expect(unknownPayment.status).toBe(200)
    const missing = await feedback(api, merchant.id, body.payment_id, {})
    expect(missing.status).toBe(400)
  })
})
