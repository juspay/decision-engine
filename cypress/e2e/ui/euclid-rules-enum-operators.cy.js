const factory = require('../../support/test-data-factory')
const {
  ruleBlock,
  addGatewayToBlock,
} = require('../../support/euclid-helpers')

describe('"is one of" / "is not one of" operator', () => {
  let merchantId
  let ruleName

  before(() => {
    merchantId = factory.merchantId('euclid_enum')
    cy.ensureMerchantAccount(merchantId)
  })

  after(() => {
    cy.cleanupTestData(merchantId)
  })

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    ruleName = factory.ruleName('adv_rule')
    cy.visitWithSession('/routing/rules', merchantId)
    cy.contains('h1', 'Rule-Based Routing').should('be.visible')
    cy.contains('Loading routing keys from backend...', { timeout: 15000 }).should('not.exist')
    cy.contains('button', 'Add Rule').click()
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('payment_method')
    })
  })

  it('exposes "is one of" and "is not one of" in the operator dropdown for enum fields', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).within(() => {
        cy.contains('option', 'is one of').should('exist')
        cy.contains('option', 'is not one of').should('exist')
      })
    })
  })

  it('does not offer "is one of" for numeric fields', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('amount')
      cy.get('select.cond-select').eq(1).within(() => {
        cy.contains('option', 'is one of').should('not.exist')
      })
    })
  })

  it('shows a checkbox list when "is one of" is selected', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is one of')
      cy.get('input[type="checkbox"]').should('have.length.gte', 1)
      // Single-value dropdown replaced by checkboxes
      cy.get('select.cond-select').should('have.length', 2)
    })
  })

  it('shows a checkbox list when "is not one of" is selected', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is not one of')
      cy.get('input[type="checkbox"]').should('have.length.gte', 1)
    })
  })

  it('each checkbox is independently toggleable', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is one of')
      cy.get('input[type="checkbox"]').first().check()
      cy.get('input[type="checkbox"]').first().should('be.checked')
      cy.get('input[type="checkbox"]').first().uncheck()
      cy.get('input[type="checkbox"]').first().should('not.be.checked')
    })
  })

  it('multiple checkboxes can be checked simultaneously', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is one of')
      cy.get('input[type="checkbox"]').eq(0).check()
      cy.get('input[type="checkbox"]').eq(1).check()
      cy.get('input[type="checkbox"]').filter(':checked').should('have.length', 2)
    })
  })

  it('switching back to "equals" replaces checkboxes with the single-value dropdown', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is one of')
      cy.get('input[type="checkbox"]').should('exist')
      cy.get('select.cond-select').eq(1).select('equals')
      cy.get('input[type="checkbox"]').should('not.exist')
      cy.get('select.cond-select').should('have.length', 3)
    })
  })

  it('preserves the previously selected single value as the first checked box when switching to "is one of"', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(2).find('option').eq(1).then(($opt) => {
        const val = $opt.val()
        cy.get('select.cond-select').eq(2).select(val)
        cy.get('select.cond-select').eq(1).select('is one of')
        cy.get('input[type="checkbox"]').filter(':checked').should('have.length', 1)
      })
    })
  })

  it('JSON preview emits enum_variant_array type with the checked values', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is one of')
      cy.get('input[type="checkbox"]').eq(0).check()
      cy.get('input[type="checkbox"]').eq(1).check()
    })
    addGatewayToBlock(0, 'stripe')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"type": "enum_variant_array"')
    cy.get('pre').should('contain.text', '"value": [')
  })

  it('JSON preview for "is not one of" uses not_equal comparison', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(1).select('is not one of')
      cy.get('input[type="checkbox"]').eq(0).check()
    })
    addGatewayToBlock(0, 'stripe')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"comparison": "not_equal"')
    cy.get('pre').should('contain.text', '"type": "enum_variant_array"')
  })
})
