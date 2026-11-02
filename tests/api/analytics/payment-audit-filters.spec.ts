import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { poll } from '../../helpers/poll'

/**
 * The payment audit list filters on what happened to the payment, not on any one event:
 * - `outcome` reads the statuses its score updates reported (a routing call's own `success` is
 *   not a payment status), so a failed-then-retried payment is a success and one with no
 *   terminal status is pending.
 * - payment attributes (method, type, currency, network, auth type, amount) come from the
 *   routing decision and select the whole payment, so its score updates stay in its summary.
 */

const INGEST_TIMEOUT = 40_000

type Payment = {
  key: string
  parameters: Record<string, string | number>
  statuses: string[]
}

const PAYMENTS: Payment[] = [
  {
    key: 'eur_credit_charged',
    parameters: {
      payment_method: 'card',
      payment_method_type: 'credit',
      currency: 'EUR',
      card_network: 'Visa',
      amount: 75,
    },
    statuses: ['CHARGED'],
  },
  {
    key: 'usd_debit_declined',
    parameters: {
      payment_method: 'card',
      payment_method_type: 'debit',
      currency: 'USD',
      amount: 150,
    },
    statuses: ['AUTHORIZATION_FAILED'],
  },
  {
    key: 'usd_wallet_retried',
    parameters: {
      payment_method: 'wallet',
      payment_method_type: 'google_pay',
      currency: 'USD',
      amount: 50,
    },
    statuses: ['AUTHORIZATION_FAILED', 'CHARGED'],
  },
  {
    key: 'usd_credit_vbv',
    parameters: {
      payment_method: 'card',
      payment_method_type: 'credit',
      currency: 'USD',
      amount: 20,
    },
    statuses: ['PENDING_VBV'],
  },
  {
    key: 'eur_credit_unanswered',
    parameters: {
      payment_method: 'card',
      payment_method_type: 'credit',
      currency: 'EUR',
      amount: 300,
    },
    statuses: [],
  },
]

function ruleParameters(parameters: Payment['parameters']) {
  return Object.fromEntries(
    Object.entries(parameters).map(([name, value]) => [
      name,
      typeof value === 'number' ? { type: 'number', value } : { type: 'enum_variant', value },
    ]),
  )
}

/** Routes a payment through /routing/hybrid, as Hyperswitch does, and returns the connector. */
async function routeHybrid(
  api: ApiClient,
  merchantId: string,
  paymentId: string,
  payment: Payment,
) {
  const r = await api.raw('POST', '/routing/hybrid', {
    body: {
      static_routing_request: {
        created_by: merchantId,
        payment_id: paymentId,
        parameters: ruleParameters(payment.parameters),
        fallback_output: [factory.gatewayConnector('stripe')],
      },
      dynamic_routing_request: factory.srDecideGatewayRequest({
        merchantId,
        eliminationEnabled: false,
        paymentInfo: { paymentId, amount: payment.parameters.amount },
      }),
    },
  })
  const gateway: string | undefined =
    r.body?.dynamic_routing?.decision?.decided_gateway ??
    r.body?.evaluated_connectors?.[0]?.gateway_name
  expect(gateway, JSON.stringify(r.body).slice(0, 500)).toBeTruthy()
  return gateway!
}

function search(api: ApiClient, qs: Record<string, string | number> = {}) {
  return api.raw('GET', '/analytics/payment-audit', {
    qs: { range: '1d', page: 1, page_size: 50, ...qs },
  })
}

test.describe('Payment audit outcome and payment filters (API)', () => {
  test.setTimeout(90_000)

  test('filters payments by outcome and by payment attributes', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)
    const rule = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, {
        name: factory.ruleName('audit_filters'),
        gateway: 'stripe',
      }),
    )
    await api.activateRoutingAlgorithm(m, rule.body.rule_id)

    const idOf = new Map<string, string>()
    for (const payment of PAYMENTS) {
      const paymentId = factory.paymentId(`audit_${payment.key}`)
      idOf.set(payment.key, paymentId)
      const gateway = await routeHybrid(api, m, paymentId, payment)
      for (const status of payment.statuses) {
        await api.updateGatewayScore(
          factory.updateGatewayScoreRequest({ merchantId: m, gateway, paymentId, status }),
        )
      }
    }

    // Every payment and every score update ingested: 2 successes (one after a retry), 1 failure.
    const all = await poll(
      () => search(api),
      (r) =>
        r.body.total_results === PAYMENTS.length &&
        r.body.total_success === 2 &&
        r.body.total_failure === 1,
      { message: 'payments did not reach the audit', timeout: INGEST_TIMEOUT },
    )
    const outcomeOf = new Map<string, string>(
      all.body.results.map((row: any) => [row.payment_id, row.outcome]),
    )
    expect(outcomeOf.get(idOf.get('eur_credit_charged')!)).toBe('success')
    expect(outcomeOf.get(idOf.get('usd_debit_declined')!)).toBe('failure')
    expect(outcomeOf.get(idOf.get('usd_wallet_retried')!)).toBe('success')
    expect(outcomeOf.get(idOf.get('usd_credit_vbv')!)).toBe('pending')
    expect(outcomeOf.get(idOf.get('eur_credit_unanswered')!)).toBe('pending')

    const matching = async (qs: Record<string, string | number>) => {
      const r = await search(api, qs)
      const byId = new Map([...idOf].map(([key, id]) => [id, key]))
      return r.body.results.map((row: any) => byId.get(row.payment_id)).sort()
    }

    expect(await matching({ outcome: 'success' })).toEqual([
      'eur_credit_charged',
      'usd_wallet_retried',
    ])
    expect(await matching({ outcome: 'failure' })).toEqual(['usd_debit_declined'])
    expect(await matching({ outcome: 'pending' })).toEqual([
      'eur_credit_unanswered',
      'usd_credit_vbv',
    ])
    // Several outcomes match a payment with any of them, and the totals follow.
    expect(await matching({ outcome: 'failure,pending' })).toEqual([
      'eur_credit_unanswered',
      'usd_credit_vbv',
      'usd_debit_declined',
    ])
    const unresolvedOrFailed = await search(api, { outcome: 'failure,pending' })
    expect(unresolvedOrFailed.body.total_results).toBe(3)
    expect(unresolvedOrFailed.body.total_failure).toBe(1)
    expect(unresolvedOrFailed.body.total_success).toBe(0)

    expect(await matching({ currency: 'eur' })).toEqual([
      'eur_credit_charged',
      'eur_credit_unanswered',
    ])
    expect(await matching({ payment_method: 'wallet' })).toEqual(['usd_wallet_retried'])
    expect(await matching({ payment_method_type: 'DEBIT' })).toEqual(['usd_debit_declined'])
    expect(await matching({ card_network: 'visa' })).toEqual(['eur_credit_charged'])

    // Several values for one attribute match a payment carrying any of them.
    expect(await matching({ payment_method_type: 'debit,GOOGLE_PAY' })).toEqual([
      'usd_debit_declined',
      'usd_wallet_retried',
    ])
    expect(await matching({ currency: 'USD,EUR' })).toHaveLength(PAYMENTS.length)
    expect(
      await matching({ currency: 'EUR', payment_method: 'card,wallet', outcome: 'pending' }),
    ).toEqual(['eur_credit_unanswered'])
    expect(await matching({ min_amount: 60, max_amount: 200 })).toEqual([
      'eur_credit_charged',
      'usd_debit_declined',
    ])

    // Attribute filters select whole payments: the matched payment keeps its score update, so its
    // outcome (and the totals) still read from it.
    const eur = await search(api, { currency: 'EUR', outcome: 'success' })
    expect(eur.body.results.map((row: any) => row.payment_id)).toEqual([
      idOf.get('eur_credit_charged'),
    ])
    expect(eur.body.results[0].outcome).toBe('success')
    expect(eur.body.total_success).toBe(1)
    expect(eur.body.total_success).toBe(1)

    // The dropdowns offer exactly what this merchant's payments carried, in the form the filters
    // match, so every option finds payments.
    const options = await api.raw('GET', '/analytics/payment-audit/filter-options', {
      qs: { range: '1d' },
    })
    expect(options.status).toBe(200)
    expect(options.body.payment_methods).toEqual(['card', 'wallet'])
    expect(options.body.payment_method_types).toEqual(['credit', 'debit', 'google_pay'])
    expect(options.body.currencies).toEqual(['eur', 'usd'])
    expect(options.body.card_networks).toEqual(['visa'])
    expect(options.body.gateways.length).toBeGreaterThan(0)
    for (const currency of options.body.currencies) {
      expect((await matching({ currency })).length, currency).toBeGreaterThan(0)
    }

    // Gateway and route take several values too.
    const allGateways = await search(api, { gateway: options.body.gateways.join(',') })
    expect(allGateways.body.total_results).toBe(PAYMENTS.length)
    const routes = await search(api, { route: 'routing_hybrid,decide_gateway' })
    expect(routes.body.total_results).toBe(PAYMENTS.length)

    // Without page_size the API pages at 20, as the dashboard does; up to 100 payments can be
    // listed and larger requests are capped there.
    const unsized = await api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', page: 1 } })
    expect(unsized.body.page_size).toBe(20)
    expect((await search(api, { page_size: 100 })).body.page_size).toBe(100)
    expect((await search(api, { page_size: 500 })).body.page_size).toBe(100)
  })

  test('filter options are scoped to the merchant', async ({ api, merchant }) => {
    // A merchant with no payments has nothing to offer, whatever other merchants routed.
    const options = await api.raw('GET', '/analytics/payment-audit/filter-options', {
      qs: { range: '1d' },
    })
    expect(options.status).toBe(200)
    expect(options.body).toEqual({
      gateways: [],
      payment_methods: [],
      payment_method_types: [],
      card_networks: [],
      auth_types: [],
      currencies: [],
      error_codes: [],
    })
    expect(merchant.id).toBeTruthy()
  })
})
