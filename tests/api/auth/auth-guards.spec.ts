import { test, expect, factory } from '../../fixtures/test'

/**
 * Every protected route must actually be protected.
 *
 * SELF-VALIDATING BY DESIGN: the `authenticate` middleware short-circuits and lets requests through
 * unauthenticated when `api_key_auth_enabled` is false (src/middleware.rs). If that ever gets flipped
 * off in the environment under test, the first test here fails loudly rather than the whole suite
 * silently passing for the wrong reason. Treat a failure in 'rejects a request with no credentials' as
 * "auth is disabled", not as "one endpoint regressed".
 *
 * Middleware errors are plain text, not the JSON error envelope the handlers use — so these assert on
 * status only.
 */

/** A representative protected route per surface area. */
const PROTECTED_ROUTES: Array<{ method: string; path: string; body?: unknown }> = [
  { method: 'POST', path: '/decide-gateway', body: {} },
  { method: 'GET', path: '/merchant-account/some_merchant' },
  { method: 'POST', path: '/rule/get', body: { merchant_id: 'some_merchant', algorithm: 'successRate' } },
  { method: 'GET', path: '/analytics/overview' },
  { method: 'POST', path: '/routing/create', body: {} },
  { method: 'GET', path: '/api-key/list/some_merchant' },
]

test.describe('Auth guards (API)', () => {
  test('rejects a request with no credentials', async ({ api }) => {
    const anon = api.anonymous()

    for (const route of PROTECTED_ROUTES) {
      const r = await anon.raw(route.method, route.path, {
        failOnStatusCode: false,
        body: route.body,
      })
      expect(r.status, `${route.method} ${route.path} must require authentication`).toBe(401)
    }
  })

  test('rejects a malformed bearer token', async ({ api }) => {
    const anon = api.anonymous()

    for (const route of PROTECTED_ROUTES) {
      const r = await anon.raw(route.method, route.path, {
        failOnStatusCode: false,
        headers: { Authorization: 'Bearer not-a-real-jwt' },
        body: route.body,
      })
      expect(r.status, `${route.method} ${route.path} must reject a bad token`).toBe(401)
    }
  })

  test('rejects an unknown x-api-key', async ({ api }) => {
    const anon = api.anonymous()

    const r = await anon.raw('POST', '/decide-gateway', {
      failOnStatusCode: false,
      headers: { 'x-api-key': `DE_${'0'.repeat(64)}` },
      body: {},
    })

    expect(r.status).toBe(401)
  })

  test('rejects a revoked api key immediately', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'guard-revoked')
    const apiKey = created.body.api_key

    // Works before revocation.
    const anon = api.anonymous()
    const before = await anon.raw('GET', `/api-key/list/${merchant.id}`, {
      failOnStatusCode: false,
      headers: { 'x-api-key': apiKey },
    })
    expect(before.status).toBe(200)

    await api.raw('DELETE', `/api-key/${created.body.key_id}`)

    // Revocation clears the key's cache entry, so it must fail now rather than after the cache TTL.
    const after = await anon.raw('GET', `/api-key/list/${merchant.id}`, {
      failOnStatusCode: false,
      headers: { 'x-api-key': apiKey },
    })
    expect(after.status).toBe(401)
  })

  test('a bad bearer token is not rescued by a valid x-api-key', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'guard-precedence')
    const anon = api.anonymous()

    const r = await anon.raw('GET', `/api-key/list/${merchant.id}`, {
      failOnStatusCode: false,
      headers: {
        Authorization: 'Bearer not-a-real-jwt',
        'x-api-key': created.body.api_key,
      },
    })

    // The middleware checks Authorization first and returns on failure — x-api-key is never consulted.
    expect(r.status).toBe(401)
  })

  test('health endpoints stay reachable without credentials', async ({ api }) => {
    const anon = api.anonymous()

    const health = await anon.raw('GET', '/health', { failOnStatusCode: false })
    expect(health.status).toBe(200)

    const ready = await anon.raw('GET', '/health/ready', { failOnStatusCode: false })
    expect(ready.status).toBe(200)
  })

  // `merchant` is pulled in only so `api` carries a token for the authenticated cleanup DELETE.
  test('merchant-account create stays public (it is the bootstrap route)', async ({ api, merchant }) => {
    const id = factory.merchantId('guard_public')
    const anon = api.anonymous()

    const r = await anon.raw('POST', '/merchant-account/create', {
      failOnStatusCode: false,
      body: { merchant_id: id, gateway_success_rate_based_decider_input: null },
    })

    // A merchant has to be creatable before any credential for it can exist.
    expect(r.status).toBe(200)

    await api.cleanupTestData(id)
  })
})
