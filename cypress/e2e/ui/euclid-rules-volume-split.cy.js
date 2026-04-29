const factory = require('../../support/test-data-factory')
const {
  thenSection,
  switchOutputType,
  addVolumeSplitEntry,
} = require('../../support/euclid-helpers')

describe('Volume split output', () => {
  let merchantId
  let ruleName

  before(() => {
    merchantId = factory.merchantId('euclid_vsplit')
    cy.ensureMerchantAccount(merchantId)
  })

  after(() => {
    cy.cleanupTestData(merchantId)
  })

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    ruleName = factory.ruleName('adv_rule')
    cy.intercept('GET', '**/config/routing-keys').as('routingKeys')
    cy.visitWithSession('/routing/rules', merchantId)
    // Wait for page to finish loading
    cy.contains(/Loading\.{3}|No rule-based rules yet\.|Existing Rules/).should('be.visible')
    cy.get('h1').should('contain', 'Rule-Based Routing')
    cy.wait('@routingKeys', { timeout: 15000 })
    cy.contains('button', 'Add Rule').click()
    switchOutputType(0, 'Volume Split')
  })

  it('switches the THEN section to volume split mode', () => {
    thenSection(0).within(() => {
      cy.get('input[placeholder="Split %"]').should('be.visible')
      cy.get('input[placeholder="Gateway name"]').should('be.visible')
      cy.contains('1.').should('not.exist')
    })
  })

  it('adds a volume split entry and shows split % with gateway name', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    thenSection(0).within(() => {
      cy.contains('60%').should('be.visible')
      cy.contains('stripe').should('be.visible')
    })
  })

  it('shows a running total after adding entries', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    thenSection(0).contains('Total: 60%').should('be.visible')
  })

  it('shows a warning when the total is not 100%', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    thenSection(0).contains('must equal 100%').should('be.visible')
  })

  it('shows a success indicator when the total reaches exactly 100%', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')
    thenSection(0).within(() => {
      cy.contains('Total: 100%').should('be.visible')
      cy.contains('✓').should('be.visible')
      cy.contains('must equal 100%').should('not.exist')
    })
  })

  it('removes a split entry via its delete button', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')
    thenSection(0).within(() => {
      cy.contains('stripe').closest('div').find('button').click()
      cy.contains('stripe').should('not.exist')
      cy.contains('Total: 40%').should('be.visible')
    })
  })

  it('switching back to Priority mode hides the split editor', () => {
    addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
    switchOutputType(0, 'Priority')
    thenSection(0).within(() => {
      cy.get('input[placeholder="Split %"]').should('not.exist')
      cy.get('input[placeholder="Gateway name"]').should('be.visible')
    })
  })

  it('JSON preview emits routing_type: volume_split with correct split values', () => {
    addVolumeSplitEntry(0, 70, 'stripe', 'mca_stripe')
    addVolumeSplitEntry(0, 30, 'adyen', 'mca_adyen')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"routing_type": "volume_split"')
    cy.get('pre').should('contain.text', '"split": 70')
    cy.get('pre').should('contain.text', '"split": 30')
    cy.get('pre').should('contain.text', '"volume_split": [')
  })
})
