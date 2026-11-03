import { test, expect, factory, poll } from '../../fixtures/test'

/**
 * The payment field filter (`payment_fields`) narrows the audit list by what a payment's routing
 * requests carried: each endpoint stores it differently (rule parameters on /decide and
 * /routing/hybrid, the decider's paymentInfo on /decide-gateway), and one field name finds all of
 * them.
 */
test.describe('Payment audit payment field filter (API)', () => {
  test('finds payments by card BIN, country and metadata across every routing endpoint', async ({ api, merchant }) => {
    test.setTimeout(120_000)
    const connectors = [{ gateway_name: 'stripe', merchant_connector_id: 'mca_stripe' }]
    const decide = (paymentId: string, bin: string, segment: string, country: string) =>
      api.raw('POST', '/decide', { body: {
        payment_id: paymentId, merchant_id: merchant.id, amount: 1000, currency: 'USD',
        payment_method_type: 'card', payment_method_sub_type: 'credit', billing_country: country,
        card: { card_isin: bin }, metadata: { segment },
        routing_config: { algorithm_for: 'payment', eligible_connectors: connectors },
      } })

    const decidePremium = factory.paymentId('fields_decide_premium')
    const decideBasic = factory.paymentId('fields_decide_basic')
    const hybridPremium = factory.paymentId('fields_hybrid_premium')
    const gatewayPremium = factory.paymentId('fields_gateway_premium')
    expect((await decide(decidePremium, '424242', 'premium', 'US')).status).toBe(200)
    expect((await decide(decideBasic, '555555', 'basic', 'IN')).status).toBe(200)
    const hybrid = await api.raw('POST', '/routing/hybrid', { body: {
      static_routing_request: {
        created_by: merchant.id, payment_id: hybridPremium,
        parameters: {
          payment_method: { type: 'enum_variant', value: 'card' },
          card_bin: { type: 'str_value', value: '424242' },
          currency: { type: 'enum_variant', value: 'USD' },
          segment: { type: 'metadata_variant', value: { key: 'segment', value: 'premium' } },
        },
        fallback_output: [factory.gatewayConnector('stripe')],
      },
    } })
    expect(hybrid.status).toBe(200)
    const gateway = await api.decideGateway(factory.srDecideGatewayRequest({
      merchantId: merchant.id, eligibleGatewayList: ['stripe'],
      paymentInfo: { paymentId: gatewayPremium, cardIsin: '424242', cardSwitchProvider: 'VISA', metadata: JSON.stringify({ segment: 'premium' }) },
    }))
    expect(gateway.status).toBe(200)

    const seeded = [decidePremium, decideBasic, hybridPremium, gatewayPremium]
    const listed = async (fields: Record<string, string>) => {
      const response = await poll(
        () => api.raw('GET', '/analytics/payment-audit', {
          qs: { range: '1h', page_size: 100, payment_fields: JSON.stringify(fields) },
        }),
        (response) => response.status === 200,
        { message: `payment audit with ${JSON.stringify(fields)}` },
      )
      return response.body.results
        .map((row: any) => row.payment_id)
        .filter((id: string) => seeded.includes(id))
        .sort()
    }
    // Wait until every seeded payment's decision is queryable.
    await poll(
      () => api.raw('GET', '/analytics/payment-audit', { qs: { range: '1h', page_size: 100 } }),
      (response) => seeded.every((id) => response.body.results?.some((row: any) => row.payment_id === id)),
      { message: 'every seeded payment in the audit list', timeout: 60_000 },
    )

    expect(await listed({ card_bin: '424242' })).toEqual([decidePremium, gatewayPremium, hybridPremium].sort())
    expect(await listed({ 'metadata.segment': 'PREMIUM' })).toEqual([decidePremium, gatewayPremium, hybridPremium].sort())
    // /decide takes ISO codes and routes on the country's name; either finds it.
    expect(await listed({ billing_country: 'US' })).toEqual([decidePremium])
    expect(await listed({ billing_country: 'UnitedStatesOfAmerica' })).toEqual([decidePremium])
    // Several fields: a payment must carry all of them.
    expect(await listed({ card_bin: '555555', 'metadata.segment': 'basic' })).toEqual([decideBasic])
    expect(await listed({ card_bin: '424242', 'metadata.segment': 'basic' })).toEqual([])
    // /decide-gateway carries the card network as cardSwitchProvider.
    expect(await listed({ card_network: 'visa' })).toEqual([gatewayPremium])
    // A metadata field reads only metadata: the currency rule parameter isn't metadata.currency.
    expect(await listed({ currency: 'USD' })).toContain(hybridPremium)
    expect(await listed({ 'metadata.currency': 'USD' })).toEqual([])

    // The totals come with the page; a page past the last still reports them.
    const fields = JSON.stringify({ card_bin: '424242' })
    const first = await api.raw('GET', '/analytics/payment-audit', { qs: { range: '1h', page_size: 1, payment_fields: fields } })
    const beyond = await api.raw('GET', '/analytics/payment-audit', { qs: { range: '1h', page: 99, page_size: 1, payment_fields: fields } })
    expect(first.body.results).toHaveLength(1)
    expect(first.body.total_results).toBe(3)
    expect(beyond.body.results).toHaveLength(0)
    expect(beyond.body.total_results).toBe(3)
    expect(beyond.body.total_success).toBe(first.body.total_success)

    // A matched payment keeps its whole trail: the filter selects payments, not events.
    const response = await api.raw('GET', '/analytics/payment-audit', {
      qs: { range: '1h', payment_id: decidePremium, payment_fields: JSON.stringify({ card_bin: '424242' }) },
    })
    expect(response.body.results).toHaveLength(1)
  })

  test('rejects a field it cannot match, naming the problem', async ({ api, merchant: _merchant }) => {
    for (const [fields, message] of [
      ['{"card_bni":"424242"}', 'not a routing key'],
      ['{"amount":"100"}', 'min_amount'],
      ['card_bin=424242', 'JSON object'],
      ['{"metadata.a\'b":"x"}', 'metadata keys'],
    ]) {
      const response = await api.raw('GET', '/analytics/payment-audit', {
        qs: { range: '1h', payment_fields: fields },
        failOnStatusCode: false,
      })
      expect(response.status, fields).toBe(400)
      expect(JSON.stringify(response.body), fields).toContain(message)
    }
  })
})
