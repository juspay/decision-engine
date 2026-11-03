import { test, expect, factory } from '../../fixtures/test'
import { redisKeys } from '../../helpers/redis'

/** A valid legacy body, so a 403 must come from stored ownership rather than parsing. */
function legacyFeedback(merchantId: string, paymentId: string, gateway: string) {
  const dateCreated = new Date().toISOString()
  return {
    txn_detail: {
      id: '1',
      orderId: paymentId,
      txnId: paymentId,
      txnUuid: paymentId,
      merchantId,
      gateway,
      status: 'CHARGED',
      type: 'ORDER_PAYMENT',
      dateCreated,
      txnAmount: 100,
      netAmount: 100,
      currency: 'USD',
      txnObjectType: 'ORDER_PAYMENT',
      sourceObject: 'CREDIT',
      partitionKey: null,
    },
    txn_card_info: {
      id: '1',
      dateCreated,
      paymentMethodType: 'CARD',
      paymentMethod: 'CREDIT',
      cardType: 'CREDIT',
      authType: 'THREE_DS',
      partitionKey: null,
    },
    log_message: 'feedback-context-ownership-regression',
  }
}

test('both feedback APIs reject a foreign stored context before acknowledgement or locks', async ({ api, merchant }) => {
  const service = api.asService()
  const other = factory.merchantId('feedback_owner')
  const paymentId = factory.paymentId('feedback_collision')
  const locks = () => redisKeys(`*gateway_scores_lock_*${paymentId}*`)

  expect((await service.ensureMerchantAccount(other)).status).toBe(200)
  try {
    await service.createSuccessRateConfig(other)
    const ourKey = (await api.createApiKey(merchant.id, 'feedback-caller')).body.api_key
    const ownerKey = (await service.createApiKey(other, 'feedback-owner')).body.api_key
    const caller = api.anonymous()
    const owner = api.anonymous()
    const callerHeaders = { 'x-api-key': ourKey }
    const ownerHeaders = { 'x-api-key': ownerKey }

    // The public routing path writes a real B-owned context; no direct Redis seeding.
    const decided = await owner.decideGateway(factory.srDecideGatewayRequest({
      merchantId: other,
      eligibleGatewayList: ['stripe'],
      eliminationEnabled: false,
      paymentInfo: { paymentId, paymentMethod: 'CARD', paymentMethodType: 'CREDIT', cardType: 'CREDIT' },
    }), { headers: ownerHeaders })
    expect(decided.status).toBe(200)
    expect(decided.body.decided_gateway).toBe('stripe')
    expect(await locks()).toEqual([])

    const foreignBodies = [
      {
        path: '/update-gateway-score',
        body: factory.updateGatewayScoreRequest({ merchantId: merchant.id, paymentId, gateway: 'stripe', status: 'CHARGED' }),
      },
      { path: '/update-score', body: legacyFeedback(merchant.id, paymentId, 'stripe') },
    ]
    for (const { path, body } of foreignBodies) {
      // A names A in its body, so its credential and body pass the ordinary merchant guard.
      // Only the B owner stored in Redis should prevent this feedback from running.
      const refused = await caller.raw('POST', path, { body, headers: callerHeaders, failOnStatusCode: false })
      expect(refused.status, `${path}: ${JSON.stringify(refused.body)}`).toBe(403)
      expect(refused.body.error_code).toBe('SCORING_CONTEXT_MERCHANT_MISMATCH')
      expect(await locks()).toEqual([])
    }

    // The legitimate owner can still score this exact payment. The lock proves the asynchronous
    // worker consumed the seeded context, rather than accepting and skipping a missing context.
    const accepted = await owner.updateGatewayScore(
      factory.updateGatewayScoreRequest({ merchantId: other, paymentId, gateway: 'stripe', status: 'CHARGED' }),
      { headers: ownerHeaders },
    )
    expect(accepted.status).toBe(200)
    await expect.poll(locks).toEqual([expect.stringContaining(`gateway_scores_lock_REWARD_${paymentId}_stripe`)])
  } finally {
    await service.cleanupTestData(other)
  }
})
