const factory = require('../../support/test-data-factory')

describe('Merchant CRUD API', () => {
  let testMerchantId

  beforeEach(() => {
    cy.waitForService()
    testMerchantId = factory.merchantId('merchant_crud')
  })

  afterEach(() => {
    cy.cleanupTestData(testMerchantId)
  })

  it('creates, fetches, rejects duplicate create, and deletes a merchant account', () => {
    cy.createMerchantAccount(testMerchantId).then(({ response }) => {
      expect(response).to.haveValidMerchantCreateResponse()
      expect(response.merchant_id).to.eq(testMerchantId)
    })

    cy.getMerchantAccount(testMerchantId).then(({ response }) => {
      expect(response).to.haveValidMerchantGetResponse()
      expect(response.merchant_id).to.eq(testMerchantId)
    })

    cy.createMerchantAccount(testMerchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.not.eq(200)
    })

    cy.deleteMerchantAccount(testMerchantId).then(({ response }) => {
      expect(response).to.haveValidMerchantDeleteResponse()
      expect(response.merchant_id).to.eq(testMerchantId)
    })

    cy.getMerchantAccount(testMerchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.not.eq(200)
    })
  })

  it('gets and updates the debit routing feature flag', () => {
    const missingMerchantId = factory.merchantId('merchant_missing_debit')

    cy.createMerchantAccount(testMerchantId)

    cy.getDebitRoutingFlag(testMerchantId).then(({ response }) => {
      expect(response.merchant_id).to.eq(testMerchantId)
      expect(response.debit_routing_enabled).to.eq(false)
    })

    cy.updateDebitRoutingFlag(testMerchantId, true).then(({ response }) => {
      expect(response.merchant_id).to.eq(testMerchantId)
      expect(response.debit_routing_enabled).to.eq(true)
    })

    cy.getDebitRoutingFlag(testMerchantId).then(({ response }) => {
      expect(response.debit_routing_enabled).to.eq(true)
    })

    cy.updateDebitRoutingFlag(testMerchantId, false).then(({ response }) => {
      expect(response.debit_routing_enabled).to.eq(false)
    })

    cy.getDebitRoutingFlag(testMerchantId).then(({ response }) => {
      expect(response.debit_routing_enabled).to.eq(false)
    })

    cy.getDebitRoutingFlag(missingMerchantId, { failOnStatusCode: false }).then(({ status }) => {
      expect(status).to.eq(404)
    })
  })
})
