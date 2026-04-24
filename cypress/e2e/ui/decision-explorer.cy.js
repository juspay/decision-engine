const factory = require('../../support/test-data-factory')

describe('Decision Explorer UI', () => {
  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
  })

  it('renders the auth-rate simulation surface', () => {
    const merchantId = factory.merchantId('decision_explorer_batch')
    cy.ensureMerchantAccount(merchantId)
      .then(() => cy.visitWithSession('/decisions', merchantId))
      .then(() => {
        cy.contains('h1', 'Decision Explorer').should('exist')
        cy.contains('button', 'Auth-Rate Based Routing').click()
        cy.contains('button', 'Run Auth-Rate Simulation').should('be.visible')
        cy.contains('Total Payments').should('be.visible')
        cy.contains('Success Count').should('be.visible')
        cy.contains('Failure Count').should('be.visible')
      })
      .then(() => cy.cleanupTestData(merchantId))
  })

  it('renders the rule-based evaluation surface', () => {
    const merchantId = factory.merchantId('decision_explorer_rule')
    cy.ensureMerchantAccount(merchantId)
      .then(() => cy.visitWithSession('/decisions', merchantId))
      .then(() => {
        cy.contains('button', 'Rule Based Routing').click()
        cy.contains('button', 'Evaluate Rules').should('be.visible')
        cy.contains('Rule Evaluation Parameters').should('be.visible')
        cy.contains('Fallback gateway_name/gateway_id').should('be.visible')
        cy.contains('Add Parameter').should('be.visible')
      })
      .then(() => cy.cleanupTestData(merchantId))
  })

  it('renders the volume split evaluation surface', () => {
    const merchantId = factory.merchantId('decision_explorer_volume')
    cy.ensureMerchantAccount(merchantId)
      .then(() => cy.visitWithSession('/decisions', merchantId))
      .then(() => {
        cy.contains('button', 'Volume Based Routing').click()
        cy.get('input').filter('[value="100"]').first().clear().type('20')
        cy.contains('button', 'Run Volume Evaluation').should('be.visible')
        cy.contains('Volume Split Configuration').should('be.visible')
        cy.contains('Number of Payments').should('be.visible')
        cy.contains('/routing/evaluate calls against the active volume rule.').should('be.visible')
      })
      .then(() => cy.cleanupTestData(merchantId))
  })
})
