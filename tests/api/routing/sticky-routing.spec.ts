import { test, expect, factory } from '../../fixtures/test'
import { expectValidGatewayResponse } from '../../helpers/assertions'

/**
 * Sticky routing (#393): the decide → feedback → decide loop against the real stack.
 *
 * The engine records success counts per (customer, PM:PMT, connector) from
 * update-gateway-score and, when the merchant's `sticky-routing` feature is on, pins the
 * customer's highest-count connector at decide time. Score writes happen in a spawned task
 * after the feedback response, so every assertion that depends on a prior write polls.
 *
 * Every applied pin reports `routing_approach === "STICKY_ROUTING"` (agreeing or diverging),
 * so pin assertions check both the decided gateway and the label. Deliberately NOT asserted
 * here: the health veto — driving a connector into elimination needs a scored history that
 * belongs in the Rust-level tests.
 */

const PM = 'INTERAC'
const PMT = 'RTP'
const ELIGIBLE = ['stripe', 'adyen', 'checkout']

async function enableSticky(api: any, merchantId: string) {
  const r = await api.raw('POST', `/merchant-account/${merchantId}/features/sticky-routing`, {
    body: { enabled: true },
  })
  expect(r.status).toBe(200)
}

function decideRequest(merchantId: string, customerId: string, overrides: Record<string, unknown> = {}) {
  // The factory emits a fixed shape and silently drops unknown keys, so top-level extras
  // like stickyRouting must be re-attached to the built request explicitly.
  const { stickyRouting, ...factoryOverrides } = overrides
  const request = factory.srDecideGatewayRequest({
    merchantId,
    eligibleGatewayList: ELIGIBLE,
    paymentInfo: {
      paymentId: factory.paymentId('sticky'),
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    },
    ...factoryOverrides,
  })
  if (stickyRouting !== undefined) request.stickyRouting = stickyRouting
  return request
}

/** Feedback score writes are async server-side: retry the decide until the pin lands. */
async function pollDecideUntil(
  api: any,
  request: () => Record<string, unknown>,
  predicate: (body: any) => boolean,
  attempts = 40,
): Promise<any> {
  let last: any
  for (let i = 0; i < attempts; i++) {
    const r = await api.decideGateway(request())
    expect(r.status).toBe(200)
    expectValidGatewayResponse(r.body)
    last = r.body
    if (predicate(r.body)) return r.body
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  return last
}

// Sequential on purpose: enabling a feature does a read-modify-write on the SHARED
// FeatureConf row (one service_configuration row holds every merchant's entry), so
// concurrent enableSticky calls from parallel tests lose each other's updates and a
// test's flag can silently vanish mid-run. One worker sidesteps the race; the API-side
// fix is tracked separately.
test.describe.configure({ mode: 'default' })

test.describe('Sticky routing (API)', () => {
  test('a recorded success pins the customer to that connector', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('sticky')

    // Seed one success on adyen: decide (writes the scoring-data snapshot), then feedback.
    const seedPaymentId = factory.paymentId('seed')
    const seed = await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: seedPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    expect(seed.status).toBe(200)
    const fb = await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: seedPaymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    expect(fb.status).toBe(200)

    const pinned = await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'adyen' && body.routing_approach === 'STICKY_ROUTING',
    )
    expect(pinned.decided_gateway).toBe('adyen')
    expect(pinned.routing_approach).toBe('STICKY_ROUTING')

    // The pin is stable across repeated decisions.
    const again = await api.decideGateway(decideRequest(merchant.id, customerId))
    expect(again.body.decided_gateway).toBe('adyen')
    expect(again.body.routing_approach).toBe('STICKY_ROUTING')
  })

  test('stickyRouting:false leaves the SR decision alone', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('optout')

    const seedPaymentId = factory.paymentId('seed')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: seedPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: seedPaymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    // Wait until the seeded state provably pins…
    await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'adyen' && body.routing_approach === 'STICKY_ROUTING',
    )

    // …then the opt-out flag must suppress the sticky label on the same state.
    const optedOut = await api.decideGateway(
      decideRequest(merchant.id, customerId, { stickyRouting: false }),
    )
    expect(optedOut.status).toBe(200)
    expect(optedOut.body.routing_approach).not.toBe('STICKY_ROUTING')
  })

  test('no sticky behavior without the merchant feature flag', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    // Flag deliberately NOT enabled: neither the write nor the read side may engage.
    const customerId = factory.customerId('flagoff')

    const seedPaymentId = factory.paymentId('seed')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: seedPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: seedPaymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })

    for (let i = 0; i < 5; i++) {
      const r = await api.decideGateway(decideRequest(merchant.id, customerId))
      expect(r.status).toBe(200)
      expect(r.body.routing_approach).not.toBe('STICKY_ROUTING')
      await new Promise((resolve) => setTimeout(resolve, 100))
    }
  })

  test('a retried payment sticks to the connector that finally succeeded', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('retry')

    const paymentId = factory.paymentId('retry')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    // Attempt 1 fails on stripe; the retry succeeds on adyen. Only the success may stick.
    const fail = await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId,
      gateway: 'stripe',
      status: 'AUTHORIZATION_FAILED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    expect(fail.status).toBe(200)
    const success = await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    expect(success.status).toBe(200)

    const pinned = await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'adyen' && body.routing_approach === 'STICKY_ROUTING',
    )
    expect(pinned.decided_gateway).toBe('adyen')
    expect(pinned.routing_approach).toBe('STICKY_ROUTING')
  })

  test('sticky state is scoped to the payment-method combo', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('combo')

    const seedPaymentId = factory.paymentId('seed')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: seedPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: seedPaymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'adyen' && body.routing_approach === 'STICKY_ROUTING',
    )

    // A different combo for the same customer has no counts — no sticky label.
    const otherCombo = await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: {
          paymentId: factory.paymentId('card'),
          customerId,
          paymentMethod: 'VISA',
          paymentMethodType: 'CARD',
        },
      }),
    )
    expect(otherCombo.status).toBe(200)
    expect(otherCombo.body.routing_approach).not.toBe('STICKY_ROUTING')
  })

  test('gateway failures erode the habit until the pin releases', async ({ api, merchant }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('erode')

    // Build a habit of exactly one success on adyen…
    const seedPaymentId = factory.paymentId('seed')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: seedPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: seedPaymentId,
      gateway: 'adyen',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'adyen' && body.routing_approach === 'STICKY_ROUTING',
    )

    // …then one gateway failure (a different payment) subtracts it back to zero.
    const failPaymentId = factory.paymentId('fail')
    await api.decideGateway(
      decideRequest(merchant.id, customerId, {
        paymentInfo: { paymentId: failPaymentId, customerId, paymentMethod: PM, paymentMethodType: PMT },
      }),
    )
    const fb = await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: failPaymentId,
      gateway: 'adyen',
      status: 'AUTHORIZATION_FAILED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    expect(fb.status).toBe(200)

    // A zeroed habit is no habit: the pin (and its label) must disappear.
    const released = await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.routing_approach !== 'STICKY_ROUTING',
    )
    expect(released.routing_approach).not.toBe('STICKY_ROUTING')
  })

  test('a late webhook with inline fields records without the decide-time snapshot', async ({
    api,
    merchant,
  }) => {
    await api.createSuccessRateConfig(merchant.id)
    await enableSticky(api, merchant.id)
    const customerId = factory.customerId('late')

    // No prior decide for this paymentId: the snapshot never existed, so the write can
    // only come from the payload fields (the >30-min-webhook path, compressed in time).
    const fb = await api.updateGatewayScore({
      merchantId: merchant.id,
      paymentId: factory.paymentId('nodecide'),
      gateway: 'checkout',
      status: 'CHARGED',
      customerId,
      paymentMethod: PM,
      paymentMethodType: PMT,
    })
    expect(fb.status).toBe(200)

    const pinned = await pollDecideUntil(
      api,
      () => decideRequest(merchant.id, customerId),
      (body) => body.decided_gateway === 'checkout' && body.routing_approach === 'STICKY_ROUTING',
    )
    expect(pinned.decided_gateway).toBe('checkout')
    expect(pinned.routing_approach).toBe('STICKY_ROUTING')
  })
})
