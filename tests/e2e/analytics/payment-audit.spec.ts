import { test, expect, factory } from '../../fixtures/test'
import { seedRoutedTraffic, waitForAuditFlowType, waitForAuditListing, waitForPreviewFlowType } from '../../helpers/seed'

/**
 * Port of cypress/e2e/ui/payment-audit.cy.js.
 *
 * The audit page is the support tool: given a payment id, show why it routed the way it did. It lists
 * one entry per payment with every audit event in its trail, and a "Routing type" filter (once the
 * page's tabs) narrows the list to Multi Objective, Auth Rate, rule-based or debit payments. Both the SR
 * decision and the rule-preview payment are exercised here because a broken lookup for either means an
 * operator cannot answer "why did this payment go to that PSP".
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Payment Audit UI', () => {
  test('searches transaction and rule-based audit trails from the UI', async ({
    api,
    authedPage,
    merchant,
  }) => {
    test.setTimeout(120_000)

    const seeded = await seedRoutedTraffic(api, merchant.id, {
      scoreStatus: 'FAILURE',
      prefix: 'audit',
    })

    // Both trails are populated asynchronously — wait for each before driving the UI at it.
    await waitForAuditFlowType(api, seeded.decisionPaymentId, 'decide_gateway_decision')
    await waitForPreviewFlowType(api, seeded.previewPaymentId!, 'routing_evaluate_advanced')

    // NOTE: the Cypress original asserts on "Search Decision Trail" / "Search Rule Decision Trail".
    // Those strings still exist in the page's content object but are no longer rendered anywhere, so
    // this asserts on the heading and the search input, which are what a user actually sees. The
    // search box is addressed by its accessible name, which survives placeholder copy changes.
    await authedPage.goto('/audit')

    await expect(authedPage.getByRole('heading', { level: 1, name: 'Decision Audit' })).toBeVisible()
    // The tabs are gone; the routing type is a filter next to gateway and status.
    await expect(authedPage.getByRole('button', { name: 'Multi-objective' })).toHaveCount(0)
    const routingType = authedPage.getByLabel('Routing type')
    await expect(routingType).toHaveValue('')

    await authedPage.getByLabel('Payment ID', { exact: true }).fill(seeded.decisionPaymentId)
    await authedPage.keyboard.press('Enter')

    await expect(authedPage.getByText(seeded.decisionPaymentId).first()).toBeVisible({ timeout: 20_000 })

    // Rule-based decisions live in the same list; the routing-type filter narrows to them.
    await routingType.selectOption('rule_based')
    const search = authedPage.getByLabel('Payment ID', { exact: true })
    await search.fill(seeded.previewPaymentId!)
    await authedPage.keyboard.press('Enter')

    await expect(authedPage.getByText(seeded.previewPaymentId!).first()).toBeVisible({ timeout: 20_000 })

    // Links written when the page had tabs still land on the matching filter.
    await authedPage.goto('/audit?mode=rule_based')
    await expect(authedPage.getByLabel('Routing type')).toHaveValue('rule_based', { timeout: 20_000 })
  })
})

test.describe('Payment Audit UI payment field filter', () => {
  test('narrows the list by a payment field and keeps it in the URL', async ({ api, authedPage, merchant }) => {
    test.setTimeout(120_000)
    const decide = (paymentId: string, bin: string) =>
      api.raw('POST', '/decide', { body: {
        payment_id: paymentId, merchant_id: merchant.id, amount: 1000, currency: 'USD',
        payment_method_type: 'card', payment_method_sub_type: 'credit', card: { card_isin: bin },
        routing_config: { algorithm_for: 'payment', eligible_connectors: [{ gateway_name: 'stripe', payment_merchant_connector_id: 'mca_stripe' }] },
      } })
    const matching = factory.paymentId('ui_fields_match')
    const other = factory.paymentId('ui_fields_other')
    expect((await decide(matching, '424242')).status).toBe(200)
    expect((await decide(other, '555555')).status).toBe(200)
    // The list reads per-payment summaries, which land shortly after the events.
    await waitForAuditListing(api, matching)
    await waitForAuditListing(api, other)

    await authedPage.goto('/audit?range=1h')
    await expect(authedPage.getByText(other).first()).toBeVisible({ timeout: 20_000 })
    await authedPage.getByRole('button', { name: 'More filters' }).click()
    await authedPage.locator('[data-cy="payment-field-name"] button').click()
    await authedPage.getByPlaceholder('Search…').fill('card_bin')
    await authedPage.locator('button[data-value="card_bin"]').click()
    await authedPage.getByLabel('Payment field value').fill('424242')
    await authedPage.getByRole('button', { name: 'Add field' }).click()

    await expect(authedPage).toHaveURL(/payment_fields=/)
    await expect(authedPage.getByText(matching).first()).toBeVisible({ timeout: 20_000 })
    await expect(authedPage.getByText(other)).toHaveCount(0)

    // The filter is restored from the URL, and removing it lists every payment again.
    await authedPage.reload()
    await authedPage.getByRole('button', { name: 'More filters' }).click()
    await expect(authedPage.getByText('card_bin = 424242')).toBeVisible()
    await expect(authedPage.getByText(other)).toHaveCount(0)
    await authedPage.getByRole('button', { name: 'Remove card_bin filter' }).click()
    await expect(authedPage).not.toHaveURL(/payment_fields=/)
    await expect(authedPage.getByText(other).first()).toBeVisible({ timeout: 20_000 })
  })
})
