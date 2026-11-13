import type { Page } from '@playwright/test'
import type { Session } from './session'

/**
 * Port of Cypress `seedDashboardStorage` (commands.js). Injects the zustand-persisted auth and
 * merchant stores into localStorage BEFORE the app's scripts run, so the dashboard boots
 * authenticated (AuthGuard validates the seeded token via GET /auth/me) with the merchant context
 * already selected.
 *
 * `page.addInitScript` runs on every navigation before page scripts, so call this once before the
 * first `page.goto()`.
 */
export async function seedDashboardStorage(page: Page, merchantId: string, session: Session): Promise<void> {
  await page.addInitScript(
    ({ merchantId, session }) => {
      window.localStorage.setItem(
        'merchant-store',
        JSON.stringify({ state: { merchantId }, version: 0 }),
      )
      window.localStorage.setItem(
        'auth-store',
        JSON.stringify({ state: { token: session.token, user: session.user }, version: 0 }),
      )
    },
    { merchantId, session },
  )
}
