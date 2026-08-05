import { test, expect } from '../../fixtures/test'
import { stubApi } from '../../helpers/stub'

/**
 * Port of cypress/e2e/ui/auth-page.cy.js.
 *
 * Uses the plain `page` fixture, not `authedPage` — Playwright gives every test a fresh browser
 * context, so the Cypress `onBeforeLoad(win) { win.localStorage.removeItem('auth-store') }` boilerplate
 * is unnecessary and is deliberately NOT ported.
 *
 * DEVIATION FROM SOURCE: the Cypress spec's first test renders /login unauthenticated and then visits /
 * with a session in the same test. `addInitScript` is per-page and permanent, so one page cannot do
 * both — it is split into two tests here. A fourth test (AuthGuard redirect) is new; the Cypress suite
 * never covered it.
 */

test.describe('Auth UI', () => {
  test('renders the login page when unauthenticated', async ({ page }) => {
    await page.goto('/login')

    await expect(
      page.getByRole('heading', { name: 'Manage routing, analytics, and audits from one dashboard.' }),
    ).toBeVisible()
    await expect(page.getByText('Welcome back')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Enter dashboard' })).toBeVisible()
  })

  test('renders the dashboard for a seeded session', async ({ authedPage, merchant }) => {
    await authedPage.goto('/')

    await expect(authedPage.getByText(`${merchant.id}@example.com`)).toBeVisible({ timeout: 20_000 })
    await expect(authedPage.getByText(merchant.id).first()).toBeVisible()
  })

  test('redirects an unauthenticated visit to a protected route back to login', async ({ page }) => {
    await page.goto('/routing')

    // AuthGuard has no token to validate, so the dashboard must never render.
    await expect(page).toHaveURL(/\/login/)
  })

  test('keeps the sign-up tab active across refresh', async ({ page }) => {
    await page.goto('/login')

    await page.getByRole('button', { name: 'Sign up' }).click()
    await expect(page).toHaveURL(/\/signup/)
    await expect(page.getByText('Create account').first()).toBeVisible()

    await page.reload()

    // The tab is encoded in the URL, so a refresh must not silently drop the user back to sign-in.
    await expect(page).toHaveURL(/\/signup/)
    await expect(page.getByRole('button', { name: 'Create account' })).toBeVisible()
  })

  test('switches duplicate sign-up attempts to sign-in with email preserved', async ({ page }) => {
    const duplicateEmail = 'duplicate-user@example.com'

    // Register the stub BEFORE navigating, or the route handler misses the request.
    await stubApi(page, '**/auth/signup', {
      status: 409,
      body: { message: 'Email already registered' },
    })

    await page.goto('/login')
    await page.getByRole('button', { name: 'Sign up' }).click()

    await page.locator('input[type="email"]').fill(duplicateEmail)
    await page.getByPlaceholder('e.g. Acme Corp').fill('Venom')
    await page.getByPlaceholder('Enter your password').fill('ValidPass1!')
    await page.getByRole('button', { name: 'Create account' }).click()

    // The recovery is the point: an existing account should drop the user into sign-in with their
    // email already filled in and the cursor in the password field, not strand them on an error.
    await expect(page.getByText('Welcome back')).toBeVisible()
    await expect(page.getByText('Account already exists. Sign in with this email.')).toBeVisible()
    await expect(page.locator('input[type="email"]')).toHaveValue(duplicateEmail)
    await expect(page.getByPlaceholder('Enter your password')).toBeFocused()
  })
})
