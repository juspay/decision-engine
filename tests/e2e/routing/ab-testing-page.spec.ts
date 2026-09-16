import { test, expect, factory } from '../../fixtures/test'
import { makeRule, setAutopilot } from '../../helpers/ab-test'

/**
 * The A/B testing page at /routing/ab-testing.
 *
 * The behaviour worth guarding in a browser is the CONTROL ARM, because it is the one thing on the
 * page the merchant does not normally configure: it is read off their live setup and pinned. If it
 * renders as anything other than what they actually run, every number the experiment produces is
 * being attributed to the wrong baseline — and the merchant has no way to tell.
 *
 * Arm assignment, storage shape and the create-time guards are covered far more cheaply in
 * tests/api/ab-testing/ab-testing.spec.ts; this file only asserts what needs a rendered page.
 */
test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('A/B testing page', () => {
  test('renders the page for a merchant with nothing configured', async ({ authedPage }) => {
    await authedPage.goto('/routing/ab-testing')
    await expect(authedPage.getByRole('heading', { name: /A\/B Test/i })).toBeVisible()
  })

  test.describe('Control arm', () => {
    test('a merchant with no rule and no SR is told there is nothing to compare against', async ({
      authedPage,
    }) => {
      await authedPage.goto('/routing/ab-testing')
      await authedPage.getByRole('button', { name: /New experiment|Create experiment/i }).first().click()

      await expect(
        authedPage.getByText(/no active routing rule and no success-rate routing/i),
      ).toBeVisible()
    })

    test('shows the merchant’s live rule as the control, without asking for it', async ({
      api,
      authedPage,
      merchant,
    }) => {
      const ruleId = await makeRule(api, merchant.id, 'checkout')
      await api.activateRoutingAlgorithm(merchant.id, ruleId)

      await authedPage.goto('/routing/ab-testing')
      await authedPage.getByRole('button', { name: /New experiment|Create experiment/i }).first().click()

      const control = authedPage.locator('div').filter({ hasText: /^Control/ }).first()
      await expect(control).toBeVisible()
      // The rule is displayed, not selected — a control the merchant can edit is not a control.
      await expect(authedPage.getByText(/Your rule decides/i)).toBeVisible()
    })

    /**
     * REGRESSION GUARD for the control arm reading as "manual tuning" while the merchant had
     * autopilot switched on.
     *
     * A resolved control arm now pins its dials at the merchant's live values, so the label has a
     * concrete thing to agree with — and must agree with it. The reader that produced this bug
     * collapsed an arm onto the sr_auth strategy, whose label says the opposite of what an arm
     * honoring autopilot does.
     */
    test('a control arm resolved with autopilot on is labelled auto-tuned', async ({
      api,
      authedPage,
      merchant,
    }) => {
      const m = merchant.id
      const controlRule = await makeRule(api, m, 'checkout')
      await api.activateRoutingAlgorithm(m, controlRule)
      await api.createSuccessRateConfig(m)
      // Both flags — the calibration job requires both, so either alone leaves the arm manual.
      await setAutopilot(api, m, true)

      try {
        const variantRule = await makeRule(api, m, 'adyen')
        const experimentName = factory.ruleName('ab_ui_exp')
        const created = await api.createRoutingAlgorithm({
          name: experimentName,
          description: 'control-arm label guard',
          created_by: m,
          algorithm_for: 'payment',
          metadata: {},
          algorithm: {
            type: 'ab_test',
            data: {
              control: { kind: 'current' },
              variant: { kind: 'hybrid', algorithm_id: variantRule, sr_config: { enable_multi_objective: true, use_autopilot: false } },
              variant_split_pct: 10,
              min_sample_size: 100,
              guardrail_threshold_pp: 3,
            },
          },
        })
        await api.activateRoutingAlgorithm(m, created.body.rule_id)

        await authedPage.goto('/routing/ab-testing')
        await expect(authedPage.getByText(experimentName).first()).toBeVisible()

        // Control pinned use_autopilot: true, so its label has to say so.
        await expect(authedPage.getByText(/auto-tuned/i).first()).toBeVisible()
      } finally {
        await setAutopilot(api, m, false)
      }
    })

    /**
     * Control arms created before the dials were pinned store no sr_config at all, and those rows
     * outlive the deploy that changed it. An arm that pins nothing must not be labelled with a
     * tuning mode it does not pin — naming one is the original bug, in the shape it still reaches.
     */
    test('a control arm storing no dials reads as live settings, not a tuning mode', async ({
      api,
      authedPage,
      merchant,
    }) => {
      const m = merchant.id
      const controlRule = await makeRule(api, m, 'checkout')
      await api.activateRoutingAlgorithm(m, controlRule)
      await api.createSuccessRateConfig(m)

      const variantRule = await makeRule(api, m, 'adyen')
      const experimentName = factory.ruleName('ab_ui_legacy')
      const created = await api.createRoutingAlgorithm({
        name: experimentName,
        description: 'unpinned control arm',
        created_by: m,
        algorithm_for: 'payment',
        metadata: {},
        algorithm: {
          type: 'ab_test',
          data: {
            // Written the way resolution used to store it: both legs, no dials.
            control: { kind: 'hybrid', algorithm_id: controlRule },
            variant: { kind: 'hybrid', algorithm_id: variantRule, sr_config: { enable_multi_objective: true, use_autopilot: true } },
            variant_split_pct: 10,
            min_sample_size: 100,
            guardrail_threshold_pp: 3,
          },
        },
      })
      await api.activateRoutingAlgorithm(m, created.body.rule_id)

      await authedPage.goto('/routing/ab-testing')
      await expect(authedPage.getByText(experimentName).first()).toBeVisible()

      await expect(authedPage.getByText(/your live settings/i).first()).toBeVisible()
      await expect(authedPage.getByText(/manual tuning/i)).toHaveCount(0)
    })
  })

  test.describe('Naming the control arm', () => {
    /**
     * The escape hatch for a baseline the live setup cannot express. Testing autopilot against a
     * static rule needs SR configured for autopilot to tune, and once it is, `current` resolves to
     * hybrid — so "my rule alone" has to be sayable outright.
     */
    test('the control can be switched from the live setup to a chosen arm', async ({
      api,
      authedPage,
      merchant,
    }) => {
      const ruleId = await makeRule(api, merchant.id, 'checkout')
      await api.activateRoutingAlgorithm(merchant.id, ruleId)
      await api.createSuccessRateConfig(merchant.id)

      await authedPage.goto('/routing/ab-testing')
      await authedPage.getByRole('button', { name: /New experiment|Create experiment/i }).first().click()

      // Starts on the live setup, described rather than selected.
      await expect(authedPage.getByText(/your rule picks the eligible connectors/i)).toBeVisible()

      await authedPage.getByRole('button', { name: /Choose the control arm instead/i }).click()

      // Now a real picker, and the live-setup description is gone.
      await expect(authedPage.getByText(/your rule picks the eligible connectors/i)).toHaveCount(0)
      await expect(authedPage.getByRole('button', { name: /Use my live setup instead/i })).toBeVisible()
    })
  })

  test.describe('Create form validation', () => {
    test('submitting a nameless experiment focuses the name field instead of explaining it', async ({
      api,
      authedPage,
      merchant,
    }) => {
      const ruleId = await makeRule(api, merchant.id, 'checkout')
      await api.activateRoutingAlgorithm(merchant.id, ruleId)

      await authedPage.goto('/routing/ab-testing')
      await authedPage.getByRole('button', { name: /New experiment|Create experiment/i }).first().click()

      const nameInput = authedPage.getByPlaceholder(/experiment name|e\.g\./i).first()
      await expect(nameInput).toBeVisible()
      await nameInput.fill('')

      await authedPage.getByRole('button', { name: /^Create experiment$/i }).last().click()

      // The cursor is the message.
      await expect(nameInput).toBeFocused()
      await expect(authedPage.getByText('Enter an experiment name')).toHaveCount(0)
    })
  })
})
