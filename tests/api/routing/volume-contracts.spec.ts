import { test, expect, factory } from '../../fixtures/test'

/**
 * Volume-commitment contract documents riding the euclid rule machinery: stored via
 * /routing/create as the `volume_contract` algorithm variant in the dedicated
 * `algorithm_for: volume_commitment` slot.
 *
 * Two properties carry the weight here:
 *   1. Write-time discipline — canonicalization (amounts → integer minor units, tolerance → bps)
 *      and validation (archetype gating, tier ordering, slot pairing) happen on create/update.
 *   2. Isolation — activating a volume contract must leave the merchant's live *payment* routing
 *      untouched: same-merchant /routing/evaluate keeps returning the payment rule.
 */

function volumeContractPayload(createdBy: string, overrides: Record<string, any> = {}) {
  const data = {
    routing_mode: 'pace_guarded',
    tolerance: '5pp',
    volume_contracts: [
      {
        id: 'adyen_lumpsum',
        connector: 'adyen',
        currency: { denomination: 'USD', amount_units: 'major' },
        billing_cycle: { type: 'calendar_month', anchor: 1, timezone: 'America/New_York' },
        archetype: 'lumpsum',
        terms: { target: 6_000_000, reward: { kind: 'flat', value: { flat_amount: 15_000 } } },
      },
      {
        id: 'stripe_tiered',
        connector: 'stripe',
        currency: { denomination: 'USD' },
        billing_cycle: { type: 'calendar_month', anchor: 1, timezone: 'UTC' },
        archetype: 'tiered',
        terms: {
          tiers: [
            { kind: 'retroactive', rate: { rebate_bps: 20 }, threshold: 800_000_000 },
            {
              kind: 'retroactive',
              rate: { rebate_bps: 25 },
              threshold: 1_000_000_000,
              rebate_lag_days: 30,
              rebate_settlement: 'credit_note',
            },
          ],
        },
      },
    ],
    ...(overrides.data || {}),
  }
  return {
    name: overrides.name || factory.ruleName('volume_contracts'),
    description: 'volume commitments (playwright)',
    created_by: createdBy,
    algorithm_for: overrides.algorithm_for || 'volume_commitment',
    algorithm: { type: 'volume_contract', data },
  }
}

async function createContract(api: any, payload: unknown) {
  return api.raw('POST', '/routing/create', { failOnStatusCode: false, body: payload })
}

test.describe('Volume contracts (API)', () => {
  test('stores a lumpsum + tiered document in canonical form', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await createContract(api, volumeContractPayload(m))
    expect(created.status).toBe(200)
    const ruleId = created.body.rule_id
    expect(ruleId).toBeTruthy()

    const listed = await api.listRoutingAlgorithms(m)
    const doc = listed.body.find((r: any) => r.id === ruleId)
    expect(doc).toBeTruthy()
    expect(doc.algorithm_for).toBe('volume_commitment')
    expect(doc.algorithm_data.type).toBe('volume_contract')

    const stored = doc.algorithm_data.data
    // "5pp" tolerance and $6M/$15k major-unit amounts are canonicalized on write.
    expect(stored.tolerance_bps).toBe(500)
    expect(stored.schema_version).toBe(1)
    const lumpsum = stored.volume_contracts.find((c: any) => c.id === 'adyen_lumpsum')
    expect(lumpsum.currency.amount_units).toBe('minor')
    expect(lumpsum.terms.target).toBe(600_000_000)
    expect(lumpsum.terms.reward.value.flat_amount).toBe(1_500_000)
    // Defaults materialize in the stored document.
    expect(lumpsum.status).toBe('active')
    expect(lumpsum.metric).toBe('gmv')
    expect(lumpsum.billing_cycle.proration).toBe('full_period')
  })

  test('rejects invalid documents with field-level errors', async ({ api, merchant }) => {
    const m = merchant.id

    // A contract document must ride the volume_commitment slot.
    const wrongSlot = await createContract(api, volumeContractPayload(m, { algorithm_for: 'payment' }))
    expect(wrongSlot.status).toBe(400)

    // ...and the slot only takes contract documents.
    const wrongPayload = await createContract(api, {
      ...factory.singleRoutingPayload(m, { gateway: 'stripe' }),
      algorithm_for: 'volume_commitment',
    })
    expect(wrongPayload.status).toBe(400)

    // Archetype B parses but is gated off in v1.
    const minCommitment = volumeContractPayload(m)
    minCommitment.algorithm.data.volume_contracts = [
      {
        id: 'wp_floor',
        connector: 'worldpay',
        currency: { denomination: 'USD' },
        billing_cycle: { type: 'calendar_month', anchor: 1, timezone: 'UTC' },
        archetype: 'min_commitment',
        terms: { floor: 600_000_000, reward: { kind: 'flat', value: { flat_amount: 12_000_000 } } },
      },
    ]
    const gated = await createContract(api, minCommitment)
    expect(gated.status).toBe(400)
    expect(JSON.stringify(gated.body)).toContain('min_commitment')

    // Tier thresholds must strictly increase.
    const badTiers = volumeContractPayload(m)
    badTiers.algorithm.data.volume_contracts[1].terms.tiers[1].threshold = 800_000_000
    const tiers = await createContract(api, badTiers)
    expect(tiers.status).toBe(400)

    // Unknown fields are rejected, not silently dropped.
    const typo = volumeContractPayload(m)
    ;(typo.algorithm.data.volume_contracts[0] as any).rebate_lagdays = 30
    const unknown = await createContract(api, typo)
    expect(unknown.status).toBe(400)

    // Amounts must fit the currency: USD has two decimal places.
    const badAmount = volumeContractPayload(m)
    badAmount.algorithm.data.volume_contracts[0].terms.target = '6000000.001'
    const amount = await createContract(api, badAmount)
    expect(amount.status).toBe(400)
  })

  test('runs the activate/update/deactivate/delete lifecycle in its own slot', async ({ api, merchant }) => {
    const m = merchant.id
    const created = await createContract(api, volumeContractPayload(m))
    const ruleId = created.body.rule_id

    await api.activateRoutingAlgorithm(m, ruleId)
    const active = await api.listActiveRoutingAlgorithms(m)
    const activeDoc = active.body.find((r: any) => r.id === ruleId)
    expect(activeDoc).toBeTruthy()
    expect(activeDoc.algorithm_for).toBe('volume_commitment')

    // Active documents are immutable — deactivate first, like any routing rule.
    const blockedUpdate = await api.raw('POST', '/routing/update', {
      failOnStatusCode: false,
      body: {
        created_by: m,
        routing_algorithm_id: ruleId,
        name: factory.ruleName('volume_contracts_blocked'),
        description: '',
        algorithm: volumeContractPayload(m).algorithm,
      },
    })
    expect(blockedUpdate.status).toBe(400)

    const deactivated = await api.raw('POST', '/routing/deactivate', {
      failOnStatusCode: false,
      body: { created_by: m, routing_algorithm_id: ruleId },
    })
    expect(deactivated.status).toBe(200)

    // Update re-validates: an edit that breaks the document is rejected even while inactive.
    const badEdit = volumeContractPayload(m)
    badEdit.algorithm.data.volume_contracts[0].billing_cycle.timezone = 'Mars/Olympus_Mons'
    const rejectedEdit = await api.raw('POST', '/routing/update', {
      failOnStatusCode: false,
      body: {
        created_by: m,
        routing_algorithm_id: ruleId,
        name: factory.ruleName('volume_contracts_bad_edit'),
        description: '',
        algorithm: badEdit.algorithm,
      },
    })
    expect(rejectedEdit.status).toBe(400)

    const deleted = await api.raw('POST', '/routing/delete', {
      failOnStatusCode: false,
      body: { created_by: m, routing_algorithm_id: ruleId },
    })
    expect(deleted.status).toBe(200)
    const all = await api.listRoutingAlgorithms(m)
    expect(all.body.some((r: any) => r.id === ruleId)).toBe(false)
  })

  test('an active volume contract does not disturb payment routing for the same merchant', async ({ api, merchant }) => {
    const m = merchant.id

    // Live payment rule: everything routes to stripe.
    const paymentRule = await api.createRoutingAlgorithm(
      factory.singleRoutingPayload(m, { name: factory.ruleName('vc_isolation'), gateway: 'stripe' }),
    )
    await api.activateRoutingAlgorithm(m, paymentRule.body.rule_id)

    const before = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(before.body.output.connector.gateway_name).toBe('stripe')

    // Activate a volume contract next to it — a second mapper row in its own slot.
    const contract = await createContract(api, volumeContractPayload(m))
    await api.activateRoutingAlgorithm(m, contract.body.rule_id)

    const active = await api.listActiveRoutingAlgorithms(m)
    expect(active.body.some((r: any) => r.algorithm_for === 'volume_commitment')).toBe(true)
    expect(active.body.some((r: any) => r.algorithm_for === 'payment')).toBe(true)

    // The payment flow still evaluates the payment rule — activate-time caching and the
    // evaluate-path lookup are both payment-scoped.
    const after = await api.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(m))
    expect(after.body.output.connector.gateway_name).toBe('stripe')
  })
})
