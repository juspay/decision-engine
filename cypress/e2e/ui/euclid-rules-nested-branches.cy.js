const factory = require('../../support/test-data-factory')
const {
  ruleBlock,
  addGatewayToBlock,
} = require('../../support/euclid-helpers')

describe('Nested AND+OR branches', () => {
  let merchantId
  let ruleName

  before(() => {
    merchantId = factory.merchantId('euclid_nested')
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
  })

  it('shows "Add nested branch" in each condition group footer', () => {
    ruleBlock(0).contains('button', 'Add nested branch').should('be.visible')
  })

  it('adds a nested branch section on click', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).contains('Then match any of').should('be.visible')
  })

  it('renders the nested group indented with a sky left border', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').should('have.length', 1)
  })

  it('a second nested branch shows an OR separator', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').should('have.length', 2)
    ruleBlock(0).contains('p', 'OR').should('be.visible')
  })

  it('"Add nested branch" does not appear inside a nested group (depth capped at 1)', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    // Still exactly one "Add nested branch" button in the whole block
    ruleBlock(0).find('button').filter(':contains("Add nested branch")').should('have.length', 1)
  })

  it('allows adding AND conditions inside a nested branch', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
      cy.contains('button', 'Add condition').click()
      cy.contains('AND').should('be.visible')
    })
  })

  it('nested branch can target a different field from the parent condition', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('payment_method')
    })
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
      cy.get('select.cond-select').eq(0).select('currency')
    })
    // Parent field must remain unchanged
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).should('have.value', 'payment_method')
    })
  })

  it('removes a nested branch via Remove group', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').should('have.length', 2)
    ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
      cy.contains('button', 'Remove group').click()
    })
    ruleBlock(0).find('.border-l-2.border-sky-200').should('have.length', 1)
    ruleBlock(0).contains('p', 'OR').should('not.exist')
  })

  it('hides the nested section when all branches are removed', () => {
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
      cy.contains('button', 'Remove group').click()
    })
    ruleBlock(0).contains('Then match any of').should('not.exist')
  })

  it('OR groups each get their own independent "Add nested branch" button', () => {
    ruleBlock(0).within(() => {
      cy.contains('button', 'Add OR group').click()
    })
    // Two condition groups → two "Add nested branch" buttons
    ruleBlock(0).find('button').filter(':contains("Add nested branch")').should('have.length', 2)
  })

  it('JSON preview emits a non-null nested array when a branch is added', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('amount')
      cy.get('select.cond-select').eq(1).select('greater than')
      cy.get('input[type="number"]').type('10')
    })
    ruleBlock(0).contains('button', 'Add nested branch').click()
    ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
      cy.get('select.cond-select').eq(0).select('payment_method')
    })
    addGatewayToBlock(0, 'rbl')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"nested": [')
  })
})
