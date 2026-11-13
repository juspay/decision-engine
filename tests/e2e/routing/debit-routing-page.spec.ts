import { test, expect } from '../../fixtures/test'

/**
 * Port of cypress/e2e/ui/debit-routing-page.cy.js.
 *
 * Debit routing is runtime-access only from the dashboard: the operator flips a flag, and the network
 * configuration itself (MCC, accepted networks) is NOT editable here. The negative assertions are the
 * point — they pin that the config-editing UI stays absent, since exposing it would imply the
 * dashboard can change routing behaviour it does not actually own.
 */

test.use({ viewport: { width: 1600, height: 1200 } })

test.describe('Debit Routing UI', () => {
  test('toggles merchant debit routing access without exposing unsupported config editing', async ({
    api,
    authedPage,
    merchant,
  }) => {
    await authedPage.goto('/routing/debit')

    await expect(
      authedPage.getByRole('heading', { level: 1, name: 'Network / Debit Routing' }),
    ).toBeVisible()
    await expect(authedPage.getByText('Debit Routing Runtime Access')).toBeVisible()
    await expect(authedPage.getByText('Save Config')).toHaveCount(0)
    await expect(authedPage.getByText('Merchant Category Code (MCC)')).toHaveCount(0)

    await authedPage.getByRole('button', { name: 'Enable Debit Routing' }).click()
    await expect(authedPage.getByText('Debit routing enabled.')).toBeVisible()

    // Confirm the toggle actually persisted server-side, not just in local UI state.
    let flag = await api.raw('GET', `/merchant-account/${merchant.id}/debit-routing`)
    expect(flag.body.debit_routing_enabled).toBe(true)

    await authedPage.getByRole('button', { name: 'Disable Debit Routing' }).click()
    await expect(authedPage.getByText('Debit routing disabled.')).toBeVisible()

    flag = await api.raw('GET', `/merchant-account/${merchant.id}/debit-routing`)
    expect(flag.body.debit_routing_enabled).toBe(false)
  })
})
