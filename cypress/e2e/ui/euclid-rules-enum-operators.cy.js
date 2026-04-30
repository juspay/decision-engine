const factory = require('../../support/test-data-factory')
const {
  ruleBlock,
  addGatewayToBlock,
  selectCondLhs,
  selectCondVal,
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
    cy.contains('Loading routing keys from backend...', { timeout: 15000 }).should('not.exist')
    cy.contains('button', 'Add Rule').click()
    ruleBlock(0).within(() => {
      selectCondLhs(0, 'payment_method')
    })
  })

  it('exposes "is one of" and "is not one of" in the operator dropdown for enum fields', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).within(() => {
        cy.contains('option', 'is one of').should('exist')
        cy.contains('option', 'is not one of').should('exist')
      })
    })
  })

  it('does not offer "is one of" for numeric fields', () => {
    ruleBlock(0).within(() => {
      selectCondLhs(0, 'amount')
      cy.get('select.cond-select').eq(0).within(() => {
        cy.contains('option', 'is one of').should('not.exist')
      })
    })
  })

  it('shows a multi-value picker when "is one of" is selected', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
      // LHS button + operator select — single-value button replaced by multi-select
      cy.get('.cond-select').should('have.length', 2)
      cy.get('[data-cy="cond-val"]').should('exist')
    })
  })

  it('shows a multi-value picker when "is not one of" is selected', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is not one of')
      cy.get('.cond-select').should('have.length', 2)
      cy.get('[data-cy="cond-val"]').should('exist')
    })
  })

  it('each option is independently toggleable', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
    })
    // handleOperatorChange preserves the single value — clear it first via the pill X button
    cy.get('[data-cy="cond-val"]', { withinSubject: null })
      .find('span[class*="bg-brand-100"] button').click()
    // Open portal and select an option, capturing its value for the deselect step
    cy.get('[data-cy="cond-val"]', { withinSubject: null }).click()
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null })
      .should('have.length.gte', 1)
      .then($buttons => {
        const val = $buttons.eq(0).attr('data-value')
        cy.wrap($buttons.eq(0)).click({ force: true })
        cy.get('body', { withinSubject: null }).click({ force: true })
        // 1 pill selected
        cy.get('[data-cy="cond-val"]', { withinSubject: null })
          .find('span[class*="bg-brand-100"]').should('have.length', 1)
        // Reopen and click the same option to deselect it
        cy.get('[data-cy="cond-val"]', { withinSubject: null }).click()
        cy.get(`button[data-value="${val}"]:not(.cond-select)`, { withinSubject: null })
          .click({ force: true })
        cy.get('body', { withinSubject: null }).click({ force: true })
        // No pills — back to unselected state
        cy.get('[data-cy="cond-val"]', { withinSubject: null })
          .find('span[class*="bg-brand-100"]').should('have.length', 0)
      })
  })

  it('multiple options can be selected simultaneously', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
    })
    // handleOperatorChange preserves the single value — clear it first via the pill X button
    cy.get('[data-cy="cond-val"]', { withinSubject: null })
      .find('span[class*="bg-brand-100"] button').click()
    // Open portal, capture two data-values, then click each by specific selector
    cy.get('[data-cy="cond-val"]', { withinSubject: null }).click()
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null })
      .should('have.length.gte', 2)
      .then($buttons => {
        const val0 = $buttons.eq(0).attr('data-value')
        const val1 = $buttons.eq(1).attr('data-value')
        cy.get(`button[data-value="${val0}"]:not(.cond-select)`, { withinSubject: null })
          .click({ force: true })
        cy.get(`button[data-value="${val1}"]:not(.cond-select)`, { withinSubject: null })
          .click({ force: true })
      })
    cy.get('body', { withinSubject: null }).click({ force: true })
    // Both values should appear as pills in the trigger
    cy.get('[data-cy="cond-val"]', { withinSubject: null })
      .find('span[class*="bg-brand-100"]').should('have.length', 2)
  })

  it('switching back to "equals" replaces the multi-picker with the single-value dropdown', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
      // Multi-select mode: LHS button + operator select (no value button)
      cy.get('.cond-select').should('have.length', 2)
      cy.get('select.cond-select').eq(0).select('equals')
      // Single-value mode restored: LHS button + operator select + value button
      cy.get('.cond-select').should('have.length', 3)
    })
  })

  it('preserves the previously selected single value when switching to "is one of"', () => {
    // Open the single-value enum picker (SearchableSelect — portal rendered to body)
    ruleBlock(0).within(() => {
      cy.get('[data-cy="cond-val"]').eq(0).within(() => {
        cy.get('button.cond-select').click()
      })
    })
    // Select the second option from the portal (outside within scope)
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null }).eq(1).click()
    // Switch operator to multi mode
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
    })
    // Previously selected value should be pre-populated as 1 pill
    cy.get('[data-cy="cond-val"]', { withinSubject: null })
      .find('span[class*="bg-brand-100"]').should('have.length', 1)
  })

  it('JSON preview emits enum_variant_array type with the selected values', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is one of')
    })
    // Select two values (dropdown stays open between clicks)
    cy.get('[data-cy="cond-val"]', { withinSubject: null }).click()
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null }).eq(0).click()
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null }).eq(1).click()
    cy.get('body', { withinSubject: null }).click({ force: true })
    addGatewayToBlock(0, 'stripe')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"type": "enum_variant_array"')
    cy.get('pre').should('contain.text', '"value": [')
  })

  it('JSON preview for "is not one of" uses not_equal comparison', () => {
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('is not one of')
    })
    // Select one value
    cy.get('[data-cy="cond-val"]', { withinSubject: null }).click()
    cy.get('button[data-value]:not(.cond-select)', { withinSubject: null }).eq(0).click()
    cy.get('body', { withinSubject: null }).click({ force: true })
    addGatewayToBlock(0, 'stripe')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"comparison": "not_equal"')
    cy.get('pre').should('contain.text', '"type": "enum_variant_array"')
  })
})
