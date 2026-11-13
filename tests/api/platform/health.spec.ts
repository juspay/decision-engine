import { test, expect } from '../../fixtures/test'

/**
 * Smoke check: the decision-engine API is up and healthy. This is the cheapest possible
 * reliability signal — if it fails, nothing else in the suite is meaningful.
 *
 * All three health routes are nested OUTSIDE the auth middleware, which is what makes them usable as a
 * load-balancer probe; auth-guards.spec.ts asserts that stays true.
 */
test.describe('Health (API smoke)', () => {
  test('GET /health returns 200', async ({ api }) => {
    const r = await api.raw('GET', '/health', { failOnStatusCode: false })
    expect(r.status).toBe(200)
    expect(r.body.message).toBe('Health is good')
  })

  test('GET /health/ready reports the server as up', async ({ api }) => {
    const r = await api.raw('GET', '/health/ready', { failOnStatusCode: false })

    // Readiness answers 400 (not 503) while draining. The suite only ever runs against a live server,
    // so anything other than Up here means the stack came up wrong.
    expect(r.status).toBe(200)
    expect(r.body.message).toBe('Up')
  })

  test('GET /health/diagnostics reports storage round-trips as working', async ({ api }) => {
    const r = await api.raw('GET', '/health/diagnostics', { failOnStatusCode: false })

    expect(r.status).toBe(200)
    expect(r.body.key_custodian_locked).toBe(false)
    for (const check of ['database_connection', 'database_read', 'database_write', 'database_delete']) {
      expect(r.body.database[check], `${check} should be Working`).toBe('Working')
    }
  })

  test('diagnostics requires a tenant header', async ({ api }) => {
    // The tenant resolver runs before the handler — without it there is no database to diagnose.
    const r = await api.raw('GET', '/health/diagnostics', {
      failOnStatusCode: false,
      headers: { 'x-tenant-id': '' },
    })

    expect(r.status).toBe(400)
  })
})
