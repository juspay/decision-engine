import { test, expect } from '../../fixtures/test'

/**
 * Port of cypress/e2e/ui/decision-explorer.cy.js.
 *
 * The Decision Explorer is the "try before you route" surface — four tabs, each simulating a different
 * routing mode against the merchant's real configuration. These assert each tab's controls render with
 * backend-valid defaults, plus one end-to-end run of the debit tab (the only mode whose result depends
 * on a merchant feature flag being on).
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Decision Explorer UI', () => {
  test.beforeEach(async ({ authedPage }) => {
    await authedPage.goto('/decisions')
    await expect(authedPage.getByRole('heading', { level: 1, name: 'Decision Explorer' })).toBeVisible()
    // Interacting before the routing config lands gives false failures on empty dropdowns.
    await expect(authedPage.getByText('Loading routing config from backend...')).toHaveCount(0)
  })

  // Each tab button renders a title and a subtitle, so its accessible name is both lines joined.
  // Matching the full name is what makes it unique — the bare title also matches other buttons on the
  // page (e.g. the always-visible 'Reset Auth-Rate Based Routing').
  test('renders the auth-rate simulation surface', async ({ authedPage }) => {
    await authedPage.getByRole('button', { name: 'Auth-rate Score simulation' }).click()

    await expect(authedPage.getByRole('button', { name: 'Run Auth-Rate Simulation' }).first()).toBeVisible()
    await expect(authedPage.getByText('Payments').first()).toBeVisible()
    await expect(authedPage.locator('input[type="range"]').first()).toBeVisible()
  })

  test('renders the rule-based evaluation surface', async ({ authedPage }) => {
    await authedPage.getByRole('button', { name: 'Rule based Policy evaluator' }).click()

    await expect(authedPage.getByRole('button', { name: 'Evaluate Rules' }).first()).toBeVisible()
    await expect(authedPage.getByText('Rule Evaluation Parameters').first()).toBeVisible()
    await expect(authedPage.getByText('Fallback Gateways').first()).toBeVisible()
    await expect(authedPage.getByPlaceholder('gateway name').first()).toBeVisible()
    await expect(authedPage.getByPlaceholder('gateway id (optional)').first()).toBeVisible()
    await expect(authedPage.getByText('Add Parameter').first()).toBeVisible()
  })

  test('renders the volume split evaluation surface', async ({ authedPage }) => {
    await authedPage.getByRole('button', { name: 'Volume split Distribution run' }).click()

    await expect(authedPage.getByText('Evaluation count').first()).toBeVisible()
    await authedPage.locator('input[type="number"]').first().fill('20')
    // The volume tab's run button is labelled just "Run" (the frozen Cypress spec still expects the
    // older "Run Volume Evaluation" wording), so match it exactly to keep it unique.
    await expect(authedPage.getByRole('button', { name: 'Run', exact: true })).toBeVisible()
    await expect(authedPage.getByText('Volume Split Configuration').first()).toBeVisible()
    await expect(authedPage.getByText('/routing/evaluate').first()).toBeVisible()
  })

  test('renders the debit routing surface with backend-valid defaults', async ({ authedPage }) => {
    await authedPage.getByRole('button', { name: 'Debit routing Network decision' }).click()

    await expect(authedPage.getByText('Debit Routing Parameters').first()).toBeVisible()
    await expect(authedPage.getByText('Debit routing is disabled.').first()).toBeVisible()
    await expect(authedPage.getByRole('button', { name: 'Enable Debit Routing' }).first()).toBeVisible()
    await expect(authedPage.locator('input[value="merchant_category_code_0001"]')).toBeVisible()
    await expect(authedPage.locator('input[value="VISA, NYCE, PULSE, STAR"]')).toBeVisible()
    // The run button stays disabled until the feature is enabled — no misleading empty results.
    await expect(authedPage.getByRole('button', { name: 'Run Debit Routing' }).first()).toBeDisabled()
  })

  test('runs debit routing through decide-gateway when enabled', async ({ api, authedPage, merchant }) => {
    await api.raw('POST', `/merchant-account/${merchant.id}/debit-routing`, { body: { enabled: true } })
    await authedPage.reload()
    await expect(authedPage.getByText('Loading routing config from backend...')).toHaveCount(0)

    await authedPage.getByRole('button', { name: 'Debit routing Network decision' }).click()
    await expect(authedPage.getByText('Debit routing is enabled.').first()).toBeVisible()

    await authedPage.getByRole('button', { name: 'Run Debit Routing' }).first().click()

    await expect(authedPage.getByText('Debit Routing Result').first()).toBeVisible({ timeout: 20_000 })
    await expect(authedPage.getByText('Ranked Debit Networks').first()).toBeVisible()
    await expect(authedPage.getByRole('cell', { name: 'VISA' }).first()).toBeVisible()
  })
})
