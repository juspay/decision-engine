const factory = require('../../support/test-data-factory')

describe('Volume Split UI', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('volume_ui')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates and activates a volume split rule from the page', () => {
    cy.intercept('POST', '**/routing/create').as('createVolumeSplit')
    cy.visitWithMerchant('/routing/volume', merchantId)

    cy.contains('h1', 'Volume Split Routing').should('be.visible')
    cy.get('input[placeholder="e.g. ab-test-split"]').clear().type('ui-volume-split')

    cy.get('input[placeholder="e.g. stripe"]').eq(0).type('stripe')
    cy.get('input[placeholder="optional gateway_id"]').eq(0).type('mca_stripe_ui')
    cy.get('input[placeholder="e.g. stripe"]').eq(1).type('adyen')
    cy.get('input[placeholder="optional gateway_id"]').eq(1).type('mca_adyen_ui')

    cy.contains('button', 'Create Rule').click()
    cy.wait('@createVolumeSplit', { timeout: 20000 })
    cy.contains('created successfully', { timeout: 15000 }).should('be.visible')
    cy.contains('button', 'Activate').click()
    cy.contains('Rule activated.', { timeout: 15000 }).should('be.visible')
    cy.contains('Active Volume Split').scrollIntoView().should('exist')
  })
})
