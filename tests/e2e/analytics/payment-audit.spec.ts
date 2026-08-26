import { test, expect } from '../../fixtures/test'
import { seedRoutedTraffic, waitForAuditFlowType, waitForPreviewFlowType } from '../../helpers/seed'

/**
 * Port of cypress/e2e/ui/payment-audit.cy.js.
 *
 * The audit page is the support tool: given a payment id, show why it routed the way it did. It has two
 * modes reading two different traces — the live decision trail (`/analytics/payment-audit`) and the
 * rule-preview trail (`/analytics/preview-trace`) — and both are exercised here because a broken lookup
 * in either one means an operator cannot answer "why did this payment go to that PSP".
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
    await authedPage.getByLabel('Payment ID', { exact: true }).fill(seeded.decisionPaymentId)
    await authedPage.keyboard.press('Enter')

    await expect(authedPage.getByText(seeded.decisionPaymentId).first()).toBeVisible({ timeout: 20_000 })

    // Rule-based mode is a separate view reading the preview trace instead of the decision trail.
    await authedPage.goto('/audit?mode=rule_based')

    const ruleSearch = authedPage.getByLabel('Decision payment ID', { exact: true })
    await expect(ruleSearch).toBeVisible({ timeout: 20_000 })
    await ruleSearch.fill(seeded.previewPaymentId!)
    await authedPage.keyboard.press('Enter')

    await expect(authedPage.getByText(seeded.previewPaymentId!).first()).toBeVisible({ timeout: 20_000 })
  })
})
