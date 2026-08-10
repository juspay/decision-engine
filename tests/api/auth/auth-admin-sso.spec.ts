import { test, expect, factory } from '../../fixtures/test'

/**
 * The HyperSwitch → Decision Engine merchant SSO handoff (PR #331).
 *
 * Shape: an admin-secret-authenticated caller mints a short-lived one-time CODE for a merchant, and
 * the dashboard redeems that code for a session token. The token is only ever returned in the exchange
 * RESPONSE BODY — it never travels in a URL — and the code is single-use, which is the property this
 * spec exists to hold onto.
 *
 * `ApiClient` already sends `x-admin-secret` on every request (defaulting to `test_admin`, matching
 * config/development.toml), so only the negative case has to set the header explicitly.
 */
test.describe('Admin merchant-token SSO (API)', () => {
  test('mints a one-time code that exchanges for a session token', async ({ api, merchant }) => {
    const minted = await api.raw('POST', '/auth/admin/merchant-token', {
      failOnStatusCode: false,
      body: { merchant_id: merchant.id },
    })

    expect(minted.status).toBe(200)
    // The code is generated with the same helper as an API key, hence the DE_ prefix + 64 hex chars.
    expect(minted.body.code).toMatch(/^DE_[0-9a-f]{64}$/)
    expect(minted.body.expires_in).toBe(60)

    const exchanged = await api.raw('POST', '/auth/admin/merchant-token/exchange', {
      failOnStatusCode: false,
      body: { code: minted.body.code },
    })

    expect(exchanged.status).toBe(200)
    expect(typeof exchanged.body.token).toBe('string')
    expect(exchanged.body.merchant_id).toBe(merchant.id)
    expect(exchanged.body.role).toBe('admin')
    // The redirect session is synthetic — no real user row behind it.
    expect(exchanged.body.user_id).toBe(`hs_${merchant.id}`)
  })

  test('a code cannot be redeemed twice', async ({ api, merchant }) => {
    const minted = await api.raw('POST', '/auth/admin/merchant-token', {
      body: { merchant_id: merchant.id },
    })

    const first = await api.raw('POST', '/auth/admin/merchant-token/exchange', {
      failOnStatusCode: false,
      body: { code: minted.body.code },
    })
    expect(first.status).toBe(200)

    // This is the security property: the claim is atomic, so a replayed code is dead.
    const second = await api.raw('POST', '/auth/admin/merchant-token/exchange', {
      failOnStatusCode: false,
      body: { code: minted.body.code },
    })
    expect(second.status).toBe(401)
  })

  test('an unknown code is rejected', async ({ api }) => {
    const r = await api.raw('POST', '/auth/admin/merchant-token/exchange', {
      failOnStatusCode: false,
      body: { code: `DE_${'0'.repeat(64)}` },
    })

    expect(r.status).toBe(401)
  })

  test('minting requires the admin secret', async ({ api, merchant }) => {
    const r = await api.raw('POST', '/auth/admin/merchant-token', {
      failOnStatusCode: false,
      headers: { 'x-admin-secret': 'definitely-not-the-admin-secret' },
      body: { merchant_id: merchant.id },
    })

    expect(r.status).toBe(401)
  })

  test('minting for an unknown merchant is rejected', async ({ api }) => {
    const r = await api.raw('POST', '/auth/admin/merchant-token', {
      failOnStatusCode: false,
      body: { merchant_id: factory.merchantId('sso_missing') },
    })

    expect(r.status).toBe(404)
  })

  test('a redirect session can read its own identity but not switch merchant', async ({ api, merchant }) => {
    const minted = await api.raw('POST', '/auth/admin/merchant-token', {
      body: { merchant_id: merchant.id },
    })
    const exchanged = await api.raw('POST', '/auth/admin/merchant-token/exchange', {
      body: { code: minted.body.code },
    })
    const redirectToken = exchanged.body.token

    const me = await api.raw('GET', '/auth/me', {
      failOnStatusCode: false,
      headers: { Authorization: `Bearer ${redirectToken}` },
    })
    expect(me.status).toBe(200)
    expect(me.body.merchant_id).toBe(merchant.id)

    // A redirect session is deliberately restricted — it is not a full user account.
    const switched = await api.raw('POST', '/auth/switch-merchant', {
      failOnStatusCode: false,
      headers: { Authorization: `Bearer ${redirectToken}` },
      body: { merchant_id: merchant.id },
    })
    expect(switched.status).toBe(403)
  })
})
