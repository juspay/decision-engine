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
    await euclid.goto('/routing/rules/new')
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
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

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
    })
  })

  test.describe('Rules list', () => {
    test.beforeEach(async ({ api, merchant }) => {
      // Seed a rule through the API so the list has something to manage.
      const created = await api.createRoutingAlgorithm(
        factory.advancedRoutingPayload(merchant.id, { name: ruleName }),
      )
      expect(created.status).toBe(200)
      await euclid.gotoList()
    })

    test('shows the created rule as Inactive', async () => {
      await expect(euclid.ruleRow(ruleName)).toBeVisible()
      await expect(euclid.ruleRow(ruleName).getByText('Inactive')).toBeVisible()
    })

    test('summarises the rule destination in the table', async () => {
      await expect(euclid.ruleRow(ruleName).getByText(/Priority 100%|fallback/)).toBeVisible()
    })

    test('expands rule details when the row is clicked', async ({ authedPage }) => {
      await euclid.ruleRow(ruleName).click()

      await expect(authedPage.getByText('advanced routing rule')).toBeVisible()
    })

    test('hides rule details when the row is clicked again', async ({ authedPage }) => {
      await euclid.ruleRow(ruleName).click()
      await expect(authedPage.getByText('advanced routing rule')).toBeVisible()

      await euclid.ruleRow(ruleName).click()

      await expect(authedPage.getByText('advanced routing rule')).toHaveCount(0)
    })

    test('activates the rule', async ({ authedPage }) => {
      await euclid.ruleAction(ruleName, 'Activate')

      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible()
      await expect(euclid.ruleRow(ruleName).getByText('Active', { exact: true })).toBeVisible()
    })

    test('deactivates an active rule', async ({ authedPage }) => {
      await euclid.ruleAction(ruleName, 'Activate')
      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible()

      await euclid.ruleAction(ruleName, 'Deactivate')

      // Deactivation is behind a confirmation dialog — turning off live routing is not a stray click.
      await expect(authedPage.getByText('Deactivate this rule?')).toBeVisible()
      await authedPage.locator('.fixed.inset-0').getByRole('button', { name: 'Deactivate' }).click()

      await expect(authedPage.getByText('Rule deactivated successfully.')).toBeVisible()
      await expect(euclid.ruleRow(ruleName).getByText('Inactive')).toBeVisible()
    })

    test('blocks Edit and Delete while the rule is active', async ({ authedPage }) => {
      await euclid.ruleAction(ruleName, 'Activate')
      await expect(authedPage.getByText('Rule activated successfully.')).toBeVisible()

      // /routing/update and /routing/delete both reject an active algorithm.
      await euclid.openRuleMenu(ruleName)
      await expect(euclid.menuItem('Edit')).toBeDisabled()
      await expect(euclid.menuItem('Delete')).toBeDisabled()
    })

    test('edits an inactive rule and sends the update to the backend', async ({ authedPage }) => {
      const renamed = `${ruleName}-edited`

      await euclid.ruleAction(ruleName, 'Edit')
      await euclid.waitUntilReady()
      await expect(authedPage.getByRole('heading', { name: 'Edit Payment Rule' })).toBeVisible()
      await expect(authedPage.getByPlaceholder('my-rule')).toHaveValue(ruleName)

      await authedPage.getByPlaceholder('my-rule').fill(renamed)
      const call = expectApiCall(authedPage, '/routing/update')
      await authedPage.getByRole('button', { name: 'Save Changes' }).click()
      const { status, requestBody, body } = await call
      expect(status, `POST /routing/update failed: ${JSON.stringify(body)}`).toBe(200)
      expect(requestBody.name).toBe(renamed)

      await expect(euclid.ruleRow(renamed)).toBeVisible()
    })

    test('deletes an inactive rule', async ({ authedPage }) => {
      await euclid.ruleAction(ruleName, 'Delete')

      await expect(authedPage.getByText('Delete this rule?')).toBeVisible()
      await authedPage.locator('.fixed.inset-0').getByRole('button', { name: 'Delete' }).click()

      await expect(euclid.ruleRow(ruleName)).toHaveCount(0)
    })

    test('filters the list by name from the column header', async ({ authedPage }) => {
      // The header label is a button until it is clicked; then it becomes the filter input.
      await authedPage.getByLabel('Filter rules by name').click()
      await authedPage.getByLabel('Filter rules by name').fill('no-such-rule-exists')

      await expect(euclid.ruleRow(ruleName)).toHaveCount(0)
      await expect(authedPage.getByText('No rules match these filters.')).toBeVisible()
    })

    test('restores every row via Clear filters', async ({ authedPage }) => {
      await authedPage.getByLabel('Filter rules by name').click()
      await authedPage.getByLabel('Filter rules by name').fill('no-such-rule-exists')
      await expect(authedPage.getByText('No rules match these filters.')).toBeVisible()

      await authedPage.getByRole('button', { name: 'Clear filters' }).click()

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
    })

    test('filters the list by status from the column header', async ({ authedPage }) => {
      await authedPage.getByLabel('Filter by status').click()
      await authedPage.getByRole('button', { name: 'Active', exact: true }).click()

      await expect(euclid.ruleRow(ruleName)).toHaveCount(0)

      await authedPage.getByLabel('Filter by status').click()
      await authedPage.getByRole('button', { name: 'Inactive', exact: true }).click()

      await expect(euclid.ruleRow(ruleName)).toBeVisible()
    })

    test('names the active filter in the column header', async ({ authedPage }) => {
      await authedPage.getByLabel('Filter by status').click()
      await authedPage.getByRole('button', { name: 'Inactive', exact: true }).click()

      // A narrowed table has to say why it is short.
      await expect(authedPage.getByLabel('Filter by status')).toHaveText(/Status: Inactive/i)
    })

    test('returns to the list from the builder via Cancel', async ({ authedPage }) => {
      await authedPage.getByRole('button', { name: 'Create Rule' }).click()
      await euclid.waitUntilReady()
      await expect(authedPage.getByRole('heading', { name: 'Create Payment Rule' })).toBeVisible()

      await authedPage.getByRole('button', { name: 'Cancel' }).click()

      await expect(authedPage.getByRole('heading', { name: 'Rule-Based Routing' })).toBeVisible()
    })
  })
})
