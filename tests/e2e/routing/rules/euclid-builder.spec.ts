import { test, expect } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'

/**
 * Port of cypress/e2e/ui/euclid-rules-builder.cy.js.
 *
 * The rule builder FORM only: page rendering, rule blocks, conditions, OR groups, gateways, the JSON
 * preview, and client-side validation. No rule reaches the backend — the one Create Rule click is the
 * empty-name case, which is blocked before it submits.
 *
 * Because nothing here mutates merchant state, these run against the worker-scoped `sharedMerchant`
 * (one signup per worker rather than one per test). Do not add a test to this file that creates a rule.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Rule Builder — UI interactions', () => {
  let euclid: EuclidRuleBuilder

  test.beforeEach(async ({ sharedPage }) => {
    euclid = new EuclidRuleBuilder(sharedPage)
    await euclid.goto('/routing/rules')
  })

  test.describe('Page rendering', () => {
    test('shows the rule builder form and an empty existing-rules panel', async ({ sharedPage }) => {
      await expect(sharedPage.getByRole('heading', { name: 'Rule Builder' })).toBeVisible()
      await expect(sharedPage.getByRole('heading', { name: 'Existing Rules' })).toBeVisible()
      await expect(sharedPage.getByPlaceholder('my-rule')).toBeVisible()
      await expect(sharedPage.getByPlaceholder('Optional description')).toBeVisible()
      await expect(sharedPage.getByText('No rule-based rules yet.')).toBeVisible()
    })

    test('shows the Default Fallback section below the rule list', async ({ sharedPage }) => {
      await expect(sharedPage.getByText('Default Fallback')).toBeVisible()
      await expect(sharedPage.getByText('Used when no rule matches')).toBeVisible()
    })

    test('shows Create Rule and Preview JSON buttons', async ({ sharedPage }) => {
      await expect(sharedPage.getByRole('button', { name: 'Create Rule' })).toBeVisible()
      await expect(sharedPage.getByRole('button', { name: 'Preview JSON' })).toBeVisible()
    })
  })

  test.describe('Rule block management', () => {
    test('adds a rule block when clicking Add Rule', async ({ sharedPage }) => {
      await euclid.addRuleBlock()

      await expect(sharedPage.getByPlaceholder('Rule name')).toHaveCount(1)
      await expect(euclid.ruleBlock(0).getByText('If', { exact: true })).toBeVisible()
      await expect(euclid.ruleBlock(0).getByText('Then route')).toBeVisible()
    })

    test('adds multiple rule blocks independently', async ({ sharedPage }) => {
      await euclid.addRuleBlock()
      await euclid.addRuleBlock()

      const names = sharedPage.getByPlaceholder('Rule name')
      await expect(names).toHaveCount(2)
      await expect(names.nth(0)).toHaveValue('Rule 1')
      await expect(names.nth(1)).toHaveValue('Rule 2')
    })

    test('allows renaming a rule block inline', async ({ sharedPage }) => {
      await euclid.addRuleBlock()

      const name = euclid.ruleBlock(0).getByPlaceholder('Rule name')
      await name.fill('card-rule')
      await expect(name).toHaveValue('card-rule')
    })

    test('collapses and expands a rule block', async ({ sharedPage }) => {
      await euclid.addRuleBlock()
      const block = euclid.ruleBlock(0)
      await expect(block.getByText('If', { exact: true })).toBeVisible()

      await block.locator('button[aria-label="Collapse rule"]').click()
      await expect(block.getByText('If', { exact: true })).toHaveCount(0)

      await block.locator('button[aria-label="Expand rule"]').click()
      await expect(block.getByText('If', { exact: true })).toBeVisible()
    })

    test('removes a rule block with the delete button', async ({ sharedPage }) => {
      await euclid.addRuleBlock()
      await euclid.addRuleBlock()
      await expect(sharedPage.getByPlaceholder('Rule name')).toHaveCount(2)

      await euclid.ruleBlock(0).locator('button[aria-label="Delete rule"]').click()

      await expect(sharedPage.getByPlaceholder('Rule name')).toHaveCount(1)
    })
  })

  test.describe('Condition editing', () => {
    test.beforeEach(async ({ sharedPage }) => {
      await euclid.addRuleBlock()
    })

    test('shows one condition row by default in a new rule block', async () => {
      const block = euclid.ruleBlock(0)
      expect(await block.locator('.cond-select').count()).toBeGreaterThanOrEqual(2)
      await expect(block.getByRole('button', { name: 'Add condition' })).toBeVisible()
    })

    test('adds a second AND condition', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add condition' }).click()

      await expect(block.getByText('IF', { exact: true })).toBeVisible()
      await expect(block.getByText('AND', { exact: true })).toBeVisible()
    })

    test('adds a third AND condition', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add condition' }).click()
      await block.getByRole('button', { name: 'Add condition' }).click()

      await expect(block.getByText('AND', { exact: true }).first()).toBeVisible()
      await expect(
        block.locator('.rounded-lg.border').first().locator('[class*="divide-y"] > div'),
      ).toHaveCount(3)
    })

    test('removes a condition when there are multiple', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add condition' }).click()
      await expect(block.getByText('AND', { exact: true })).toBeVisible()

      await block.locator('button[aria-label="Remove condition"]').first().click()

      await expect(block.getByText('AND', { exact: true })).toHaveCount(0)
    })

    test('shows enum value dropdown for an enum-type field', async () => {
      await euclid.selectCondLhs(0, 'payment_method')

      const block = euclid.ruleBlock(0)
      await expect(block.locator('.cond-select')).toHaveCount(3)
      await expect(block.locator('input[type="number"]')).toHaveCount(0)
    })

    test('shows numeric input and extra comparison operators for amount field', async () => {
      await euclid.selectCondLhs(0, 'amount')

      const block = euclid.ruleBlock(0)
      const operators = block.locator('select.cond-select').first().locator('option')
      expect(await operators.count()).toBeGreaterThanOrEqual(4)
      await expect(block.locator('input[type="number"]')).toBeVisible()
      // The value picker becomes a number input, so only LHS + operator remain as cond-selects.
      await expect(block.locator('.cond-select')).toHaveCount(2)
    })

    test('shows human-readable labels in the field dropdown', async ({ sharedPage }) => {
      await euclid.ruleBlock(0).locator('[data-cy="cond-lhs"] button.cond-select').first().click()

      // The dropdown renders through a portal at the document root, not inside the rule block.
      const options = sharedPage.locator('button[data-value]:not(.cond-select)')
      expect(await options.count()).toBeGreaterThan(0)
      for (const label of await options.allTextContents()) {
        expect(label.trim(), 'field labels should be humanised, not raw snake_case').not.toMatch(/_/)
      }

      await sharedPage.locator('body').click({ force: true })
    })

    test('shows human-readable labels in the enum value dropdown', async ({ sharedPage }) => {
      await euclid.selectCondLhs(0, 'payment_method')
      await euclid.ruleBlock(0).locator('[data-cy="cond-val"] button.cond-select').first().click()

      const options = sharedPage.locator('button[data-value]:not(.cond-select)')
      expect(await options.count()).toBeGreaterThan(0)
      for (const label of await options.allTextContents()) {
        expect(label.trim(), 'enum labels should be humanised').not.toMatch(/_/)
      }

      await sharedPage.locator('body').click({ force: true })
    })

    test('can select a different field and choose a value', async ({ sharedPage }) => {
      await euclid.selectCondLhs(0, 'currency')

      const value = euclid.ruleBlock(0).locator('[data-cy="cond-val"]').first()
      await expect(value).toBeVisible()
      await value.locator('button.cond-select').click()

      expect(
        await sharedPage.locator('button[data-value]:not(.cond-select)').count(),
      ).toBeGreaterThan(1)

      await sharedPage.locator('body').click({ force: true })
    })
  })

  test.describe('OR group management', () => {
    test.beforeEach(async ({ sharedPage }) => {
      await euclid.addRuleBlock()
    })

    test('does not show Remove group when only one group exists', async () => {
      await expect(euclid.ruleBlock(0).getByRole('button', { name: 'Remove group' })).toHaveCount(0)
    })

    test('adds an OR group when clicking Add OR group', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add OR group' }).click()

      await expect(block.getByRole('button', { name: 'Add condition' })).toHaveCount(2)
      await expect(block.getByText('or', { exact: true })).toBeVisible()
    })

    test('shows OR separator between groups', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add OR group' }).click()

      await expect(block.getByText('or', { exact: true })).toBeVisible()
    })

    test('shows Remove group button when multiple groups exist', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add OR group' }).click()

      await expect(block.getByRole('button', { name: 'Remove group' }).first()).toBeVisible()
    })

    test('removes an OR group', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add OR group' }).click()
      await expect(block.getByRole('button', { name: 'Add condition' })).toHaveCount(2)

      await block.getByRole('button', { name: 'Remove group' }).first().click()

      await expect(block.getByRole('button', { name: 'Add condition' })).toHaveCount(1)
      await expect(block.getByRole('button', { name: 'Remove group' })).toHaveCount(0)
    })

    test('can configure each OR group independently', async () => {
      const block = euclid.ruleBlock(0)
      await block.getByRole('button', { name: 'Add OR group' }).click()

      // Compared against what the second group actually started as, rather than against a field
      // name it is assumed not to hold. Both groups open on the same default, so asserting the
      // second is merely *something else* passes whenever that default happens not to be the value
      // set below — which is luck, not the property under test.
      const secondGroupLhs = block.locator('[data-cy="cond-lhs"] button.cond-select').nth(1)
      const before = await secondGroupLhs.getAttribute('data-value')

      // Setting the first group's field must not propagate to the second.
      await euclid.selectCondLhs(0, 'currency')

      await expect(block.locator('[data-cy="cond-lhs"] button.cond-select').first()).toHaveAttribute(
        'data-value',
        'currency'
      )
      await expect(secondGroupLhs).toHaveAttribute('data-value', before ?? '')
    })
  })

  test.describe('Gateway output', () => {
    test.beforeEach(async ({ sharedPage }) => {
      await euclid.addRuleBlock()
    })

    test('adds a gateway to the priority output', async () => {
      await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')

      await expect(euclid.ruleBlock(0).getByText('stripe')).toBeVisible()
    })

    test('adds multiple gateways and shows them in order', async () => {
      await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
      await euclid.addGatewayToBlock(0, 'adyen', 'mca_adyen')

      const block = euclid.ruleBlock(0)
      await expect(block.getByText('1. stripe')).toBeVisible()
      await expect(block.getByText('2. adyen')).toBeVisible()
    })

    test('removes a gateway from the priority list', async ({ sharedPage }) => {
      await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
      await euclid.addGatewayToBlock(0, 'adyen', 'mca_adyen')

      const block = euclid.ruleBlock(0)
      await block
        .locator('div')
        .filter({ hasText: /^1\. stripe/ })
        .last()
        .locator('button')
        .first()
        .click()

      await expect(block.getByText('stripe')).toHaveCount(0)
      await expect(block.getByText('1. adyen')).toBeVisible()
    })

    test('shows gateway name suggestions from other entries', async ({ sharedPage }) => {
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      const input = euclid.ruleBlock(0).getByPlaceholder('Gateway name')
      const listId = await input.getAttribute('list')
      await expect(sharedPage.locator(`datalist#${listId} option[value="stripe"]`)).toHaveCount(1)
    })
  })

  test.describe('Default Fallback', () => {
    test('adds a gateway to the default fallback', async ({ sharedPage }) => {
      await euclid.addFallbackGateway('checkout', 'mca_checkout')

      const section = sharedPage
        .locator('.rounded-xl')
        .filter({ has: sharedPage.getByText('Default Fallback') })
      await expect(section.getByText('checkout').first()).toBeVisible()
    })

    test('shows correct description text', async ({ sharedPage }) => {
      await expect(sharedPage.getByText('Used when no rule matches')).toBeVisible()
      await expect(sharedPage.getByText('fallback_output')).toBeVisible()
    })
  })

  test.describe('Preview JSON', () => {
    test('toggles the JSON preview panel', async ({ sharedPage }) => {
      await sharedPage.getByRole('button', { name: 'Preview JSON' }).click()
      await expect(sharedPage.getByRole('heading', { name: 'JSON Preview' })).toBeVisible()

      await sharedPage.getByRole('button', { name: 'Hide JSON' }).click()
      await expect(sharedPage.getByRole('heading', { name: 'JSON Preview' })).toHaveCount(0)
    })

    test('reflects the rule name in the JSON preview', async ({ sharedPage }) => {
      const ruleName = 'preview-name-rule'
      await sharedPage.getByPlaceholder('my-rule').fill(ruleName)
      await sharedPage.getByRole('button', { name: 'Preview JSON' }).click()

      await expect(sharedPage.locator('pre')).toContainText(ruleName)
    })

    test('reflects added gateways in the JSON preview', async ({ sharedPage }) => {
      await euclid.addRuleBlock()
      await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
      await sharedPage.getByPlaceholder('my-rule').fill('preview-gateway-rule')
      await sharedPage.getByRole('button', { name: 'Preview JSON' }).click()

      await expect(sharedPage.locator('pre')).toContainText('stripe')
    })

    test('reflects conditions in the JSON preview', async ({ sharedPage }) => {
      await euclid.addRuleBlock()
      await sharedPage.getByPlaceholder('my-rule').fill('preview-condition-rule')
      await sharedPage.getByRole('button', { name: 'Preview JSON' }).click()

      const preview = sharedPage.locator('pre')
      await expect(preview).toContainText('statements')
      await expect(preview).toContainText('condition')
    })
  })

  test.describe('Validation', () => {
    test('blocks submission and shows an error when rule name is empty', async ({ sharedPage }) => {
      await expect(sharedPage.getByPlaceholder('my-rule')).toHaveValue('')

      await sharedPage.getByRole('button', { name: 'Create Rule' }).click()

      await expect(sharedPage.getByText('Rule name is required')).toBeVisible()
    })
  })
})
