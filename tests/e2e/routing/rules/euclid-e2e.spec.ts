import { test, expect, factory } from '../../../fixtures/test'
import { EuclidRuleBuilder } from '../../../pages/euclid-page'
import { expectApiCall } from '../../../helpers/network'

/**
 * Port of cypress/e2e/ui/euclid-rules-e2e.cy.js.
 *
 * The highest-value Euclid tests: they drive the builder AND assert on the exact JSON payload it sends
 * to POST /routing/create. That's the contract between the rule editor and the routing engine — a UI
 * that renders correctly but emits `enum_variant` where the backend expects `enum_variant_array`
 * produces a rule that silently never matches, and only an assertion on the request body catches it.
 *
 * Per-test `merchant` fixture: every test here creates a rule.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('End-to-end creation', () => {
  let euclid: EuclidRuleBuilder
  let ruleName: string

  test.beforeEach(async ({ authedPage }) => {
    ruleName = factory.ruleName('adv_rule')
    euclid = new EuclidRuleBuilder(authedPage)
    await euclid.goto('/routing/rules/new')
    await euclid.addRuleBlock()
  })

  /** Submit and return the request body the UI built, failing loudly if the backend rejected it. */
  async function submitAndCapture(page: any) {
    const call = expectApiCall(page, '/routing/create')
    await page.getByRole('button', { name: 'Create Rule' }).click()
    const { status, requestBody, body } = await call
    expect(status, `POST /routing/create failed: ${JSON.stringify(body)}`).toBe(200)
    // A successful create returns to the rules list.
    await expect(page.getByRole('heading', { name: 'Rule-Based Routing' })).toBeVisible()
    return requestBody
  }

  const firstStatement = (requestBody: any) =>
    requestBody?.algorithm?.data?.rules?.[0]?.statements?.[0]

  test('creates a rule using "is one of" — backend receives enum_variant_array', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'payment_method')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is one of' })

    await euclid.selectMultiCondVals(0, ['card', 'bank_transfer'])

    // Confirm the UI reflects both selections before submitting.
    const value = authedPage.locator('[data-cy="cond-val"]').first()
    await expect(value.getByText('Card')).toBeVisible()
    await expect(value.getByText('Bank Transfer')).toBeVisible()

    await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
    await euclid.addFallbackGateway('adyen', 'mca_adyen')

    const requestBody = await submitAndCapture(authedPage)

    const condition = firstStatement(requestBody).condition[0]
    expect(condition.value.type).toBe('enum_variant_array')
    expect(condition.value.value).toHaveLength(2)
    expect(condition.comparison).toBe('equal')
  })

  test('creates a rule using "is not one of" — backend receives not_equal + enum_variant_array', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'payment_method')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is not one of' })

    await euclid.selectMultiCondVals(0, ['card'])

    await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
    await euclid.addFallbackGateway('adyen', 'mca_adyen')

    const requestBody = await submitAndCapture(authedPage)

    const condition = firstStatement(requestBody).condition[0]
    expect(condition.value.type).toBe('enum_variant_array')
    expect(condition.comparison).toBe('not_equal')
  })

  test('creates a rule with one nested AND+OR branch — backend receives nested array', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'amount')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'greater than' })
    await euclid.ruleBlock(0).locator('input[type="number"]').fill('10')

    await euclid.addNestedBranch(0)
    const branch = euclid.nestedBranch(0, 0)
    await euclid.selectCondLhs(0, 'payment_method', branch)
    await euclid.selectCondVal(0, 'card', branch)

    await euclid.addGatewayToBlock(0, 'rbl', 'mca_rbl')
    await euclid.addFallbackGateway('stripe', 'mca_stripe')

    const requestBody = await submitAndCapture(authedPage)

    const statement = firstStatement(requestBody)
    expect(statement.condition[0].lhs).toBe('amount')
    expect(statement.nested).toHaveLength(1)
  })

  test('creates a rule with two nested OR branches — backend receives nested array of length 2', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'amount')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'greater than' })
    await euclid.ruleBlock(0).locator('input[type="number"]').fill('10')

    await euclid.addNestedBranch(0)
    const branch = euclid.nestedBranch(0, 0)
    await euclid.selectCondLhs(0, 'payment_method', branch)
    await euclid.selectCondVal(0, 'card', branch)

    await euclid.addNestedBranch(0)
    const second = euclid.nestedBranch(0, 1)
    await euclid.selectCondLhs(0, 'currency', second)
    await euclid.selectCondVal(0, 'AED', second)

    await euclid.addGatewayToBlock(0, 'rbl', 'mca_rbl')
    await euclid.addFallbackGateway('stripe', 'mca_stripe')

    const requestBody = await submitAndCapture(authedPage)

    expect(firstStatement(requestBody).nested).toHaveLength(2)
  })

  test('creates a rule with volume split output — backend receives routing_type: volume_split', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'payment_method')
    await euclid.selectCondVal(0, 'card')

    await euclid.switchOutputType(0, 'Volume Split')
    await euclid.addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    await euclid.addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')
    await euclid.addFallbackGateway('checkout', 'mca_checkout')

    const requestBody = await submitAndCapture(authedPage)

    const rule = requestBody.algorithm.data.rules[0]
    expect(rule.routing_type).toBe('volume_split')
    expect(rule.output.volume_split).toHaveLength(2)
    expect(rule.output.volume_split[0].split).toBe(60)
    expect(rule.output.volume_split[0].output.gateway_name).toBe('stripe')
    expect(rule.output.volume_split[1].split).toBe(40)
    expect(rule.output.volume_split[1].output.gateway_name).toBe('adyen')
  })

  test('creates a rule combining nested AND+OR with volume split output', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'amount')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'greater than' })
    await euclid.ruleBlock(0).locator('input[type="number"]').fill('100')

    await euclid.addNestedBranch(0)
    const branch = euclid.nestedBranch(0, 0)
    await euclid.selectCondLhs(0, 'payment_method', branch)
    await euclid.selectCondVal(0, 'card', branch)

    await euclid.switchOutputType(0, 'Volume Split')
    await euclid.addVolumeSplitEntry(0, 70, 'stripe', 'mca_stripe')
    await euclid.addVolumeSplitEntry(0, 30, 'adyen', 'mca_adyen')
    await euclid.addFallbackGateway('checkout', 'mca_checkout')

    const requestBody = await submitAndCapture(authedPage)

    const rule = requestBody.algorithm.data.rules[0]
    expect(rule.routing_type).toBe('volume_split')
    expect(rule.statements[0].nested).toHaveLength(1)
    expect(rule.output.volume_split).toHaveLength(2)
  })

  test('creates a rule combining "is one of" with nested AND+OR', async ({ authedPage }) => {
    await authedPage.getByPlaceholder('my-rule').fill(ruleName)
    await euclid.selectCondLhs(0, 'payment_method')
    await euclid.ruleBlock(0).locator('select.cond-select').first().selectOption({ label: 'is one of' })
    await euclid.selectMultiCondVals(0, ['card', 'bank_transfer'])

    await euclid.addNestedBranch(0)
    const branch = euclid.nestedBranch(0, 0)
    await euclid.selectCondLhs(0, 'currency', branch)
    await euclid.selectCondVal(0, 'AED', branch)

    await euclid.addGatewayToBlock(0, 'stripe', 'mca_stripe')
    await euclid.addFallbackGateway('adyen', 'mca_adyen')

    const requestBody = await submitAndCapture(authedPage)

    const statement = firstStatement(requestBody)
    expect(statement.condition[0].value.type).toBe('enum_variant_array')
    expect(statement.nested).toHaveLength(1)
  })
})
