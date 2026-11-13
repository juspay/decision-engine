import { test as base, expect } from '@playwright/test'
import type { Page } from '@playwright/test'
import factory from './factory'
import { ApiClient } from './api-client'
import { ensureDashboardSession, clearSession, type Session } from './session'
import { seedDashboardStorage } from './storage'
import { poll } from '../helpers/poll'

export type Fixtures = {
  /** Authenticated API client bound to this test's isolated request context. */
  api: ApiClient
  /** A freshly created merchant + dashboard session, auto-cleaned after the test. */
  merchant: { id: string; session: Session }
  /** A page pre-seeded with the merchant's auth — protected routes render without a login flow. */
  authedPage: Page
  /** `page` seeded with the SHARED (worker) merchant's auth. Pairs with `sharedMerchant`. */
  sharedPage: Page
}

export type WorkerFixtures = {
  /**
   * One merchant per worker process, created once and reused by every test that worker runs.
   *
   * ONLY for specs that never mutate merchant-scoped backend state — no rule creation, no config
   * writes, no feature-flag toggles. A mutating test would leak state into every later test on the
   * same worker. Everything else uses the per-test `merchant` fixture, and a spec file should pick one
   * or the other, never both (mixing gives two ApiClients with different tokens in one test).
   *
   * Motivation: a per-test merchant costs three round trips, dominated by the signup's bcrypt hash
   * (DEFAULT_COST = 12) against a debug build. Sharing across the pure-UI specs turns ~63 signups into
   * one per worker. The browser context is still fresh per test — only the merchant is shared.
   */
  sharedMerchant: { id: string; session: Session; api: ApiClient }
}

export const test = base.extend<Fixtures, WorkerFixtures>({
  api: async ({ request }, use) => {
    await use(new ApiClient(request))
  },

  merchant: async ({ api }, use) => {
    const id = factory.merchantId('pw')
    await api.ensureMerchantAccount(id)
    const session = await ensureDashboardSession(api, id)
    await use({ id, session })
    // Teardown: best-effort cleanup so parallel runs don't accumulate merchants.
    await api.cleanupTestData(id)
    clearSession(id)
  },

  authedPage: async ({ page, merchant }, use) => {
    await seedDashboardStorage(page, merchant.id, merchant.session)
    await use(page)
  },

  sharedMerchant: [
    async ({ playwright }, use, workerInfo) => {
      // Worker-scoped fixtures cannot depend on the test-scoped `request`, so build our own context.
      // ApiClient resolves absolute URLs itself, so no baseURL is needed here.
      const request = await playwright.request.newContext({
        extraHTTPHeaders: { 'x-tenant-id': 'public' },
      })
      const api = new ApiClient(request)
      // parallelIndex in the id makes a CI failure traceable back to a specific worker.
      const id = factory.merchantId(`pww${workerInfo.parallelIndex}`)
      await api.ensureMerchantAccount(id)
      const session = await ensureDashboardSession(api, id)

      await use({ id, session, api })

      await api.cleanupTestData(id)
      clearSession(id)
      await request.dispose()
    },
    { scope: 'worker' },
  ],

  sharedPage: async ({ page, sharedMerchant }, use) => {
    await seedDashboardStorage(page, sharedMerchant.id, sharedMerchant.session)
    await use(page)
  },
})

export { expect, factory, poll }
