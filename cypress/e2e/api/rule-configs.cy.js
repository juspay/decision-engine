const factory = require('../../support/test-data-factory')

describe('Rule Config CRUD API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('rule_config')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates, fetches, updates, and deletes success-rate config', () => {
    cy.createSuccessRateConfig(merchantId, {
      defaultSuccessRate: 0.5,
      defaultBucketSize: 200,
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    cy.getSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
      expect(response.config.data.defaultBucketSize).to.eq(200)
    })

    cy.updateSuccessRateConfig(merchantId, {
      defaultSuccessRate: 0.7,
      defaultBucketSize: 240,
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('successRate')
    })

    cy.getSuccessRateConfig(merchantId).then(({ response }) => {
      expect(response.config.data.defaultBucketSize).to.eq(240)
      expect(response.config.data.defaultHedgingPercent).to.eq(5)
    })

    cy.deleteSuccessRateConfig(merchantId).then(({ status }) => {
      expect(status).to.eq(200)
    })

    cy.getSuccessRateConfig(merchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.not.eq(200)
    })
  })

  it('creates, fetches, updates, and deletes elimination config', () => {
    cy.createEliminationConfig(merchantId, {
      threshold: 0.35,
      txnLatency: { gatewayLatency: 4500 },
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('elimination')
    })

    cy.getEliminationConfig(merchantId).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('elimination')
      expect(response.config.data.threshold).to.eq(0.35)
    })

    cy.updateEliminationConfig(merchantId, {
      threshold: 0.55,
      txnLatency: { gatewayLatency: 6500 },
    }).then(({ response }) => {
      expect(response).to.haveValidRuleConfigResponse('elimination')
    })

    cy.getEliminationConfig(merchantId).then(({ response }) => {
      expect(response.config.data.threshold).to.eq(0.55)
      expect(response.config.data.txnLatency.gatewayLatency).to.eq(6500)
    })

    cy.deleteEliminationConfig(merchantId).then(({ status }) => {
      expect(status).to.eq(200)
    })

    cy.getEliminationConfig(merchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.not.eq(200)
    })
  })
})
