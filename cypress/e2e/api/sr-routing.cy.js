const factory = require('../../support/test-data-factory')

describe('SR Routing', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('sr_routing')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  // Validates write-through cache: an update to SR config must be immediately
  // reflected by /rule/get without waiting for TTL expiry.
  it('SR config update is immediately visible after write (cache consistency)', () => {
    cy.createSuccessRateConfig(merchantId, {
      defaultBucketSize: 150,
      defaultHedgingPercent: 3,
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    cy.getSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response.config.data.defaultBucketSize).to.eq(150)
      expect(response.config.data.defaultHedgingPercent).to.eq(3)
    })

    cy.updateSuccessRateConfig(merchantId, {
      defaultBucketSize: 300,
      defaultHedgingPercent: 10,
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    // Immediately after update — must reflect new values, not a stale cache hit
    cy.getSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response.config.data.defaultBucketSize).to.eq(300)
      expect(response.config.data.defaultHedgingPercent).to.eq(10)
    })
  })

  // Validates write-through cache eviction on delete: /rule/get must return
  // non-200 immediately after deletion, not serve the previously cached entry.
  it('SR config is not found immediately after deletion (cache eviction)', () => {
    cy.createSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    cy.getSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    cy.deleteSuccessRateConfig(merchantId).then(({ status }) => {
      expect(status).to.eq(200)
    })

    // Must not serve deleted config from cache
    cy.getSuccessRateConfig(merchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.not.eq(200)
    })
  })

  // Validates elimination config write-through cache in the same way.
  it('elimination config update is immediately visible after write (cache consistency)', () => {
    cy.createEliminationConfig(merchantId, { threshold: 0.3 }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('elimination')
    })

    cy.getEliminationConfig(merchantId).then(({ response }) => {
      expect(response.config.data.threshold).to.eq(0.3)
    })

    cy.updateEliminationConfig(merchantId, { threshold: 0.6 }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('elimination')
    })

    // Immediately after update — must not return stale 0.3
    cy.getEliminationConfig(merchantId).then(({ response }) => {
      expect(response.config.data.threshold).to.eq(0.6)
    })
  })

  // Validates that the explore-exploit fix is working: gateway scores must
  // decrease after repeated failure feedback, meaning scores are being updated
  // (not frozen at 1.0 due to the top-gateway exclusion bug).
  //
  // Each failure requires a prior /decide-gateway call with the same paymentId
  // because the backend stores GatewayScoringData in Redis keyed by paymentId
  // during decide, and /update-gateway-score looks it up by that key.
  it('gateway score decreases after repeated failure feedback (explore-exploit fix)', () => {
    cy.createSuccessRateConfig(merchantId, {
      defaultBucketSize: 10,
      defaultHedgingPercent: 50,
    })

    const gateways = ['stripe', 'adyen']
    let targetGateway

    // Run 5 decide → failure-feedback cycles in sequence.
    // Cypress chains are lazy so we build the chain with reduce.
    const FAILURES = 5
    let chain = cy.wrap(null)

    for (let i = 0; i < FAILURES; i++) {
      const pid = factory.paymentId(`fail_${i}`)
      chain = chain.then(() =>
        cy.decideGateway(
          factory.srDecideGatewayRequest({
            merchantId,
            eligibleGatewayList: gateways,
            paymentInfo: { paymentMethodType: 'CARD', paymentMethod: 'VISA', paymentId: pid },
          }),
        ).then(({ response }) => {
          expect(response).to.haveValidGatewayResponse()
          // Track whichever gateway the algorithm chose on the first iteration
          if (i === 0) targetGateway = response.decided_gateway
          return cy.updateGatewayScore(
            factory.updateGatewayScoreRequest({
              merchantId,
              gateway: response.decided_gateway,
              paymentId: pid,
              status: 'FAILURE',
            }),
          )
        }).then(({ response: sr }) => {
          expect(sr).to.haveValidScoreUpdate()
        }),
      )
    }

    // After all failures, decide once more and assert the first gateway's score dropped
    chain.then(() =>
      cy.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId,
          eligibleGatewayList: gateways,
          paymentInfo: { paymentMethodType: 'CARD', paymentMethod: 'VISA' },
        }),
      ),
    ).then(({ response }) => {
      expect(response).to.haveValidGatewayResponse()
      const scoreAfterFailures = response.gateway_priority_map[targetGateway]
      // Score must be below 1.0 — proves scores are updating, not frozen by the bug
      expect(scoreAfterFailures).to.be.lt(1.0)
    })
  })
})
