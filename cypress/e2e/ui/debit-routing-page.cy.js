const factory = require('../../support/test-data-factory')

describe('Debit Routing UI', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('debit_ui')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('toggles merchant debit routing access without exposing unsupported config editing', () => {
    cy.visitWithMerchant('/routing/debit', merchantId)

    cy.contains('h1', 'Network / Debit Routing').should('be.visible')
    cy.contains('Debit Routing Runtime Access').should('be.visible')
    cy.contains('Save Config').should('not.exist')
    cy.contains('Merchant Category Code (MCC)').should('not.exist')

    cy.contains('button', 'Enable Debit Routing').click()
    cy.contains('Debit routing enabled.').should('be.visible')
    cy.getDebitRoutingFlag(merchantId).then(({ response }) => {
      expect(response.debit_routing_enabled).to.eq(true)
    })

    cy.contains('button', 'Disable Debit Routing').click()
    cy.contains('Debit routing disabled.').should('be.visible')
    cy.getDebitRoutingFlag(merchantId).then(({ response }) => {
      expect(response.debit_routing_enabled).to.eq(false)
    })
  })
})
