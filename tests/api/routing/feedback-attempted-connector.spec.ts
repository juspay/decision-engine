import { test, expect, factory, poll } from '../../fixtures/test'
import contract from '../../fixtures/contracts/decide-request.json'
import type { ApiClient } from '../../fixtures/api-client'
import { redisKeys } from '../../helpers/redis'
import type { DecideRequest, DecideResponse } from '../../../website/src/types/api'

/**
 * `/update-gateway-score` with `attemptedConnector`: the connector account the payment was
 * attempted on, sent back in the shape `/decide` returned it. When the payment has a stored decision,
 * such an update is scored only for a connector that decision offered. Without one there is nothing
 * to check or score, so it is answered as an update that sends only `gateway` would be. An update
 * that sends only `gateway` behaves as before.
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

const srConfig = { defaultHedgingPercent: 0, defaultBucketSize: 10, subLevelInputConfig: [] }

/** A `/decide` call; `sr` says whether the SR layer is expected to rank it. */
async function decide(api: ApiClient, body: DecideRequest, sr = true) {
  const response = await api.raw<DecideResponse>('POST', '/decide', { body })
  expect(response.status).toBe(200)
  expect(response.body.success_rate?.status ?? null).toBe(sr ? 'applied' : null)
  return response.body
}

const feedback = (api: ApiClient, merchantId: string, paymentId: string, fields: Record<string, unknown>, status = 'FAILURE') =>
  api.raw('POST', '/update-gateway-score', { failOnStatusCode: false, body: {
    merchantId, paymentId, status, gatewayReferenceId: null, enforceDynamicRoutingFailure: null, ...fields,
  } })

/** A failure the bundled GSM rules answer with `retry` for adyen. */
const retryableFailure = {
  flow: 'Payment', subFlow: 'Authorize', errorCode: '127', errorMessage: "Required field 'accountHolderName' is not provided",
  issuerErrorCode: null, cardNetwork: null,
}

test.describe('Feedback with attemptedConnector (API)', () => {
  test.beforeEach(async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, srConfig)
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
  })

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

  test('answers a payment with no stored decision as a gateway update would', async ({ api, merchant }) => {
    const paymentId = factory.paymentId('never_decided')
    const attempted = await feedback(api, merchant.id, paymentId, { attemptedConnector: stripe })
    const named = await feedback(api, merchant.id, paymentId, { gateway: identifierOf(stripe) })
    expect(attempted.status).toBe(200)
    expect(attempted.body).toEqual({ ...named.body, gateway: identifierOf(stripe) })
  })

  test('suggests retry connectors and scores the retry, both by attemptedConnector', async ({ api, merchant }) => {
    const body = payment([checkout, stripe, adyen])
    await decide(api, body)
    const failed = await feedback(api, merchant.id, body.payment_id, {
      attemptedConnector: adyen, errorInfo: retryableFailure,
    }, 'AUTHORIZATION_FAILED')
    expect(failed.status).toBe(200)
    expect(failed.body.gsm_info?.decision).toBe('retry')
    // The other connectors the decision offered, never the one that failed.
    expect([...failed.body.gsm_info.retryConnectors].sort()).toEqual([identifierOf(checkout), identifierOf(stripe)])

    const retried = await feedback(api, merchant.id, body.payment_id, {
      attemptedConnector: stripe, isSmartRetry: true,
    }, 'CHARGED')
    expect(retried.status).toBe(200)
    expect(retried.body.gateway).toBe(identifierOf(stripe))
  })

  test('suggests the same retry connectors for a gateway update', async ({ api, merchant }) => {
    const body = payment([checkout, stripe, adyen])
    await decide(api, body)
    const failed = await feedback(api, merchant.id, body.payment_id, {
      gateway: identifierOf(adyen), errorInfo: retryableFailure,
    }, 'AUTHORIZATION_FAILED')
    expect(failed.status).toBe(200)
    expect([...failed.body.gsm_info.retryConnectors].sort()).toEqual([identifierOf(checkout), identifierOf(stripe)])
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

test.describe('Feedback with attemptedConnector, without a stored decision (API)', () => {
  test('a payment routed by its rule alone is answered, since nothing was stored to check', async ({ api, merchant }) => {
    // SR off and rule-routed scoring off: /decide stores no scoring context.
    const created = await api.createRoutingAlgorithm(factory.priorityRoutingPayload(merchant.id, {
      connectors: [factory.gatewayConnector('checkout'), factory.gatewayConnector('stripe')],
    }))
    await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
    const body = payment([stripe, adyen, checkout])
    const decision = await decide(api, body, false)
    expect(decision.connectors).toEqual([checkout, stripe])
    for (const attempted of [decision.connectors[0], adyen]) {
      const response = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: attempted })
      expect(response.status, identifierOf(attempted)).toBe(200)
    }
  })

  test('a payment routed by its rule with rule-routed scoring on is checked against the rule\'s connectors', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, srConfig)
    await api.setMerchantFeature(merchant.id, 'sr-scores-from-rule-routing', true)
    const created = await api.createRoutingAlgorithm(factory.priorityRoutingPayload(merchant.id, {
      connectors: [factory.gatewayConnector('checkout'), factory.gatewayConnector('stripe')],
    }))
    await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
    const body = payment([stripe, adyen, checkout])
    const decision = await decide(api, body, false)
    expect(decision.connectors).toEqual([checkout, stripe])
    expect((await feedback(api, merchant.id, body.payment_id, { attemptedConnector: checkout })).status).toBe(200)
    // A candidate the rule left out was not offered.
    const excluded = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: adyen })
    expect(excluded.status).toBe(422)
    expect(excluded.body.error_code).toBe('CONNECTOR_NOT_OFFERED')
  })

  test('a redelivered update after the payment\'s context expired is answered, not refused', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, srConfig)
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    const ttl = await api.raw('PUT', `/admin/merchant-account/${merchant.id}/scoring-data-ttl`, { body: { ttl_secs: 2 } })
    expect(ttl.status, JSON.stringify(ttl.body)).toBe(200)
    const body = payment([stripe, adyen])
    await decide(api, body)
    const first = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: stripe }, 'CHARGED')
    expect(first.status).toBe(200)
    const key = `gateway_scoring_data_${merchant.id}_${body.payment_id}`
    await expect.poll(() => redisKeys(key), { timeout: 10_000, intervals: [500] }).toEqual([])
    const redelivered = await feedback(api, merchant.id, body.payment_id, { attemptedConnector: stripe }, 'CHARGED')
    expect(redelivered.status).toBe(200)
    expect(redelivered.body.gateway).toBe(identifierOf(stripe))
  })
})
