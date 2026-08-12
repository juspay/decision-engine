import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'
import { SUPER_ADMIN_EMAIL, SUPER_ADMIN_PASSWORD } from '../../fixtures/super-admin'

/**
 * Platform super-admin: viewing ANY merchant's dashboard, plus the merchant lookup.
 *
 * Shape: a super-admin is a real (standard-session) user whose email is on the config roster
 * (`user_auth.super_admin_emails`). From that session they can mint a `super_admin_view` token for
 * any merchant — regardless of membership — that keeps their own identity, and later `exit` back to a
 * normal session. A view session is deliberately NOT a full account session (it cannot switch/create
 * merchants). Lookup lets them find a merchant id from an email or merchant name.
 *
 * Roster dependency: the API under test must have SUPER_ADMIN_EMAIL on its super-admin roster.
 * playwright.config.ts injects it via DECISION_ENGINE__USER_AUTH__SUPER_ADMIN_EMAILS into the API
 * server it starts, so nothing is hardcoded in shipped config. The per-test `merchant` fixture's own
 * admin session is a standard session that is NOT on the roster, so it doubles as the negative
 * (non-super-admin) case.
 */

const bearer = (token: string) => ({ headers: { Authorization: `Bearer ${token}` } })

/**
 * A standard-session token for the rostered super-admin. Login-first so repeated runs reuse the
 * global user; create it on the very first run, tolerating a concurrent creator from another worker.
 */
async function superAdminToken(api: ApiClient): Promise<string> {
  const login = await api.raw('POST', '/auth/login', {
    failOnStatusCode: false,
    body: { email: SUPER_ADMIN_EMAIL, password: SUPER_ADMIN_PASSWORD },
  })
  if (login.status === 200 && login.body?.token) return login.body.token

  const signup = await api.raw('POST', '/auth/signup', {
    failOnStatusCode: false,
    body: { email: SUPER_ADMIN_EMAIL, password: SUPER_ADMIN_PASSWORD },
  })
  if (signup.status === 200 && signup.body?.token) return signup.body.token

  // Lost a creation race with another worker — the account now exists, so login succeeds.
  const retry = await api.raw('POST', '/auth/login', {
    body: { email: SUPER_ADMIN_EMAIL, password: SUPER_ADMIN_PASSWORD },
  })
  return retry.body.token
}

test.describe('Super-admin merchant view (API)', () => {
  test('a rostered super-admin can enter any merchant, keeping their own identity', async ({ api, merchant }) => {
    const superToken = await superAdminToken(api)

    // The super-admin is not a member of this merchant — entry is by roster, not membership.
    const entered = await api.raw('POST', '/auth/super-admin/enter-merchant', {
      failOnStatusCode: false,
      ...bearer(superToken),
      body: { merchant_id: merchant.id },
    })

    expect(entered.status).toBe(200)
    expect(typeof entered.body.token).toBe('string')
    expect(entered.body.merchant_id).toBe(merchant.id)
    expect(entered.body.role).toBe('admin')
    // Real identity is preserved — not a synthetic hs_ redirect user.
    expect(entered.body.user_id).not.toMatch(/^hs_/)

    // /auth/me on the view token reports it as a super-admin view of the target merchant.
    const me = await api.raw('GET', '/auth/me', { failOnStatusCode: false, ...bearer(entered.body.token) })
    expect(me.status).toBe(200)
    expect(me.body.merchant_id).toBe(merchant.id)
    expect(me.body.is_super_admin).toBe(true)
    expect(me.body.is_super_admin_view).toBe(true)
  })

  test('a standard session that is not on the roster is forbidden', async ({ api, merchant }) => {
    // `api` carries the merchant's own admin session — a standard session, but not a super-admin.
    const r = await api.raw('POST', '/auth/super-admin/enter-merchant', {
      failOnStatusCode: false,
      body: { merchant_id: merchant.id },
    })

    expect(r.status).toBe(403)
  })

  test('entering an unknown merchant is rejected', async ({ api }) => {
    const superToken = await superAdminToken(api)

    const r = await api.raw('POST', '/auth/super-admin/enter-merchant', {
      failOnStatusCode: false,
      ...bearer(superToken),
      body: { merchant_id: factory.merchantId('sa_missing') },
    })

    expect(r.status).toBe(404)
  })

  test('a view session cannot switch merchant, and exit returns a full session', async ({ api, merchant }) => {
    const superToken = await superAdminToken(api)
    const entered = await api.raw('POST', '/auth/super-admin/enter-merchant', {
      ...bearer(superToken),
      body: { merchant_id: merchant.id },
    })
    const viewToken = entered.body.token

    // A view session is scoped to viewing — not a full account session.
    const switched = await api.raw('POST', '/auth/switch-merchant', {
      failOnStatusCode: false,
      ...bearer(viewToken),
      body: { merchant_id: merchant.id },
    })
    expect(switched.status).toBe(403)

    // Exit re-mints a standard session for the same admin.
    const exited = await api.raw('POST', '/auth/super-admin/exit', { failOnStatusCode: false, ...bearer(viewToken) })
    expect(exited.status).toBe(200)
    expect(typeof exited.body.token).toBe('string')

    const me = await api.raw('GET', '/auth/me', { failOnStatusCode: false, ...bearer(exited.body.token) })
    expect(me.status).toBe(200)
    expect(me.body.is_super_admin).toBe(true)
    expect(me.body.is_super_admin_view).toBe(false)
  })

  test('exit is rejected for a normal (non-view) session', async ({ api }) => {
    const superToken = await superAdminToken(api)

    // The super-admin's own standard token is not a view session — nothing to exit from.
    const r = await api.raw('POST', '/auth/super-admin/exit', { failOnStatusCode: false, ...bearer(superToken) })
    expect(r.status).toBe(403)
  })
})

test.describe('Super-admin merchant lookup (API)', () => {
  test('finds a merchant by its id and lists its members', async ({ api, merchant }) => {
    const superToken = await superAdminToken(api)

    const r = await api.raw('POST', '/auth/super-admin/lookup', {
      failOnStatusCode: false,
      ...bearer(superToken),
      body: { query: merchant.id },
    })

    expect(r.status).toBe(200)
    expect(Array.isArray(r.body)).toBe(true)
    const hit = r.body.find((m: any) => m.merchant_id === merchant.id)
    expect(hit, `expected ${merchant.id} in lookup results`).toBeTruthy()
    // The merchant's signing-up admin shows up as a member.
    expect(hit.members.some((mem: any) => mem.email === `${merchant.id}@example.com`)).toBe(true)
  })

  test('finds a merchant by a member email', async ({ api, merchant }) => {
    const superToken = await superAdminToken(api)

    const r = await api.raw('POST', '/auth/super-admin/lookup', {
      failOnStatusCode: false,
      ...bearer(superToken),
      body: { query: `${merchant.id}@example.com` },
    })

    expect(r.status).toBe(200)
    expect(r.body.some((m: any) => m.merchant_id === merchant.id)).toBe(true)
  })

  test('lookup requires a super-admin', async ({ api, merchant }) => {
    // Uses the merchant's own admin session — a standard session, not on the roster.
    const r = await api.raw('POST', '/auth/super-admin/lookup', {
      failOnStatusCode: false,
      body: { query: merchant.id },
    })

    expect(r.status).toBe(403)
  })

  test('an empty query returns no results', async ({ api }) => {
    const superToken = await superAdminToken(api)

    const r = await api.raw('POST', '/auth/super-admin/lookup', {
      failOnStatusCode: false,
      ...bearer(superToken),
      body: { query: '   ' },
    })

    expect(r.status).toBe(200)
    expect(r.body).toEqual([])
  })
})
