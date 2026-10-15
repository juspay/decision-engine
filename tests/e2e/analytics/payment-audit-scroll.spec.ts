import type { Locator } from '@playwright/test'
import { test, expect, factory } from '../../fixtures/test'
import { poll } from '../../helpers/poll'

/**
 * Leaving a payment's trail returns to the results list where it was left. Operators work down a
 * page of up to 100 payments; reopening the list at its top made them scroll back to find their
 * place after every payment they looked at.
 */

const PAYMENTS = 40

test.use({ viewport: { width: 1600, height: 900 } })

/** How far the element that scrolls the page (the app shell's scroller) is scrolled. */
function scrollOffset(row: Locator) {
  return row.evaluate((element) => {
    for (let node = element.parentElement; node; node = node.parentElement) {
      const { overflowY } = getComputedStyle(node)
      if ((overflowY === 'auto' || overflowY === 'scroll') && node.scrollHeight > node.clientHeight) {
        return node.scrollTop
      }
    }
    return document.scrollingElement?.scrollTop ?? 0
  })
}

test.describe('Payment Audit UI scroll position', () => {
  test('returns to the payment it opened, not the top of the list', async ({
    api,
    authedPage,
    merchant,
  }) => {
    test.setTimeout(120_000)
    const m = merchant.id
    await api.createSuccessRateConfig(m)

    // Two events each (decision and score update), so opening one shows its trail.
    const paymentIds = Array.from({ length: PAYMENTS }, (_, i) => factory.paymentId(`audit_scroll_${i}`))
    for (const paymentId of paymentIds) {
      const decided = await api.decideGateway(
        factory.srDecideGatewayRequest({ merchantId: m, paymentInfo: { paymentId } }),
      )
      await api.updateGatewayScore(
        factory.updateGatewayScoreRequest({
          merchantId: m,
          gateway: decided.body.decided_gateway,
          paymentId,
          status: 'CHARGED',
        }),
      )
    }
    await poll(
      () => api.raw('GET', '/analytics/payment-audit', { qs: { range: '1d', page: 1, page_size: 50 } }),
      (r) =>
        r.body.total_results === PAYMENTS &&
        r.body.results.every((row: any) => row.event_count === 2),
      { message: 'payments did not reach the audit', timeout: 40_000 },
    )

    await authedPage.goto('/audit?range=1d&page_size=50')
    const backToResults = authedPage.getByRole('button', { name: 'Back to results' })
    // A payment from the middle of the list, well below the fold.
    const middle = authedPage.getByRole('button', { name: new RegExp(paymentIds[PAYMENTS / 2]) })
    await middle.scrollIntoViewIfNeeded()
    const offset = await scrollOffset(middle)
    expect(offset).toBeGreaterThan(0)

    await middle.click()
    await expect(backToResults).toBeVisible()
    await backToResults.click()
    await expect(middle).toBeInViewport()
    expect(await scrollOffset(middle)).toBe(offset)

    // The browser's back button returns to the same place.
    await middle.click()
    await expect(backToResults).toBeVisible()
    await authedPage.goBack()
    await expect(backToResults).toBeHidden()
    await expect(middle).toBeInViewport()
    expect(await scrollOffset(middle)).toBe(offset)
  })
})
