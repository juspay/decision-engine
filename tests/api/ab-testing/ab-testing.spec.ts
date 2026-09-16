import { test, expect, factory } from '../../fixtures/test'
import { experimentPayload, makeRule, readActiveExperiment, setAutopilot } from '../../helpers/ab-test'

/**
 * A/B experiments (API).
 *
 * An experiment is stored as a `StaticRoutingAlgorithm::AbTest` in the merchant's PAYMENT
 * activation slot, so it is created and activated through the ordinary /routing endpoints. What
 * makes it worth its own spec is everything that happens around that storage:
 *
 *  - the `current` control arm, which the server resolves against the merchant's live setup at
 *    create time and pins, so traffic already splitting cannot have its baseline moved,
 *  - the guards that keep an experiment comparable (no nesting, no self-comparison),
 *  - deterministic arm assignment, which is what lets a retry stay in the arm its first attempt
 *    was in.
 *
 * `/decide-gateway` interception additionally requires the `ab_test_real_payments_enabled`
 * feature; the specs that need it say so.
 */

// ── the `current` control arm ─────────────────────────────────────────────────

test.describe('A/B control arm resolution', () => {
  test('a rule-only merchant resolves `current` to a rule arm', async ({ api, merchant }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)

    const variantId = await makeRule(api, m, 'adyen')
    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: variantId }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.control).toEqual({ kind: 'rule', algorithm_id: ruleId })
  })

  test('a merchant with a rule AND success-rate routing resolves `current` to a hybrid arm', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)
    // Writing the SR config is what makes SR_V3_INPUT_CONFIG_<merchant> exist — the merchant's own
    // statement that they run SR, and the signal resolve_current_arm reads.
    await api.createSuccessRateConfig(m)

    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'sr', sr_config: { use_autopilot: true } }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.control.kind).toBe('hybrid')
    expect(data.control.algorithm_id).toBe(ruleId)
  })

  test('an SR-only merchant resolves `current` to an SR arm', async ({ api, merchant }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)

    const variantId = await makeRule(api, m, 'adyen')
    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: variantId }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.control.kind).toBe('sr')
  })

  /**
   * A resolved control arm writes down every dial a variant can vary, at the value the merchant
   * runs today. Both dials are read from shared state the merchant can change mid-experiment —
   * `enable_multi_objective` falls back to the feature flag, `use_autopilot` defaults to true
   * against entries the calibration job starts writing the moment it is switched on — so leaving
   * them unset is what lets a baseline drift onto the variant's side.
   */
  test('a resolved control arm pins the dials at their live values', async ({ api, merchant }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)
    await api.createSuccessRateConfig(m)

    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'sr', sr_config: { use_autopilot: true } }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.control.kind).toBe('hybrid')
    // A fresh merchant has neither feature on, so both pin false rather than going unwritten.
    expect(data.control.sr_config.use_autopilot).toBe(false)
    expect(data.control.sr_config.enable_multi_objective).toBe(false)
  })

  /**
   * REGRESSION GUARD for the experiment this whole arrangement exists to make possible: "is
   * autopilot better than my hand-tuned config?".
   *
   * Autopilot cannot be evaluated without being switched on — its values do not come from the arm,
   * they are written into the merchant's shared SR config by the calibration job. So the flag goes
   * on *during* the experiment, and the control has to stay on the pre-experiment baseline anyway.
   * With `use_autopilot` left unset the control would default to honoring the very entries the job
   * starts writing, both arms would move together, and the experiment would measure nothing while
   * reporting a clean null result.
   */
  test('enabling autopilot after creation does not move the control arm', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)
    await api.createSuccessRateConfig(m)

    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'sr', sr_config: { use_autopilot: true } }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    try {
      await setAutopilot(api, m, true)
      const data = await readActiveExperiment(api, m)
      expect(data.control.sr_config.use_autopilot).toBe(false)
      expect(data.variant.sr_config.use_autopilot).toBe(true)
    } finally {
      await setAutopilot(api, m, false)
    }
  })

  /**
   * The control arm pins what was live at create time, so a merchant already on autopilot gets a
   * control that keeps honoring it — the baseline is "what I run", not "autopilot off".
   */
  test('a merchant already on autopilot gets a control arm that keeps it', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)
    await api.createSuccessRateConfig(m)

    try {
      await setAutopilot(api, m, true)
      const created = await api.createRoutingAlgorithm(
        experimentPayload(m, { kind: 'current' }, { kind: 'sr', sr_config: { use_autopilot: false } }),
        { failOnStatusCode: false },
      )
      expect(created.status).toBe(200)
      await api.activateRoutingAlgorithm(m, created.body.rule_id)

      const data = await readActiveExperiment(api, m)
      expect(data.control.sr_config.use_autopilot).toBe(true)
    } finally {
      await setAutopilot(api, m, false)
    }
  })

  /**
   * Scenario 5 in its other shape: "should I move off my static rule onto autopilot-tuned SR?".
   *
   * The control has to be the rule *alone*, and `current` cannot name that — the calibration job
   * creates SR config for the merchant itself, after which `resolve_current_arm` sees SR
   * configured and resolves to hybrid. Naming the control arm outright is what keeps it rule-only.
   */
  test('a rule-only control can be named outright against an autopilot variant', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)
    // SR configured — `current` would resolve to hybrid here, which is the baseline we do NOT want.
    await api.createSuccessRateConfig(m)

    const created = await api.createRoutingAlgorithm(
      experimentPayload(
        m,
        { kind: 'rule', algorithm_id: ruleId },
        { kind: 'sr', sr_config: { enable_multi_objective: false, use_autopilot: true } },
      ),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.control).toEqual({ kind: 'rule', algorithm_id: ruleId })
    expect(data.variant.kind).toBe('sr')
    expect(data.variant.sr_config.use_autopilot).toBe(true)
  })

  test('a merchant with neither a rule nor SR cannot use `current`', async ({ api, merchant }) => {
    const m = merchant.id
    const variantId = await makeRule(api, m, 'adyen')

    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: variantId }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(400)
    expect(String(created.body?.message ?? created.body)).toContain('neither an active payment routing rule')
  })

  test('an experiment cannot nest inside another experiment', async ({ api, merchant }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)

    const variantId = await makeRule(api, m, 'adyen')
    const first = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: variantId }),
    )
    await api.activateRoutingAlgorithm(m, first.body.rule_id)

    // With the experiment now active, `current` would resolve to the experiment itself.
    const second = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: variantId }),
      { failOnStatusCode: false },
    )
    expect(second.status).toBe(400)
    expect(String(second.body?.message ?? second.body)).toContain('already has an active A/B experiment')
  })
})

// ── comparability guards ──────────────────────────────────────────────────────

test.describe('A/B experiment validation', () => {
  test('rejects an experiment whose two arms are identical', async ({ api, merchant }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')

    const created = await api.createRoutingAlgorithm(
      experimentPayload(
        m,
        { kind: 'rule', algorithm_id: ruleId },
        { kind: 'rule', algorithm_id: ruleId },
      ),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(400)
    expect(String(created.body?.message ?? created.body)).toContain('identically to the control arm')
  })

  test('two arms differing only in their SR leg are a valid comparison', async ({ api, merchant }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')

    const created = await api.createRoutingAlgorithm(
      experimentPayload(
        m,
        { kind: 'hybrid', algorithm_id: ruleId, sr_config: { enable_multi_objective: false, use_autopilot: false } },
        { kind: 'hybrid', algorithm_id: ruleId, sr_config: { enable_multi_objective: true, use_autopilot: true } },
      ),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
  })

  /**
   * The two arms are compared on the dials the decider would actually use, not on how much of the
   * configuration each one spells out. A variant that pins a dial to the value the merchant is
   * already running is the control written differently — structurally distinct, behaviourally the
   * same, and worth catching now rather than after a week of data comes back flat.
   */
  test('a variant that pins dials to the merchant\'s live values is rejected as a duplicate', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    await api.createSuccessRateConfig(m)

    // No rule, so `current` resolves to a pure SR arm pinned at this merchant's live dials — both
    // off, neither feature being on. The variant names exactly those values.
    const created = await api.createRoutingAlgorithm(
      experimentPayload(
        m,
        { kind: 'current' },
        { kind: 'sr', sr_config: { enable_multi_objective: false, use_autopilot: false } },
      ),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(400)
    expect(String(created.body?.message ?? created.body)).toContain('identically to the control arm')
  })

  test('`current` resolving into the same shape as the variant is still rejected', async ({
    api,
    merchant,
  }) => {
    const m = merchant.id
    const ruleId = await makeRule(api, m, 'checkout')
    await api.activateRoutingAlgorithm(m, ruleId)

    // Control resolves to { kind: 'rule', algorithm_id: ruleId } — exactly the variant.
    const created = await api.createRoutingAlgorithm(
      experimentPayload(m, { kind: 'current' }, { kind: 'rule', algorithm_id: ruleId }),
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(400)
    expect(String(created.body?.message ?? created.body)).toContain('identically to the control arm')
  })
})

// ── storage shape ─────────────────────────────────────────────────────────────

test.describe('A/B arm storage', () => {
  test('a hybrid arm round-trips both legs', async ({ api, merchant }) => {
    const m = merchant.id
    const controlRule = await makeRule(api, m, 'checkout')
    const variantRule = await makeRule(api, m, 'adyen')

    const created = await api.createRoutingAlgorithm(
      experimentPayload(
        m,
        { kind: 'rule', algorithm_id: controlRule },
        { kind: 'hybrid', algorithm_id: variantRule, sr_config: { enable_multi_objective: true, use_autopilot: true } },
      ),
    )
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    expect(data.variant.kind).toBe('hybrid')
    expect(data.variant.algorithm_id).toBe(variantRule)
    expect(data.variant.sr_config.enable_multi_objective).toBe(true)
    expect(data.variant.sr_config.use_autopilot).toBe(true)
  })

  /**
   * Experiments created before per-arm strategies existed stored a flat
   * control_algorithm_id / variant_algorithm_id pair, with 'sr_routing' as the reserved id meaning
   * "no rule, run SR". Those rows are still readable — an hour-long in-flight TTL is not the only
   * thing that outlives a deploy.
   */
  test('a legacy flat-shaped experiment is still accepted and normalized', async ({ api, merchant }) => {
    const m = merchant.id
    const controlRule = await makeRule(api, m, 'checkout')

    const created = await api.createRoutingAlgorithm(
      {
        name: factory.ruleName('abtest_legacy'),
        description: 'legacy flat shape',
        created_by: m,
        algorithm_for: 'payment',
        metadata: {},
        algorithm: {
          type: 'ab_test',
          data: {
            control_algorithm_id: controlRule,
            variant_algorithm_id: 'sr_routing',
            variant_sr_config: { enable_multi_objective: true },
            variant_split_pct: 10,
            min_sample_size: 100,
            guardrail_threshold_pp: 3,
          },
        },
      },
      { failOnStatusCode: false },
    )
    expect(created.status).toBe(200)
    await api.activateRoutingAlgorithm(m, created.body.rule_id)

    const data = await readActiveExperiment(api, m)
    // Read back in the current shape: the reserved id became a real SR arm.
    expect(data.control).toEqual({ kind: 'rule', algorithm_id: controlRule })
    expect(data.variant.kind).toBe('sr')
    expect(data.variant.sr_config.enable_multi_objective).toBe(true)
  })
})
