import { test, expect, factory } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'
import { expectApiCall } from '../../../helpers/network'

/**
 * Port of cypress/e2e/ui/euclid-rules-lifecycle.cy.js.
 *
 * Rule creation through the form and management from the existing-rules panel — these actually hit
 * POST /routing/create and the activate/deactivate endpoints, so they use the PER-TEST `merchant`
 * fixture (never the shared one: each test needs a clean rule list).
 *
 * `cy.intercept('POST','**\/routing/create').as('createRule')` + `cy.wait('@createRule')` becomes
 * `expectApiCall`, which must be created BEFORE the click that triggers the request.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Rule Lifecycle — creation and management', () => {
  let euclid: EuclidRuleBuilder
  let ruleName: string

  test.beforeEach(async ({ authedPage }) => {
    ruleName = factory.ruleName('ui_rule')
    euclid = new EuclidRuleBuilder(authedPage)
    await euclid.goto('/routing/rules')
  })

  /** Click Create Rule and assert the backend accepted it, surfacing the body on failure. */
  async function createRuleAndExpectSuccess(page: any) {
    const call = expectApiCall(page, '/routing/create')
    await page.getByRole('button', { name: 'Create Rule' }).click()
    const { status, body } = await call
    expect(status, `POST /routing/create failed: ${JSON.stringify(body)}`).toBe(200)
    return body
  }

  test.describe('Rule creation', () => {
    test('creates a minimal rule with name and default fallback only', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
    })

    test('creates a rule with one condition and one gateway', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)
      await authedPage.getByPlaceholder('Optional description').fill('Playwright test rule')

      await euclid.addRuleBlock()
      await euclid.ruleBlock(0).getByPlaceholder('Rule name').fill('card-rule')
      await euclid.selectCondLhs(0, 'payment_method')
      await euclid.selectCondVal(0, 'card')

      await euclid.addGatewayToBlock(0, 'adyen', 'mca_adyen')
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
      await expect(authedPage.getByText(ruleName).first()).toBeVisible()
    })

    test('creates a rule with two AND conditions', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)
      await euclid.addRuleBlock()

      await euclid.selectCondLhs(0, 'payment_method')
      await euclid.selectCondVal(0, 'card')

      await euclid.ruleBlock(0).getByRole('button', { name: 'Add condition' }).click()
      await euclid.selectCondLhs(1, 'currency')
      await euclid.selectCondVal(1, 'USD')

      await expect(euclid.ruleBlock(0).getByText('AND', { exact: true })).toBeVisible()

      await euclid.addGatewayToBlock(0, 'checkout', 'mca_checkout')
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
    })

    test('creates a rule with two OR groups', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)
      await euclid.addRuleBlock()

      await euclid.selectCondLhs(0, 'payment_method')
      await euclid.selectCondVal(0, 'card')

      await euclid.ruleBlock(0).getByRole('button', { name: 'Add OR group' }).click()
      // The second group's condition row is the next cond-lhs on the page.
      await euclid.selectCondLhs(1, 'currency')
      await euclid.selectCondVal(1, 'USD')

      await expect(euclid.ruleBlock(0).getByText('or', { exact: true })).toBeVisible()

      await euclid.addGatewayToBlock(0, 'adyen', 'mca_adyen')
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
    })

    test('creates two rule blocks each targeting a different gateway', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)

      await euclid.addRuleBlock()
      await euclid.ruleBlock(0).getByPlaceholder('Rule name').fill('card-rule')
      await euclid.selectCondLhs(0, 'payment_method')
      await euclid.selectCondVal(0, 'card')
      await euclid.addGatewayToBlock(0, 'adyen', 'mca_adyen')

      await euclid.addRuleBlock()
      await euclid.ruleBlock(1).getByPlaceholder('Rule name').fill('upi-rule')
      await euclid.selectCondLhs(1, 'payment_method')
      await euclid.selectCondVal(1, 'upi')
      await euclid.addGatewayToBlock(1, 'razorpay', 'mca_razorpay')

      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
    })

    test('creates a rule with an amount (integer) condition', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(ruleName)
      await euclid.addRuleBlock()

      await euclid.selectCondLhs(0, 'amount')
      await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'greater than' })
      await euclid.ruleBlock(0).locator('input[type="number"]').fill('100')

      await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
      await euclid.addFallbackGateway('adyen', 'mca_adyen')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByText('Rule created')).toBeVisible()
    })
  })

  test.describe('Existing rules panel', () => {
    /** Locate the panel row for a rule by name. */
    const ruleRow = (page: any, name: string) =>
      page.getByText(name).first().locator('xpath=ancestor::*[contains(@class,"flex-col")][1]')

    test.beforeEach(async ({ api, merchant, authedPage }) => {
      // Seed a rule through the API so the panel has something to manage.
      const created = await api.createRoutingAlgorithm(
        factory.advancedRoutingPayload(merchant.id, { name: ruleName }),
      )
      expect(created.status).toBe(200)
      await euclid.goto('/routing/rules')
    })

    test('shows the created rule as Inactive', async ({ authedPage }) => {
      await expect(authedPage.getByText(ruleName).first()).toBeVisible()
      await expect(ruleRow(authedPage, ruleName).getByText('Inactive')).toBeVisible()
    })

    test('shows the rule description under the rule name', async ({ authedPage }) => {
      // The Cypress original asserted a `p.text-xs` condition summary; the panel now renders the
      // algorithm's description at text-[11px] instead, so assert on the content rather than the class.
      await expect(
        ruleRow(authedPage, ruleName).getByText('advanced routing rule'),
      ).toBeVisible()
    })

    test('expands rule details when rule header is clicked', async ({ authedPage }) => {
      await authedPage.getByText(ruleName).first().click()

      await expect(ruleRow(authedPage, ruleName).locator('.border-t').first()).toBeVisible()
    })

    test('hides rule details when rule header is clicked again', async ({ authedPage }) => {
      await authedPage.getByText(ruleName).first().click()
      await expect(ruleRow(authedPage, ruleName).locator('.border-t').first()).toBeVisible()

      await authedPage.getByText(ruleName).first().click()

      await expect(ruleRow(authedPage, ruleName).locator('.border-t')).toHaveCount(0)
    })

    test('activates the rule', async ({ authedPage }) => {
      await ruleRow(authedPage, ruleName).getByRole('button', { name: 'Activate' }).click()

      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible()
      await expect(ruleRow(authedPage, ruleName).getByText('● Active')).toBeVisible()
    })

    test('deactivates an active rule', async ({ authedPage }) => {
      await ruleRow(authedPage, ruleName).getByRole('button', { name: 'Activate' }).click()
      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible()

      await ruleRow(authedPage, ruleName).getByRole('button', { name: 'Deactivate' }).click()

      // Deactivation is behind a confirmation dialog — turning off live routing is not a stray click.
      await expect(authedPage.getByText('Deactivate this rule?')).toBeVisible()
      await authedPage.locator('.fixed.inset-0').getByRole('button', { name: 'Deactivate' }).click()

      await expect(authedPage.getByText('Rule deactivated successfully.')).toBeVisible()
      await expect(ruleRow(authedPage, ruleName).getByText('Inactive')).toBeVisible()
    })

    test('shows Activate Now immediately after creating from the form', async ({ authedPage }) => {
      await authedPage.getByPlaceholder('my-rule').fill(factory.ruleName('quick'))
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)

      await expect(authedPage.getByRole('button', { name: 'Activate Now' })).toBeVisible()
    })

    test('activates a newly created rule via Activate Now', async ({ authedPage }) => {
      const quickRule = factory.ruleName('quick')
      await authedPage.getByPlaceholder('my-rule').fill(quickRule)
      await euclid.addFallbackGateway('stripe', 'mca_stripe')

      await createRuleAndExpectSuccess(authedPage)
      await authedPage.getByRole('button', { name: 'Activate Now' }).click()

      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible({ timeout: 15_000 })
      await expect(ruleRow(authedPage, quickRule).getByText('● Active')).toBeVisible()
    })
  })
})
