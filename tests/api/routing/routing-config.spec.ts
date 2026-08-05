import { test, expect, factory } from '../../fixtures/test'

/**
 * The configuration surface the dashboard reads before it can render anything: the routing-key
 * catalogue behind the rule builder, the per-merchant SR scoring dimensions, and the GSM options list.
 *
 * `GET /config/routing-keys` earns its own test because the whole Euclid UI suite depends on it — the
 * rule builder blocks on "Loading routing keys from backend..." until it resolves, and every condition
 * test selects a key by name. A backend rename should fail HERE with a clear message rather than as
 * dozens of opaque UI timeouts.
 */

/** Dimensions the backend accepts for SR sub-level scoring. */
const ELIGIBLE_DIMENSIONS = ['currency', 'country', 'auth_type', 'card_is_in', 'card_network']

test.describe('Routing key catalogue (API)', () => {
  test('exposes the keys the rule builder depends on', async ({ api, merchant }) => {
    const r = await api.raw('GET', '/config/routing-keys', { failOnStatusCode: false })

    expect(r.status).toBe(200)
    expect(r.body.keys, 'routing-keys response should carry a `keys` map').toBeTruthy()

    // These three are selected by name across the Euclid UI specs — losing one breaks them all.
    for (const key of ['payment_method', 'currency', 'amount']) {
      expect(Object.keys(r.body.keys), `routing key '${key}' must exist`).toContain(key)
    }

    // Enum keys must carry their value list, or the builder's value dropdown renders empty.
    // `values` is a comma-separated STRING here, not an array.
    const paymentMethod = r.body.keys.payment_method
    expect(paymentMethod.type).toBe('enum')
    expect(typeof paymentMethod.values).toBe('string')
    expect(paymentMethod.values.split(',').map((v: string) => v.trim())).toContain('card')
  })
})

test.describe('SR scoring dimensions (API)', () => {
  test('a merchant with no configuration returns empty defaults', async ({ api, merchant }) => {
    const r = await api.raw('GET', `/config-sr-dimension/${merchant.id}`, { failOnStatusCode: false })

    // Deliberately not a 404 — the dashboard renders the config form off this response.
    expect(r.status).toBe(200)
    expect(r.body.merchant_id).toBe(merchant.id)
    expect(r.body.paymentInfo.udfs).toEqual([])
    expect(r.body.paymentInfo.fields).toBeNull()
  })

  test('configured dimensions round-trip', async ({ api, merchant }) => {
    const fields = ['currency', 'card_network']

    const saved = await api.raw('POST', '/config-sr-dimension', {
      failOnStatusCode: false,
      // `paymentInfo` is camelCase on the wire; the surrounding request is snake_case.
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields } },
    })
    expect(saved.status).toBe(200)

    const read = await api.raw('GET', `/config-sr-dimension/${merchant.id}`)
    expect(read.body.paymentInfo.fields).toEqual(fields)
  })

  test('overwriting the configuration replaces the previous dimensions', async ({ api, merchant }) => {
    await api.raw('POST', '/config-sr-dimension', {
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields: ['currency'] } },
    })
    await api.raw('POST', '/config-sr-dimension', {
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields: ['country', 'auth_type'] } },
    })

    const read = await api.raw('GET', `/config-sr-dimension/${merchant.id}`)
    expect(read.body.paymentInfo.fields).toEqual(['country', 'auth_type'])
  })

  test('every documented dimension is accepted', async ({ api, merchant }) => {
    const r = await api.raw('POST', '/config-sr-dimension', {
      failOnStatusCode: false,
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields: ELIGIBLE_DIMENSIONS } },
    })

    expect(r.status).toBe(200)
  })

  test('an unknown dimension is rejected', async ({ api, merchant }) => {
    const r = await api.raw('POST', '/config-sr-dimension', {
      failOnStatusCode: false,
      body: { merchant_id: merchant.id, paymentInfo: { udfs: [], fields: ['not_a_dimension'] } },
    })

    expect(r.status).toBe(400)
    expect(String(r.body?.message ?? r.body)).toContain('not_a_dimension')
  })
})

test.describe('GSM options (API)', () => {
  test('returns the gateway status-mapping rule list', async ({ api, merchant }) => {
    const r = await api.raw('GET', '/gsm/options', { failOnStatusCode: false })

    expect(r.status).toBe(200)
    expect(Array.isArray(r.body.rules)).toBe(true)

    if (r.body.rules.length > 0) {
      // Rows are camelCase on the wire, unlike most of the API.
      const row = r.body.rules[0]
      expect(typeof row.connector).toBe('string')
      expect(typeof row.flow).toBe('string')
      expect(typeof row.decision).toBe('string')
    }
  })
})
