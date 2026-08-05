import { test, expect, factory } from '../../fixtures/test'

/**
 * Authentication reliability. The dashboard and every protected route depend on this path, so
 * signup → login → session identity must be dependable, and bad credentials must be rejected.
 */
test.describe('Auth (API)', () => {
  test('signup returns a session token', async ({ api }) => {
    const id = factory.merchantId('auth')
    await api.ensureMerchantAccount(id)

    const r = await api.raw('POST', '/auth/signup', {
      failOnStatusCode: false,
      body: { email: `${id}@example.com`, password: 'Password123!', merchant_id: id },
    })

    expect(r.status).toBe(200)
    expect(typeof r.body.token).toBe('string')

    api.token = r.body.token
    await api.cleanupTestData(id)
  })

  test('login succeeds with valid credentials', async ({ api }) => {
    const id = factory.merchantId('auth')
    const email = `${id}@example.com`
    const password = 'Password123!'
    await api.ensureMerchantAccount(id)
    const signup = await api.raw('POST', '/auth/signup', { failOnStatusCode: false, body: { email, password, merchant_id: id } })
    api.token = signup.body?.token ?? null

    const login = await api.raw('POST', '/auth/login', { failOnStatusCode: false, body: { email, password } })

    expect(login.status).toBe(200)
    expect(typeof login.body.token).toBe('string')

    await api.cleanupTestData(id)
  })

  test('login is rejected with a wrong password', async ({ api }) => {
    const id = factory.merchantId('auth')
    const email = `${id}@example.com`
    await api.ensureMerchantAccount(id)
    const signup = await api.raw('POST', '/auth/signup', { failOnStatusCode: false, body: { email, password: 'Password123!', merchant_id: id } })
    api.token = signup.body?.token ?? null

    const bad = await api.raw('POST', '/auth/login', { failOnStatusCode: false, body: { email, password: 'WrongPassword!' } })

    expect(bad.status).toBeGreaterThanOrEqual(400)

    await api.cleanupTestData(id)
  })

  test('GET /auth/me returns the authenticated identity', async ({ api, merchant }) => {
    // The `merchant` fixture already signed up and set api.token to that session.
    const me = await api.raw('GET', '/auth/me', { failOnStatusCode: false })

    expect(me.status).toBe(200)
    expect(me.body.merchant_id || me.body.email).toBeTruthy()
  })
})
