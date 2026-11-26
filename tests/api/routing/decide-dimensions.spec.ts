import { test, expect, factory, poll } from '../../fixtures/test'
import contract from '../../fixtures/contracts/decide-request.json'
import type { DecideRequest, DecideResponse } from '../../../website/src/types/api'

/**
 * Which request fields `/decide` routes on, end to end:
 *   - SR scores are kept per combination of the merchant's configured dimensions (plus the card
 *     type), so training one combination leaves every other one untouched, and a field outside the
 *     configured dimensions shares the trained scores.
 *   - Rules read the card fields exactly as the caller sends them: no BIN lookup fills in a missing
 *     network or card type, and a sent network is used even when the BIN belongs to another one.
 */

const stripe = { connector: 'stripe', merchant_connector_id: 'mca_stripe' }
const adyen = { connector: 'adyen', merchant_connector_id: 'mca_adyen' }
const checkout = { connector: 'checkout', merchant_connector_id: 'mca_checkout' }
const razorpay = { connector: 'razorpay', merchant_connector_id: 'mca_razorpay' }

type Card = NonNullable<DecideRequest['card']>

/** A card carrying only the given fields, so nothing from the contract card leaks into the case. */
const cardOf = (fields: Partial<Card>): Card => ({
  ...Object.fromEntries(Object.keys(contract.card).map(key => [key, null])),
  ...fields,
}) as Card

const VISA_CREDIT = cardOf({
  card_network: 'Visa', card_type: 'credit', card_isin: '001234', card_issuing_country_code: 'US',
})

function payment(eligible: Array<typeof stripe>, change: (body: DecideRequest) => void = () => {}): DecideRequest {
  const body: DecideRequest = {
    ...structuredClone(contract),
    payment_id: factory.paymentId('dims'),
    card: VISA_CREDIT,
  }
  body.routing_config.eligible_connectors = eligible
  change(body)
  return body
}

const identifierOf = (c: { connector: string; merchant_connector_id?: string | null }) =>
  c.merchant_connector_id ? `${c.connector}:${c.merchant_connector_id}` : c.connector

const scoresOf = (response: DecideResponse) =>
  Object.fromEntries(response.success_rate!.scores.map(s => [identifierOf(s), s.score]))

test.describe('SR scores per configured dimension (API)', () => {
  test('a configured field gets its own scores; any other field shares them', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id, { defaultHedgingPercent: 0, defaultBucketSize: 10, subLevelInputConfig: [] })
    await api.setMerchantFeature(merchant.id, 'sr-routing', true)
    await api.raw('POST', '/config-sr-dimension', {
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields: ['card_network', 'currency', 'country', 'auth_type'] } },
    })
    const base = (change?: (body: DecideRequest) => void) => payment([stripe, adyen], change)
    const decide = (body: DecideRequest) => api.raw<DecideResponse>('POST', '/decide', { body })

    // Train stripe down on Visa credit USD US: each failure follows a decision for the same payment,
    // so the feedback finds the payment's scoring context.
    for (let i = 0; i < 3; i++) {
      const body = base()
      expect((await decide(body)).body.success_rate?.status).toBe('applied')
      await api.raw('POST', '/update-gateway-score', { body: {
        merchantId: merchant.id, gateway: identifierOf(stripe), gatewayReferenceId: null,
        paymentId: body.payment_id, status: 'FAILURE', enforceDynamicRoutingFailure: null,
      } })
    }
    // The score update is asynchronous.
    await poll(() => decide(base()), r => {
      const scores = scoresOf(r.body)
      return scores[identifierOf(stripe)] < scores[identifierOf(adyen)]
    }, { message: 'stripe never scored below adyen on the trained combination' })

    const cases: Array<{ name: string; change: (body: DecideRequest) => void; shares: boolean; label?: string[] }> = [
      { name: 'the trained combination', change: () => {}, shares: true, label: ['CREDIT', 'CARD', 'Visa', 'USD', 'US'] },
      { name: 'card_network Mastercard', change: b => { b.card = { ...b.card!, card_network: 'Mastercard' } }, shares: false, label: ['Mastercard', 'USD', 'US'] },
      { name: 'currency EUR', change: b => { b.currency = 'EUR' }, shares: false, label: ['Visa', 'EUR', 'US'] },
      { name: 'billing_country DE', change: b => { b.billing_country = 'DE' }, shares: false, label: ['Visa', 'USD', 'DE'] },
      { name: 'authentication_type three_ds', change: b => { b.authentication_type = 'three_ds' }, shares: false, label: ['Visa', 'USD', 'US', 'THREE_DS'] },
      {
        name: 'card_type debit',
        change: b => { b.payment_method_sub_type = 'debit'; b.card = { ...b.card!, card_type: 'debit' } },
        shares: false,
        label: ['DEBIT', 'Visa'],
      },
      // Not configured as dimensions, so they score with the trained combination.
      { name: 'issuer country GB', change: b => { b.card = { ...b.card!, card_issuing_country_code: 'GB' } }, shares: true },
      { name: 'another BIN', change: b => { b.card = { ...b.card!, card_isin: '001299' } }, shares: true },
      { name: 'another amount', change: b => { b.amount = 250_000 }, shares: true },
    ]
    for (const { name, change, shares, label } of cases) {
      await test.step(name, async () => {
        const response = await decide(base(change))
        expect(response.body.success_rate?.status).toBe('applied')
        const scores = scoresOf(response.body)
        if (shares) {
          expect.soft(scores[identifierOf(stripe)], `${name}: shares the trained scores`).toBeLessThan(scores[identifierOf(adyen)])
        } else {
          expect.soft(scores[identifierOf(stripe)], `${name}: has its own, untrained scores`).toBe(scores[identifierOf(adyen)])
        }
        if (label) {
          const dimension = response.body.success_rate!.diagnostics?.dimension as { value: string | null } | undefined
          expect.soft(dimension?.value?.split(', '), `${name}: dimension`).toEqual(expect.arrayContaining(label))
        }
      })
    }
  })
})

test.describe('Rules read the card fields as sent (API)', () => {
  const condition = (lhs: string, type: string, value: string) => ({ lhs, comparison: 'equal', value: { type, value }, metadata: {} })

  test('matches on the sent network, card type and BIN only', async ({ api, merchant }) => {
    const created = await api.createRoutingAlgorithm(factory.advancedRoutingPayload(merchant.id, {
      default_selection: { priority: [factory.gatewayConnector('stripe')] },
      rules: [
        {
          name: 'Visa credit', routing_type: 'priority',
          output: { priority: [factory.gatewayConnector('checkout'), factory.gatewayConnector('adyen')] },
          statements: [{ condition: [condition('card_network', 'enum_variant', 'Visa'), condition('card_type', 'enum_variant', 'credit')] }],
        },
        {
          name: 'Mastercard debit', routing_type: 'priority', output: { priority: [factory.gatewayConnector('adyen')] },
          statements: [{ condition: [condition('card_network', 'enum_variant', 'Mastercard'), condition('card_type', 'enum_variant', 'debit')] }],
        },
        {
          name: 'BIN 411111', routing_type: 'priority', output: { priority: [factory.gatewayConnector('razorpay')] },
          statements: [{ condition: [condition('card_bin', 'str_value', '411111')] }],
        },
      ],
    }))
    await api.activateRoutingAlgorithm(merchant.id, created.body.rule_id)
    const eligible = [stripe, adyen, checkout, razorpay]

    const cases: Array<{ name: string; card: Card; subType?: string; routesTo: typeof stripe | null }> = [
      { name: 'Visa credit sent', card: cardOf({ card_network: 'Visa', card_type: 'credit', card_isin: '424242' }), routesTo: checkout },
      { name: 'a Visa BIN alone', card: cardOf({ card_isin: '424242' }), routesTo: null },
      { name: 'a Visa BIN with card_type, no network', card: cardOf({ card_isin: '424242', card_type: 'credit' }), routesTo: null },
      { name: 'Visa without card_type (sub-type credit)', card: cardOf({ card_network: 'Visa', card_isin: '424242' }), routesTo: null },
      { name: 'Visa credit sent on a Mastercard BIN', card: cardOf({ card_network: 'Visa', card_type: 'credit', card_isin: '555555' }), routesTo: checkout },
      {
        name: 'Mastercard debit sent', subType: 'debit',
        card: cardOf({ card_network: 'Mastercard', card_type: 'debit', card_isin: '555555' }), routesTo: adyen,
      },
      { name: 'a Mastercard BIN alone (sub-type debit)', subType: 'debit', card: cardOf({ card_isin: '555555' }), routesTo: null },
      { name: 'card_isin 411111', card: cardOf({ card_network: 'Discover', card_type: 'credit', card_isin: '411111' }), routesTo: razorpay },
      // The BIN condition compares the sent value exactly.
      { name: '8-digit card_isin 41111111', card: cardOf({ card_network: 'Discover', card_type: 'credit', card_isin: '41111111' }), routesTo: null },
      {
        name: 'card_extended_bin 41111111 without card_isin',
        card: cardOf({ card_network: 'Discover', card_type: 'credit', card_extended_bin: '41111111' }), routesTo: null,
      },
    ]
    for (const { name, card, subType, routesTo } of cases) {
      await test.step(name, async () => {
        const body = payment(eligible, b => {
          b.card = card
          if (subType) b.payment_method_sub_type = subType
        })
        const response = await api.raw<DecideResponse>('POST', '/decide', { body })
        if (routesTo) {
          expect.soft(response.body.outcome, name).toMatchObject({ type: 'routed', strategy: 'rule', reason: 'rule_matched' })
          expect.soft(response.body.connectors[0], name).toEqual(routesTo)
        } else {
          expect.soft(response.body.outcome, name).toMatchObject({ type: 'fallback', strategy: 'candidate_order', reason: 'no_rule_matched' })
          expect.soft(response.body.connectors, name).toEqual(eligible)
        }
        expect.soft(response.body.warnings, name).toEqual([])
      })
    }
  })
})
