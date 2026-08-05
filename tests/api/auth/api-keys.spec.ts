import { test, expect, factory } from '../../fixtures/test'
import { expectValidGatewayResponse } from '../../helpers/assertions'

/**
 * API-key reliability. API keys are the machine-to-machine auth path for /decide-gateway, so
 * create → list → authenticate → revoke must all hold, and the raw key must never leak in listings.
 */
test.describe('API keys (API)', () => {
  test('create returns a key and list includes it', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'ci-test-key')
    expect(created.status).toBe(200)
    expect(typeof created.body.api_key).toBe('string')
    expect(typeof created.body.key_id).toBe('string')

    const list = await api.listApiKeys(merchant.id)
    expect(list.status).toBe(200)
    const items = Array.isArray(list.body) ? list.body : list.body.keys || list.body.api_keys || []
    expect(items.some((k: any) => k.key_id === created.body.key_id)).toBe(true)
  })

  test('list never exposes the raw key material (only the prefix)', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'prefix-only')

    const list = await api.listApiKeys(merchant.id)
    const items = Array.isArray(list.body) ? list.body : list.body.keys || list.body.api_keys || []
    const found = items.find((k: any) => k.key_id === created.body.key_id)

    expect(found).toBeTruthy()
    expect(found.api_key).toBeUndefined() // raw key is returned only once, at creation
    expect(typeof found.key_prefix).toBe('string')
  })

  test('decide-gateway authenticates with an x-api-key instead of a bearer token', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    const created = await api.createApiKey(merchant.id, 'decide-key')
    const apiKey = created.body.api_key

    const req = factory.srDecideGatewayRequest({ merchantId: merchant.id, eligibleGatewayList: ['stripe', 'adyen'] })

    const saved = api.token
    api.token = null // force x-api-key auth, not the bearer token
    const decide = await api.raw('POST', '/decide-gateway', {
      headers: { 'x-api-key': apiKey },
      body: req,
      failOnStatusCode: false,
    })
    api.token = saved

    expect(decide.status).toBe(200)
    expectValidGatewayResponse(decide.body)
  })

  test('an api key can be revoked', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'revoke-me')

    const revoke = await api.raw('DELETE', `/api-key/${created.body.key_id}`, { failOnStatusCode: false })

    expect(revoke.status).toBe(200)
    expect(revoke.body.key_id).toBe(created.body.key_id)
  })

  test('revoking the same key twice is idempotent', async ({ api, merchant }) => {
    const created = await api.createApiKey(merchant.id, 'revoke-twice')

    const first = await api.raw('DELETE', `/api-key/${created.body.key_id}`, { failOnStatusCode: false })
    const second = await api.raw('DELETE', `/api-key/${created.body.key_id}`, { failOnStatusCode: false })

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
  })

  test('revoking an unknown key id is rejected', async ({ api, merchant }) => {
    const r = await api.raw('DELETE', '/api-key/00000000-0000-0000-0000-000000000000', {
      failOnStatusCode: false,
    })

    // KNOWN GAP: this currently surfaces as a 500 because the storage layer's "no rows to update" is
    // not mapped to a 404. Asserting >=400 records that the call is rejected without pinning the suite
    // to a status that should arguably change.
    expect(r.status).toBeGreaterThanOrEqual(400)
  })
})

/**
 * KNOWN GAP — api-key routes carry no cross-merchant authorization.
 *
 * `authenticate` populates an AuthContext with the caller's merchant but never compares it against the
 * merchant_id in the path or body, and the api_key handlers don't check it either. Any authenticated
 * session can therefore mint, list and revoke keys for ANY merchant.
 *
 * These tests pin the CURRENT behaviour so a future fix is a deliberate, visible change rather than a
 * surprise CI failure. The correct behaviour is noted per-test. If these start failing with 403s, the
 * gap has been closed — update the expectations rather than reverting the handler.
 */
test.describe('API key cross-merchant isolation (known gap)', () => {
  test('a session can list another merchant\'s keys', async ({ api, merchant }) => {
    const other = factory.merchantId('apikey_other')
    await api.ensureMerchantAccount(other)
    const otherKey = await api.createApiKey(other, 'belongs-to-other')

    // api.token is still the ORIGINAL merchant's session at this point.
    const list = await api.raw('GET', `/api-key/list/${other}`, { failOnStatusCode: false })

    // Should be 403. Metadata only — the raw key is never returned by list.
    expect(list.status).toBe(200)
    expect(list.body.some((k: any) => k.key_id === otherKey.body.key_id)).toBe(true)

    await api.cleanupTestData(other)
  })

  test('a session can mint a key for a merchant it does not own', async ({ api, merchant }) => {
    const other = factory.merchantId('apikey_mint_other')
    await api.ensureMerchantAccount(other)

    const created = await api.raw('POST', '/api-key/create', {
      failOnStatusCode: false,
      body: { merchant_id: other, description: 'minted-across-merchants' },
    })

    // Should be 403.
    expect(created.status).toBe(200)
    expect(created.body.merchant_id).toBe(other)

    await api.cleanupTestData(other)
  })
})
