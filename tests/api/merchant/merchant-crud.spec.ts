import { test, expect, factory } from '../../fixtures/test'
import type { ApiClient, RequestOptions } from '../../fixtures/api-client'
import { ensureDashboardSession } from '../../fixtures/session'
import {
  expectValidMerchantCreateResponse,
  expectValidMerchantGetResponse,
  expectValidMerchantDeleteResponse,
} from '../../helpers/assertions'

/**
 * API-contract port of cypress/e2e/api/merchant-crud.cy.js.
 *
 * These tests create/get/DELETE their OWN merchants, so they do NOT use the shared `merchant`
 * fixture. Instead they mirror the Cypress `createMerchantAccount` command: POST the create, and on
 * success establish a dashboard session so subsequent protected GET/DELETE/debit-routing calls carry
 * a bearer token (the merchant-account routes sit behind the authenticate middleware). Cleanup runs
 * in an afterEach via `api.cleanupTestData`, matching the Cypress `cleanupTestData` afterEach.
 */

/** Port of Cypress `cy.createMerchantAccount`: create + establish a session on success. */
async function createMerchant(api: ApiClient, id: string, opts: RequestOptions = {}) {
  const res = await api.raw('POST', '/merchant-account/create', {
    ...opts,
    body: { merchant_id: id, gateway_success_rate_based_decider_input: null },
  })
  if (res.status === 200) await ensureDashboardSession(api, id)
  return res
}

test.describe('Merchant CRUD API', () => {
  let testMerchantId: string

  test.beforeEach(() => {
    testMerchantId = factory.merchantId('merchant_crud')
  })

  test.afterEach(async ({ api }) => {
    await api.cleanupTestData(testMerchantId)
  })

  test('creates, fetches, rejects duplicate create, and deletes a merchant account', async ({ api }) => {
    const created = await createMerchant(api, testMerchantId)
    expectValidMerchantCreateResponse(created.body)
    expect(created.body.merchant_id).toBe(testMerchantId)

    const got = await api.getMerchantAccount(testMerchantId)
    expectValidMerchantGetResponse(got.body)
    expect(got.body.merchant_id).toBe(testMerchantId)

    const duplicate = await createMerchant(api, testMerchantId, { failOnStatusCode: false })
    expect(duplicate.status).not.toBe(200)

    const deleted = await api.deleteMerchantAccount(testMerchantId)
    expectValidMerchantDeleteResponse(deleted.body)
    expect(deleted.body.merchant_id).toBe(testMerchantId)

    // Must return non-200 immediately after deletion — validates cache eviction
    // on delete so a cached entry doesn't ghost the deleted merchant.
    const afterDelete = await api.getMerchantAccount(testMerchantId, { failOnStatusCode: false })
    expect(afterDelete.status).not.toBe(200)
  })

  test('deleted merchant is not served from cache on immediate re-fetch', async ({ api }) => {
    await createMerchant(api, testMerchantId)

    // Warm the cache with a successful GET
    const got = await api.getMerchantAccount(testMerchantId)
    expectValidMerchantGetResponse(got.body)

    await api.deleteMerchantAccount(testMerchantId)

    // The cache must be evicted — stale hit would return 200 here
    const afterDelete = await api.getMerchantAccount(testMerchantId, { failOnStatusCode: false })
    expect(afterDelete.status).not.toBe(200)
  })

  test('gets and updates the debit routing feature flag', async ({ api }) => {
    const missingMerchantId = factory.merchantId('merchant_missing_debit')

    await createMerchant(api, testMerchantId)

    let flag = await api.raw('GET', `/merchant-account/${testMerchantId}/debit-routing`)
    expect(flag.body.merchant_id).toBe(testMerchantId)
    expect(flag.body.debit_routing_enabled).toBe(false)

    const enabled = await api.raw('POST', `/merchant-account/${testMerchantId}/debit-routing`, {
      body: { enabled: true },
    })
    expect(enabled.body.merchant_id).toBe(testMerchantId)
    expect(enabled.body.debit_routing_enabled).toBe(true)

    flag = await api.raw('GET', `/merchant-account/${testMerchantId}/debit-routing`)
    expect(flag.body.debit_routing_enabled).toBe(true)

    const disabled = await api.raw('POST', `/merchant-account/${testMerchantId}/debit-routing`, {
      body: { enabled: false },
    })
    expect(disabled.body.debit_routing_enabled).toBe(false)

    flag = await api.raw('GET', `/merchant-account/${testMerchantId}/debit-routing`)
    expect(flag.body.debit_routing_enabled).toBe(false)

    const missing = await api.raw('GET', `/merchant-account/${missingMerchantId}/debit-routing`, {
      failOnStatusCode: false,
    })
    expect(missing.status).toBe(404)
  })
})
