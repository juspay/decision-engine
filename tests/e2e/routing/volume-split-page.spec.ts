import { test, expect } from '../../fixtures/test'

/**
 * UI-journey port of cypress/e2e/ui/volume-split-page.cy.js.
 *
 * `authedPage` (page pre-seeded with the merchant's auth) replaces Cypress `visitWithMerchant`;
 * the `merchant` fixture creates + cleans the account. `cy.intercept(...).as()` + `cy.wait('@...')`
 * becomes `page.waitForResponse(...)`.
 */
test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Volume Split (UI)', () => {
  test('creates and activates a volume split rule from the page', async ({ authedPage, merchant }) => {
    const page = authedPage

    await page.goto('/routing/volume')

    await expect(page.getByRole('heading', { level: 1, name: 'Volume Split Routing' })).toBeVisible()
    await page.getByPlaceholder('e.g. ab-test-split').fill('ui-volume-split')

    await page.getByPlaceholder('e.g. stripe').nth(0).fill('stripe')
    await page.getByPlaceholder('optional gateway_id').nth(0).fill('mca_stripe_ui')
    await page.getByPlaceholder('e.g. stripe').nth(1).fill('adyen')
    await page.getByPlaceholder('optional gateway_id').nth(1).fill('mca_adyen_ui')

    const createResponse = page.waitForResponse(
      (r) => r.url().includes('/routing/create') && r.request().method() === 'POST',
      { timeout: 20000 },
    )
    await page.getByRole('button', { name: 'Create Rule' }).click()
    await createResponse

    // On success the component shows "Rule created: <id>" + an "Activate Now" button.
    await expect(page.getByText('Rule created:')).toBeVisible({ timeout: 15000 })
    await page.getByRole('button', { name: 'Activate Now' }).click()
    await expect(page.getByText('Rule activated.')).toBeVisible({ timeout: 15000 })
    await expect(page.getByText('Saved Rules').first()).toBeVisible()
  })
})
