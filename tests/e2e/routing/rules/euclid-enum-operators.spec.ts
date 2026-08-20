import { test, expect } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'

/**
 * Port of cypress/e2e/ui/euclid-rules-enum-operators.cy.js.
 *
 * The "is one of" / "is not one of" operators, which swap the single-value dropdown for a multi-select
 * and change the emitted condition from `enum_variant` to `enum_variant_array`. Pure UI, so this runs
 * on the worker-scoped merchant (matching the Cypress `before()` hook).
 *
 * The multi-select renders through a portal at the document root, so its locators are page-level rather
 * than scoped to the rule block — the Cypress original used `{ withinSubject: null }` for the same reason.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

/** Selected values render as pills in the trigger; this is how the original counted them. */
const PILL = 'span[class*="bg-brand-100"]'

test.describe('"is one of" / "is not one of" operator', () => {
  let euclid: EuclidRuleBuilder

  test.beforeEach(async ({ sharedPage }) => {
    euclid = new EuclidRuleBuilder(sharedPage)
    await euclid.goto('/routing/rules/new')
    await euclid.addRuleBlock()
    await euclid.selectCondLhs(0, 'payment_method')
  })

  test('exposes "is one of" and "is not one of" in the operator dropdown for enum fields', async () => {
    const operator = euclid.ruleBlock(0).locator('select.cond-select').first()

    await expect(operator.locator('option', { hasText: 'is one of' }).first()).toHaveCount(1)
    await expect(operator.locator('option', { hasText: 'is not one of' }).first()).toHaveCount(1)
  })

  test('does not offer "is one of" for numeric fields', async () => {
    await euclid.selectCondLhs(0, 'amount')

    const operator = euclid.ruleBlock(0).locator('select.cond-select').first()
    await expect(operator.locator('option', { hasText: 'is one of' })).toHaveCount(0)
  })

  test('shows a multi-value picker when "is one of" is selected', async () => {
    const block = euclid.ruleBlock(0)
    await block.locator('select.cond-select').first().selectOption({ label: 'is one of' })

    // LHS button + operator select; the single-value button is replaced by the multi-select.
    await expect(block.locator('.cond-select')).toHaveCount(2)
    await expect(block.locator('[data-cy="cond-val"]')).toHaveCount(1)
  })

  test('shows a multi-value picker when "is not one of" is selected', async () => {
    const block = euclid.ruleBlock(0)
    await block.locator('select.cond-select').first().selectOption({ label: 'is not one of' })

    await expect(block.locator('.cond-select')).toHaveCount(2)
    await expect(block.locator('[data-cy="cond-val"]')).toHaveCount(1)
  })

  test('each option is independently toggleable', async ({ sharedPage }) => {
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is one of' })

    // Switching operators preserves the single value, so clear the carried-over pill first.
    const value = sharedPage.locator('[data-cy="cond-val"]').first()
    await value.locator(`${PILL} button`).click()

    await value.click()
    const options = sharedPage.locator('button[data-value]:not(.cond-select)')
    const chosen = await options.first().getAttribute('data-value')
    await options.first().click({ force: true })
    await sharedPage.locator('body').click({ force: true })
    await expect(value.locator(PILL)).toHaveCount(1)

    // Re-open and click the same option to deselect it.
    await value.click()
    await sharedPage.locator(`button[data-value="${chosen}"]:not(.cond-select)`).click({ force: true })
    await sharedPage.locator('body').click({ force: true })

    await expect(value.locator(PILL)).toHaveCount(0)
  })

  test('multiple options can be selected simultaneously', async ({ sharedPage }) => {
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is one of' })

    const value = sharedPage.locator('[data-cy="cond-val"]').first()
    await value.locator(`${PILL} button`).click()

    await value.click()
    const options = sharedPage.locator('button[data-value]:not(.cond-select)')
    expect(await options.count()).toBeGreaterThanOrEqual(2)
    const first = await options.nth(0).getAttribute('data-value')
    const second = await options.nth(1).getAttribute('data-value')
    await sharedPage.locator(`button[data-value="${first}"]:not(.cond-select)`).click({ force: true })
    await sharedPage.locator(`button[data-value="${second}"]:not(.cond-select)`).click({ force: true })
    await sharedPage.locator('body').click({ force: true })

    await expect(value.locator(PILL)).toHaveCount(2)
  })

  test('switching back to "equals" replaces the multi-picker with the single-value dropdown', async () => {
    const block = euclid.ruleBlock(0)
    const operator = block.locator('select.cond-select').first()

    await operator.selectOption({ label: 'is one of' })
    await expect(block.locator('.cond-select')).toHaveCount(2)

    await operator.selectOption({ label: 'equals' })
    // LHS button + operator select + value button.
    await expect(block.locator('.cond-select')).toHaveCount(3)
  })

  test('preserves the previously selected single value when switching to "is one of"', async ({ sharedPage }) => {
    await euclid.ruleBlock(0).locator('[data-cy="cond-val"] button.cond-select').first().click()
    await sharedPage.locator('button[data-value]:not(.cond-select)').nth(1).click()

    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is one of' })

    await expect(sharedPage.locator('[data-cy="cond-val"]').first().locator(PILL)).toHaveCount(1)
  })
})
