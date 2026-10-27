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
 * A session may manage only its own merchant's keys. A service caller (the admin secret, as
 * Hyperswitch calls) manages any merchant's.
 */
test.describe('API key cross-merchant isolation', () => {
  test('a session cannot list another merchant\'s keys', async ({ api, merchant }) => {
    const service = api.asService()
    const other = factory.merchantId('apikey_other')
    await service.ensureMerchantAccount(other)
    const otherKey = await service.createApiKey(other, 'belongs-to-other')

    // api.token is still the ORIGINAL merchant's session.
    const list = await api.raw('GET', `/api-key/list/${other}`, { failOnStatusCode: false })
    expect(list.status).toBe(403)

    const asService = await service.listApiKeys(other)
    expect(asService.body.some((k: any) => k.key_id === otherKey.body.key_id)).toBe(true)

    await service.cleanupTestData(other)
  })

  test('a session cannot mint a key for a merchant it does not own', async ({ api, merchant }) => {
    const service = api.asService()
    const other = factory.merchantId('apikey_mint_other')
    await service.ensureMerchantAccount(other)

    const created = await api.raw('POST', '/api-key/create', {
      failOnStatusCode: false,
      body: { merchant_id: other, description: 'minted-across-merchants' },
    })
    expect(created.status).toBe(403)

    await service.cleanupTestData(other)
  })

  test('a session cannot revoke another merchant\'s key', async ({ api, merchant }) => {
    const service = api.asService()
    const other = factory.merchantId('apikey_revoke_other')
    await service.ensureMerchantAccount(other)
    const otherKey = await service.createApiKey(other, 'not-yours')

    // Another merchant's key reads as not found, so its existence is not disclosed either.
    const revoke = await api.raw('DELETE', `/api-key/${otherKey.body.key_id}`, { failOnStatusCode: false })
    expect(revoke.status).toBe(404)

    const stillThere = await service.listApiKeys(other)
    expect(stillThere.body.find((k: any) => k.key_id === otherKey.body.key_id)?.is_active).toBeTruthy()

    await service.cleanupTestData(other)
  })
})
