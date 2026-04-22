const factory = require('../../support/test-data-factory')

describe('Rule-Based Routing UI', () => {
  let merchantId
  let ruleName

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('euclid_ui')
    ruleName = factory.ruleName('ui_advanced_rule')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates and activates an advanced priority rule from the UI', () => {
    cy.visitWithMerchant('/routing/rules', merchantId)

    cy.contains('h1', 'Rule-Based Routing').should('be.visible')
    cy.get('input[placeholder="my-rule"]').clear().type(ruleName)
    cy.get('input[placeholder="Optional description"]').clear().type('Created by Cypress')

    cy.contains('p', 'DEFAULT SELECTION (Fallback)')
      .parent()
      .within(() => {
        cy.get('input[placeholder="gateway_name"]').first().type('stripe')
        cy.get('input[placeholder="gateway_id"]').first().type('mca_stripe_ui')
        cy.contains('button', 'Add').click()
      })

    cy.contains('button', 'Add Rule Block').click()

    cy.get('input[placeholder="Rule name"]').last().clear().type('card-rule')

    cy.get('input[placeholder="Rule name"]')
      .last()
      .parents('.border')
      .first()
      .within(() => {
        cy.contains('button', 'Add Condition').click()
        cy.get('input[placeholder="gateway_name"]').first().type('adyen')
        cy.get('input[placeholder="gateway_id"]').first().type('mca_adyen_ui')
        cy.contains('button', 'Add').click()
      })

    cy.contains('button', 'Create Rule').click()
    cy.contains(ruleName, { timeout: 20000 }).should('exist')
    cy.contains(ruleName)
      .parentsUntil('div.divide-y')
      .last()
      .parent()
      .within(() => {
        cy.contains('button', 'Activate').click()
      })
    cy.contains('Rule activated successfully.').should('be.visible')
  })
})
