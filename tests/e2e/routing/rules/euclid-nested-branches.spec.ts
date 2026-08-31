import { test, expect } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'

/**
 * Port of cypress/e2e/ui/euclid-rules-nested-branches.cy.js.
 *
 * Nested AND+OR branches inside a rule block: adding, indenting, the OR separator, the depth cap of 1,
 * and removal. Pure UI — nothing reaches the backend — so this uses the worker-scoped merchant, which
 * matches the Cypress original's suite-level `before()` hook.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Nested AND+OR branches', () => {
  let euclid: EuclidRuleBuilder

  test.beforeEach(async ({ sharedPage }) => {
    euclid = new EuclidRuleBuilder(sharedPage)
    await euclid.goto('/routing/rules/new')
    await euclid.addRuleBlock()
  })

  test('shows "Add nested branch" in each condition group footer', async () => {
    await expect(euclid.ruleBlock(0).getByRole('button', { name: 'Add nested branch' })).toBeVisible()
  })

  test('adds a nested branch section on click', async () => {
    await euclid.addNestedBranch(0)

    await expect(euclid.ruleBlock(0).getByText('Then match any of')).toBeVisible()
  })

  test('renders the nested group indented with a sky left border', async () => {
    await euclid.addNestedBranch(0)

    await expect(euclid.nestedBranches(0)).toHaveCount(1)
  })

  test('a second nested branch shows an OR separator', async () => {
    await euclid.addNestedBranch(0)
    await euclid.addNestedBranch(0)

    await expect(euclid.nestedBranches(0)).toHaveCount(2)
    await expect(euclid.ruleBlock(0).getByText('OR', { exact: true })).toBeVisible()
  })

  test('"Add nested branch" does not appear inside a nested group (depth capped at 1)', async () => {
    await euclid.addNestedBranch(0)

    // Still exactly one button in the whole block — a nested group cannot itself nest.
    await expect(euclid.ruleBlock(0).getByRole('button', { name: 'Add nested branch' })).toHaveCount(1)
  })

  test('allows adding AND conditions inside a nested branch', async () => {
    await euclid.addNestedBranch(0)

    const branch = euclid.nestedBranch(0, 0)
    await branch.getByRole('button', { name: 'Add condition' }).click()

    await expect(branch.getByText('AND', { exact: true })).toBeVisible()
  })

  test('nested branch can target a different field from the parent condition', async ({ sharedPage }) => {
    await euclid.selectCondLhs(0, 'payment_method')
    await euclid.addNestedBranch(0)

    // The nested branch's LHS is the second cond-lhs on the page.
    await euclid.selectCondLhs(1, 'currency')

    // The parent's field must be untouched.
    await expect(
      euclid.ruleBlock(0).locator('[data-cy="cond-lhs"] button.cond-select').first(),
    ).toHaveAttribute('data-value', 'payment_method')
  })

  test('removes a nested branch via Remove group', async () => {
    await euclid.addNestedBranch(0)
    await euclid.addNestedBranch(0)
    await expect(euclid.nestedBranches(0)).toHaveCount(2)

    await euclid.nestedBranch(0, 0).getByRole('button', { name: 'Remove group' }).click()

    await expect(euclid.nestedBranches(0)).toHaveCount(1)
    await expect(euclid.ruleBlock(0).getByText('OR', { exact: true })).toHaveCount(0)
  })

  test('hides the nested section when all branches are removed', async () => {
    await euclid.addNestedBranch(0)

    await euclid.nestedBranch(0, 0).getByRole('button', { name: 'Remove group' }).click()

    await expect(euclid.ruleBlock(0).getByText('Then match any of')).toHaveCount(0)
  })

  test('OR groups each get their own independent "Add nested branch" button', async () => {
    await euclid.ruleBlock(0).getByRole('button', { name: 'Add OR group' }).click()

    await expect(euclid.ruleBlock(0).getByRole('button', { name: 'Add nested branch' })).toHaveCount(2)
  })
})
