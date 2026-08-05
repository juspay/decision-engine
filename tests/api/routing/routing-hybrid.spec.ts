import { test, expect, factory } from '../../fixtures/test'

/**
 * /routing/hybrid combines the static (rule-based) and dynamic (success-rate) deciders in one call.
 *
 * The behaviour worth guarding end-to-end is its GRACEFUL DEGRADATION: a caller that supplies a
 * fallback must always get a usable connector list back, even when no routing rule matches — a
 * payment should never be blocked because the merchant hasn't configured routing yet.
 *
 * The full dynamic path (which runs the whole decider against Redis scores and gateway config) is
 * covered by decide-gateway.spec.ts; here the static side plus the request contract is the target.
 */
test.describe('Hybrid routing (API)', () => {
  test('rejects a request with neither sub-request', async ({ api, merchant }) => {
    const r = await api.raw('POST', '/routing/hybrid', { failOnStatusCode: false, body: {} })

    expect(r.status).toBe(400)
    expect(String(r.body?.message ?? r.body)).toContain('At least one of')
  })

  test('falls back to the caller-supplied connectors when no rule is active', async ({ api, merchant }) => {
    const m = merchant.id

    const r = await api.raw('POST', '/routing/hybrid', {
      failOnStatusCode: false,
      body: {
        static_routing_request: {
          created_by: m,
          parameters: {
            payment_method: { type: 'enum_variant', value: 'card' },
            amount: { type: 'number', value: 100 },
          },
          fallback_output: [factory.gatewayConnector('stripe')],
        },
      },
    })

    // No active algorithm exists for this fresh merchant, but the fallback keeps the call usable.
    expect(r.status).toBe(200)
    expect(Array.isArray(r.body.evaluated_connectors)).toBe(true)
    expect(r.body.evaluated_connectors.map((c: any) => c.gateway_name)).toContain('stripe')
  })

  test('evaluates the active static rule when one exists', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('hybrid_static'), gateway: 'checkout' }),
    )
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const r = await api.raw('POST', '/routing/hybrid', {
      failOnStatusCode: false,
      body: {
        static_routing_request: {
          created_by: m,
          parameters: {
            payment_method: { type: 'enum_variant', value: 'card' },
            amount: { type: 'number', value: 100 },
          },
          fallback_output: [factory.gatewayConnector('stripe')],
        },
      },
    })

    expect(r.status).toBe(200)
    expect(r.body.static_routing).toBeTruthy()
    // The configured rule wins over the fallback.
    expect(r.body.evaluated_connectors.map((c: any) => c.gateway_name)).toContain('checkout')
  })

  test('a static request with no rule and no fallback is rejected', async ({ api, merchant }) => {
    const r = await api.raw('POST', '/routing/hybrid', {
      failOnStatusCode: false,
      body: {
        static_routing_request: {
          created_by: merchant.id,
          parameters: { amount: { type: 'number', value: 100 } },
        },
      },
    })

    // Nothing to route to and nothing to fall back on — the caller has to be told.
    expect(r.status).toBeGreaterThanOrEqual(400)
  })
})
