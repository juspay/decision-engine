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
})
