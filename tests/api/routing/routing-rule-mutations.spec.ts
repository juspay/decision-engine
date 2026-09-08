import { test, expect, factory } from '../../fixtures/test'

/**
 * The half of the routing-rule lifecycle the existing specs never touch: update, deactivate, delete.
 *
 * The guarded invariant across all three is that an ACTIVE rule is immutable — an operator must
 * deactivate before editing or deleting, so a live routing decision can't change under a payment
 * mid-flight. That's the property worth an E2E test; the evaluation semantics are already covered by
 * rule-routing.spec.ts.
 */
test.describe('Routing rule mutations (API)', () => {
  test('updates an inactive rule and the change takes effect on activation', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('mutate_stripe'), gateway: 'stripe' }),
    )
    const ruleId = created.body.rule_id

    const renamed = factory.ruleName('mutate_renamed')
    const updated = await api.raw('POST', '/routing/update', {
      failOnStatusCode: false,
      body: {
        created_by: m,
        routing_algorithm_id: ruleId,
        name: renamed,
        description: 'updated by playwright',
        algorithm: factory.singleRoutingPayload(m, { gateway: 'checkout' }).algorithm,
      },
    })

    expect(updated.status).toBe(200)
    expect(updated.body.rule_id).toBe(ruleId)
    expect(updated.body.name).toBe(renamed)

    // The updated algorithm is what evaluates once the rule goes live.
    await api.activateRoutingAlgorithm(m, ruleId)
    const evaluated = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(evaluated.body.output.connector.gateway_name).toBe('checkout')
  })

  test('an active rule cannot be updated', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('mutate_locked'), gateway: 'stripe' }),
    )
    const ruleId = created.body.rule_id
    await api.activateRoutingAlgorithm(m, ruleId)

    const r = await api.raw('POST', '/routing/update', {
      failOnStatusCode: false,
      body: {
        created_by: m,
        routing_algorithm_id: ruleId,
        name: factory.ruleName('mutate_blocked'),
        description: '',
        algorithm: factory.singleRoutingPayload(m, { gateway: 'adyen' }).algorithm,
      },
    })

    expect(r.status).toBe(400)
  })

  test('deactivating removes the rule from the active list', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('mutate_deactivate'), gateway: 'stripe' }),
    )
    const ruleId = created.body.rule_id
    await api.activateRoutingAlgorithm(m, ruleId)

    const active = await api.listActiveRoutingAlgorithms(m)
    expect(active.body.some((r: any) => r.id === ruleId)).toBe(true)

    // The handler returns unit, so the response has an empty body — status is the only signal.
    const deactivated = await api.raw('POST', '/routing/deactivate', {
      failOnStatusCode: false,
      body: { created_by: m, routing_algorithm_id: ruleId },
    })
    expect(deactivated.status).toBe(200)

    const afterActive = await api.listActiveRoutingAlgorithms(m)
    expect(afterActive.body.some((r: any) => r.id === ruleId)).toBe(false)

    // It still exists — deactivate is not delete.
    const all = await api.listRoutingAlgorithms(m)
    expect(all.body.some((r: any) => r.id === ruleId)).toBe(true)
  })

  test('evaluating after deactivation answers with the caller\'s fallback', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('mutate_eval_off'), gateway: 'stripe' }),
    )
    const ruleId = created.body.rule_id
    await api.activateRoutingAlgorithm(m, ruleId)

    await api.raw('POST', '/routing/deactivate', {
      body: { created_by: m, routing_algorithm_id: ruleId },
    })

    const evaluated = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(m, {}, { fallback_output: [factory.gatewayConnector('adyen')] }),
      { failOnStatusCode: false },
    )

    // A merchant who switched their rules off has an answer coming — route by the fallback —
    // rather than an error a caller would read as "engine unavailable" and paper over with its
    // own stale copy of the rule.
    expect(evaluated.status).toBe(200)
    expect(evaluated.body.status).toBe('no_active_algorithm')
    expect(evaluated.body.output.type).toBe('priority')
    expect(evaluated.body.evaluated_output.map((c: any) => c.gateway_name)).toEqual(['adyen'])
    expect(evaluated.body.eligible_connectors.map((c: any) => c.gateway_name)).toEqual(['adyen'])
    // The rule the merchant deactivated is gone from the answer entirely.
    expect(evaluated.body.evaluated_output.map((c: any) => c.gateway_name)).not.toContain('stripe')
  })

  test('evaluating for a merchant with no rules at all answers with the fallback', async ({ api, merchant }) => {
    // Indistinguishable to the caller from a profile whose rules are all switched off: both mean
    // "route by your fallback", so both are answered the same way.
    const evaluated = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(merchant.id, {}, {
        fallback_output: [factory.gatewayConnector('adyen')],
      }),
      { failOnStatusCode: false },
    )

    expect(evaluated.status).toBe(200)
    expect(evaluated.body.status).toBe('no_active_algorithm')
    expect(evaluated.body.evaluated_output.map((c: any) => c.gateway_name)).toEqual(['adyen'])
  })

  test('evaluating with no rules and no fallback still errors', async ({ api, merchant }) => {
    // Nothing to answer with -- a 200 carrying an empty output would read as a decision to nowhere.
    const evaluated = await api.evaluateRoutingAlgorithm(
      factory.ruleEvaluatePayload(merchant.id, {}, {}),
      { failOnStatusCode: false },
    )

    expect(evaluated.status).toBe(400)
  })

  test('rule deletion is disabled -- the delete route is removed', async ({ api, merchant }) => {
    // Deletion is disabled for parity with the Hyperswitch dashboard, which cannot delete
    // routing rules yet (juspay/hyperswitch-control-center#5495, item 4). The /routing/delete
    // route is commented out in src/app.rs, so any call to it 404s and the rule survives.
    const m = merchant.id
    const created = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('mutate_delete'), gateway: 'stripe' }),
    )
    const ruleId = created.body.rule_id

    const deleted = await api.raw('POST', '/routing/delete', {
      failOnStatusCode: false,
      body: { created_by: m, routing_algorithm_id: ruleId },
    })
    expect(deleted.status).toBe(404)

    const all = await api.listRoutingAlgorithms(m)
    expect(all.body.some((r: any) => r.id === ruleId)).toBe(true)
  })

  test('mutating an unknown rule id is rejected', async ({ api, merchant }) => {
    const m = merchant.id
    const unknown = 'routing_algorithm_that_does_not_exist'

    const updated = await api.raw('POST', '/routing/update', {
      failOnStatusCode: false,
      body: {
        created_by: m,
        routing_algorithm_id: unknown,
        name: factory.ruleName('ghost'),
        description: '',
        algorithm: factory.singleRoutingPayload(m, { gateway: 'stripe' }).algorithm,
      },
    })
    expect(updated.status).toBe(400)

    const deactivated = await api.raw('POST', '/routing/deactivate', {
      failOnStatusCode: false,
      body: { created_by: m, routing_algorithm_id: unknown },
    })
    expect(deactivated.status).toBe(400)
    // Delete is intentionally not exercised here: the /routing/delete route is removed
    // (deletion disabled for dashboard parity), so it 404s regardless of the id. The
    // dedicated "rule deletion is disabled" test covers that.
  })
})
