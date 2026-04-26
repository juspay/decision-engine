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

  it('renders the debit routing surface with backend-valid defaults', () => {
    const merchantId = factory.merchantId('decision_explorer_debit')
    cy.ensureMerchantAccount(merchantId)
      .then(() => cy.visitWithSession('/decisions', merchantId))
      .then(() => {
        cy.contains('button', 'Debit Routing').click()
        cy.contains('Debit Routing Parameters').should('be.visible')
        cy.contains('Debit routing is disabled for this merchant.').should('be.visible')
        cy.contains('button', 'Enable Debit Routing').should('be.visible')
        cy.get('input[value="merchant_category_code_0001"]').should('be.visible')
        cy.get('input[value="VISA, NYCE, PULSE, STAR"]').should('be.visible')
        cy.contains('button', 'Run Debit Routing').should('be.disabled')
      })
      .then(() => cy.cleanupTestData(merchantId))
  })

  it('runs debit routing through decide-gateway when enabled', () => {
    const merchantId = factory.merchantId('decision_explorer_debit_run')
    cy.ensureMerchantAccount(merchantId)
      .then(() => cy.updateDebitRoutingFlag(merchantId, true))
      .then(() => cy.visitWithSession('/decisions', merchantId))
      .then(() => {
        cy.contains('button', 'Debit Routing').click()
        cy.contains('Debit routing is enabled for this merchant.').should('be.visible')
        cy.contains('button', 'Run Debit Routing').should('not.be.disabled').click()
        cy.contains('Debit Routing Result', { timeout: 20000 }).should('be.visible')
        cy.contains('Ranked Debit Networks').should('be.visible')
        cy.contains('td', 'VISA').should('be.visible')
      })
      .then(() => cy.cleanupTestData(merchantId))
  })
})
