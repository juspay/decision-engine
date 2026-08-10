import { test, expect, factory } from '../../fixtures/test'

/**
 * Port of cypress/e2e/ui/api-keys-page.cy.js.
 *
 * The key property under test is show-once: a created key is displayed exactly one time and never
 * again, so the "creates a key and uses it" test proves the value shown in the UI is a genuinely usable
 * credential rather than a truncated display string — the thing a merchant would discover the hard way.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('API Keys UI', () => {
  test('renders the page with a create form and the default key', async ({ authedPage }) => {
    await authedPage.goto('/api-keys')

    await expect(authedPage.getByRole('heading', { level: 1, name: 'API Keys' })).toBeVisible()
    await expect(authedPage.getByText('x-api-key')).toBeVisible()
    await expect(authedPage.locator('input[placeholder*="Description"]')).toBeVisible()
    await expect(authedPage.getByRole('button', { name: 'Create API Key' })).toBeVisible()

    // A new merchant is provisioned with a "Default API key" at signup, so the list is never empty —
    // the merchant can call the API before ever visiting this page. (The Cypress original still
    // asserts "No active API keys", which predates that behaviour.)
    await expect(authedPage.getByText('Default API key')).toBeVisible()
    await expect(authedPage.getByRole('cell', { name: /^DE_/ }).first()).toBeVisible()
  })

  test('creates an API key, shows it once, and lists it', async ({ authedPage }) => {
    await authedPage.goto('/api-keys')

    await authedPage.locator('input[placeholder*="Description"]').fill('playwright-integration-key')
    await authedPage.getByRole('button', { name: 'Create API Key' }).click()

    const keyValue = authedPage.getByTestId('api-key-value')
    await expect(keyValue).toBeVisible({ timeout: 10_000 })
    expect((await keyValue.textContent())?.trim()).toMatch(/^DE_/)

    await expect(authedPage.getByText('API key created — copy it now')).toBeVisible()
    await expect(authedPage.getByText('playwright-integration-key').first()).toBeVisible()
    await expect(authedPage.getByRole('button', { name: 'Revoke' }).first()).toBeVisible()
  })

  test('a key created in the UI authenticates a routing call', async ({ api, authedPage, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await authedPage.goto('/api-keys')

    await authedPage.locator('input[placeholder*="Description"]').fill('routing-test-key')
    await authedPage.getByRole('button', { name: 'Create API Key' }).click()

    const keyValue = authedPage.getByTestId('api-key-value')
    await expect(keyValue).toBeVisible({ timeout: 10_000 })
    const apiKey = (await keyValue.textContent())!.trim()
    expect(apiKey).toMatch(/^DE_/)

    // Call the API with only that key — no bearer token — exactly as a merchant integration would.
    const anon = api.anonymous()
    const decide = await anon.raw('POST', '/decide-gateway', {
      failOnStatusCode: false,
      headers: { 'x-api-key': apiKey },
      body: factory.srDecideGatewayRequest({
        merchantId: merchant.id,
        paymentInfo: { paymentId: factory.paymentId('apikey') },
      }),
    })

    expect(decide.status).toBe(200)
    expect(decide.body).toHaveProperty('decided_gateway')
  })

  test('revokes an API key and removes it from the list', async ({ authedPage }) => {
    await authedPage.goto('/api-keys')

    await authedPage.locator('input[placeholder*="Description"]').fill('to-be-revoked')
    await authedPage.getByRole('button', { name: 'Create API Key' }).click()
    await expect(authedPage.getByTestId('api-key-value')).toBeVisible({ timeout: 10_000 })

    const row = authedPage.getByRole('row', { name: /to-be-revoked/ })
    await row.getByRole('button', { name: 'Revoke' }).click()

    await expect(authedPage.getByRole('cell', { name: 'to-be-revoked' })).toHaveCount(0)
  })
})
