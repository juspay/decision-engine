import { test, expect, factory } from '../../fixtures/test'
import { ApiClient, type ApiResponse } from '../../fixtures/api-client'
import { ensureDashboardSession, clearSession } from '../../fixtures/session'

/**
 * Merchant scope: a session (dashboard JWT or merchant api key) acts only on its own merchant, and a
 * service caller (the admin secret, as Hyperswitch calls) acts for the merchant a request names.
 *
 * - A request naming another merchant — in the `:merchant_id` path segment or the body — is refused
 *   with 403 before the handler runs.
 * - Another merchant's object (a routing algorithm, an api key) reads as not found on every lane.
 * - Deployment-level routes take only the admin secret.
 *
 * Each test gets merchant A (the `merchant` fixture; `api` carries A's session) and merchant B, created
 * here with a session of its own.
 */

type Merchants = { a: string; b: string; service: ApiClient; bApi: ApiClient }

async function withOtherMerchant(api: ApiClient, a: string, run: (m: Merchants) => Promise<void>) {
  const service = api.asService()
  const b = factory.merchantId('scope_b')
  await service.ensureMerchantAccount(b)
  const bApi = new ApiClient(api.request)
  await ensureDashboardSession(bApi, b)
  try {
    await run({ a, b, service, bApi })
  } finally {
    await service.cleanupTestData(b)
    clearSession(b)
  }
}

/** A client authenticating with a merchant api key only: no session, no admin secret. */
function keyClient(api: ApiClient, apiKey: string) {
  const anonymous = api.anonymous()
  return (method: string, path: string, body?: unknown) =>
    anonymous.raw(method, path, { body, failOnStatusCode: false, headers: { 'x-api-key': apiKey } })
}

const refused = (response: ApiResponse) => expect(response.status, JSON.stringify(response.body)).toBe(403)
const allowed = (response: ApiResponse) =>
  expect(response.status, JSON.stringify(response.body)).toBeLessThan(400)

test.describe('Merchant scope: the path', () => {
  test('a session cannot reach another merchant through the path', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ b }) => {
      const call = (method: string, path: string, body?: unknown) =>
        api.raw(method, path, { body, failOnStatusCode: false })

      refused(await call('GET', `/merchant-account/${b}`))
      refused(await call('DELETE', `/merchant-account/${b}`))
      refused(await call('GET', `/merchant-account/${b}/features`))
      refused(await call('POST', `/merchant-account/${b}/features/sr-routing`, { enabled: true }))
      refused(await call('POST', `/merchant-account/${b}/debit-routing`, { enabled: true }))
      refused(await call('GET', `/merchant-account/${b}/seed-costs`))
      refused(await call('PUT', `/merchant-account/${b}/connectors/stripe/fee-override`, { pct_bps: 1, fixed: 0 }))
      refused(await call('DELETE', `/merchant-account/${b}/connectors/adyen/credentials/acct`))
      refused(await call('GET', `/merchant-account/${b}/cost-ingestions`))
      refused(await call('POST', `/routing/list/${b}`))
      refused(await call('POST', `/routing/list/active/${b}`))
      refused(await call('GET', `/api-key/list/${b}`))
      refused(await call('GET', `/config-sr-dimension/${b}`))
    })
  })

  test('a session reaches its own merchant through the path', async ({ api, merchant }) => {
    const call = (method: string, path: string) => api.raw(method, path, { failOnStatusCode: false })
    allowed(await call('GET', `/merchant-account/${merchant.id}`))
    allowed(await call('GET', `/merchant-account/${merchant.id}/features`))
    allowed(await call('POST', `/routing/list/${merchant.id}`))
    allowed(await call('GET', `/api-key/list/${merchant.id}`))
  })

  test('an api key is held to its merchant like a session', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ a, b }) => {
      const key = (await api.createApiKey(a, 'scope-key')).body.api_key
      const asKey = keyClient(api, key)
      allowed(await asKey('POST', `/routing/list/${a}`))
      refused(await asKey('POST', `/routing/list/${b}`))
      refused(await asKey('GET', `/merchant-account/${b}/features`))
    })
  })
})

test.describe('Merchant scope: the body', () => {
  test('a session cannot name another merchant in the body', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ b }) => {
      const call = (path: string, body: unknown) => api.raw('POST', path, { body, failOnStatusCode: false })
      const paymentId = factory.paymentId('scope')

      refused(await call('/routing/create', factory.singleRoutingPayload(b)))
      refused(await call('/routing/evaluate', factory.ruleEvaluatePayload(b)))
      refused(await call('/rule/create', { merchant_id: b, config: { type: 'successRate', data: factory.srConfigData() } }))
      refused(await call('/rule/get', { merchant_id: b, algorithm: 'successRate' }))
      refused(await call('/api-key/create', { merchant_id: b, description: 'not mine' }))
      refused(await call('/gateway-score/reset', { merchant_id: b }))
      refused(await call('/config-sr-dimension', { merchant_id: b, paymentInfo: { udfs: [], fields: ['currency'] } }))
      refused(await call('/update-gateway-score', factory.updateGatewayScoreRequest({ merchantId: b, paymentId })))
      refused(await call('/routing/hybrid', {
        dynamic_routing_request: factory.srDecideGatewayRequest({ merchantId: b, paymentInfo: { paymentId } }),
      }))
    })
  })

  test('the decider answers a foreign merchant in its own error format', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ b }) => {
      const decide = await api.raw('POST', '/decide-gateway', {
        body: factory.srDecideGatewayRequest({ merchantId: b }),
        failOnStatusCode: false,
      })
      refused(decide)
      expect(decide.body.error_info.code).toBe('MERCHANT_SCOPE_VIOLATION')
    })
  })

  test('a session naming its own merchant passes the scope check', async ({ api, merchant }) => {
    const created = await api.createRoutingAlgorithm(factory.singleRoutingPayload(merchant.id), {
      failOnStatusCode: false,
    })
    allowed(created)
    const ours = await api.listRoutingAlgorithms(merchant.id)
    expect(ours.body.some((rule: any) => rule.id === created.body.rule_id)).toBe(true)
  })

  test('a parse failure names the field that failed', async ({ api, merchant }) => {
    const bad = { ...factory.singleRoutingPayload(merchant.id), algorithm: { type: 'single', data: 7 } }
    const created = await api.createRoutingAlgorithm(bad, { failOnStatusCode: false })
    expect(created.status).toBe(400)
    expect(JSON.stringify(created.body)).toContain('could not parse routing rule')
    expect(JSON.stringify(created.body)).toContain('algorithm')
  })
})

test.describe('Merchant scope: objects another merchant owns', () => {
  test('another merchant\'s routing algorithm reads as not found', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ a, b, service, bApi }) => {
      const theirs = (await bApi.createRoutingAlgorithm(factory.singleRoutingPayload(b))).body.rule_id
      const notFound = (response: ApiResponse) => {
        expect(response.status, JSON.stringify(response.body)).toBe(400)
        expect(JSON.stringify(response.body)).toContain('not found')
      }

      // The body names A, so the scope check passes; the algorithm is still B's.
      notFound(await api.activateRoutingAlgorithm(a, theirs, { failOnStatusCode: false }))
      notFound(await api.raw('POST', '/routing/deactivate', {
        body: { created_by: a, routing_algorithm_id: theirs },
        failOnStatusCode: false,
      }))
      notFound(await api.raw('POST', '/routing/update', {
        body: { created_by: a, routing_algorithm_id: theirs, name: 'hijack', algorithm: factory.singleRoutingPayload(a).algorithm },
        failOnStatusCode: false,
      }))
      // The admin secret acts for a named merchant, but cannot wire one merchant's algorithm into
      // another's active slot.
      notFound(await service.activateRoutingAlgorithm(a, theirs, { failOnStatusCode: false }))

      const listed = await service.listRoutingAlgorithms(b)
      expect(listed.body.find((rule: any) => rule.id === theirs)?.name).not.toBe('hijack')
      const ours = await api.listRoutingAlgorithms(a)
      expect(ours.body.every((rule: any) => rule.created_by === a)).toBe(true)
    })
  })

  test('another merchant\'s api key cannot be revoked, and keeps working', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ b, bApi }) => {
      const theirs = (await bApi.createApiKey(b, 'still-mine')).body
      const revoke = await api.raw('DELETE', `/api-key/${theirs.key_id}`, { failOnStatusCode: false })
      expect(revoke.status).toBe(404)
      allowed(await keyClient(api, theirs.api_key)('POST', `/routing/list/${b}`))
    })
  })
})

test.describe('Merchant scope: service callers', () => {
  test('the admin secret acts for the merchant a request names', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ b, service }) => {
      allowed(await service.listRoutingAlgorithms(b))
      allowed(await service.getMerchantFeatures(b))
    })
  })

  test('a request naming two different merchants is refused', async ({ api, merchant }) => {
    await withOtherMerchant(api, merchant.id, async ({ a, b, service }) => {
      const paymentId = factory.paymentId('scope')
      const hybrid = await service.raw('POST', '/routing/hybrid', {
        body: {
          static_routing_request: { created_by: a, payment_id: paymentId, parameters: {}, fallback_output: [factory.gatewayConnector('stripe')] },
          dynamic_routing_request: factory.srDecideGatewayRequest({ merchantId: b, paymentInfo: { paymentId } }),
        },
        failOnStatusCode: false,
      })
      expect(hybrid.status).toBe(400)
      expect(JSON.stringify(hybrid.body)).toContain('more than one merchant')
    })
  })
})

test.describe('Admin routes', () => {
  test('take only the admin secret', async ({ api, merchant }) => {
    const anonymous = api.anonymous()
    const key = (await api.createApiKey(merchant.id, 'not-an-admin')).body.api_key
    const asKey = keyClient(api, key)
    const id = factory.merchantId('scope_admin')

    for (const [path, body] of [
      ['/merchant-account/create', { merchant_id: id }],
      ['/admin/hierarchy/reconcile', {}],
      [`/admin/profiles/${id}/connectors`, []],
      ['/auth/admin/merchant-token', { merchant_id: merchant.id }],
    ] as const) {
      const none = await anonymous.raw('POST', path, { body, failOnStatusCode: false })
      expect(none.status, `${path} without credentials`).toBe(401)
      const wrong = await anonymous.raw('POST', path, {
        body,
        failOnStatusCode: false,
        headers: { 'x-admin-secret': 'wrong-secret' },
      })
      expect(wrong.status, `${path} with a wrong secret`).toBe(401)
      expect((await asKey('POST', path, body)).status, `${path} with a merchant api key`).toBe(401)
    }

    const created = await api.asService().ensureMerchantAccount(id)
    expect(created.status).toBe(200)
    await api.asService().cleanupTestData(id)
  })

  test('an unknown path is still not found', async ({ api }) => {
    const missing = await api.anonymous().raw('POST', '/admin/does-not-exist', { failOnStatusCode: false })
    expect(missing.status).toBe(404)
  })
})
