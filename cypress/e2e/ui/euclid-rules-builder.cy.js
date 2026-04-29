/**
 * euclid-rules-builder.cy.js
 *
 * Tests for the rule builder form: page rendering, rule blocks,
 * conditions, OR groups, gateways, preview JSON, and validation.
 * No rule creation API calls — purely UI interactions.
 */

const factory = require('../../support/test-data-factory')
const { ruleBlock, addGatewayToBlock, addFallbackGateway } = require('../../support/euclid-helpers')

describe('Rule Builder — UI interactions', () => {
  let merchantId
  let ruleName

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('euclid_ui')
    ruleName = factory.ruleName('ui_rule')
    cy.visitWithMerchant('/routing/rules', merchantId)
    cy.contains('h1', 'Rule-Based Routing').should('be.visible')
    cy.contains('Loading routing keys from backend...', { timeout: 15000 }).should('not.exist')
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  // ── Page rendering ────────────────────────────────────────────────────────

  describe('Page rendering', () => {
    it('shows the rule builder form and an empty existing-rules panel', () => {
      cy.contains('h2', 'Rule Builder').should('be.visible')
      cy.contains('h2', 'Existing Rules').should('be.visible')
      cy.get('input[placeholder="my-rule"]').should('be.visible')
      cy.get('input[placeholder="Optional description"]').should('be.visible')
      cy.contains('No rule-based rules yet.').should('be.visible')
    })

    it('shows the Default Fallback section below the rule list', () => {
      cy.contains('p', 'Default Fallback').should('be.visible')
      cy.contains('Used when no rule matches').should('be.visible')
    })

    it('shows Create Rule and Preview JSON buttons', () => {
      cy.contains('button', 'Create Rule').should('be.visible')
      cy.contains('button', 'Preview JSON').should('be.visible')
    })
  })

  // ── Rule block management ─────────────────────────────────────────────────

  describe('Rule block management', () => {
    it('adds a rule block when clicking Add Rule', () => {
      cy.contains('button', 'Add Rule').click()
      cy.get('input[placeholder="Rule name"]').should('have.length', 1)
      cy.contains('p', 'If').should('be.visible')
      cy.contains('p', 'Then route').should('be.visible')
    })

    it('adds multiple rule blocks independently', () => {
      cy.contains('button', 'Add Rule').click()
      cy.contains('button', 'Add Rule').click()
      cy.get('input[placeholder="Rule name"]').should('have.length', 2)
      cy.get('input[placeholder="Rule name"]').eq(0).should('have.value', 'Rule 1')
      cy.get('input[placeholder="Rule name"]').eq(1).should('have.value', 'Rule 2')
    })

    it('allows renaming a rule block inline', () => {
      cy.contains('button', 'Add Rule').click()
      ruleBlock(0).find('input[placeholder="Rule name"]').clear().type('card-rule')
      ruleBlock(0).find('input[placeholder="Rule name"]').should('have.value', 'card-rule')
    })

    it('collapses and expands a rule block', () => {
      cy.contains('button', 'Add Rule').click()
      ruleBlock(0).contains('p', 'If').should('be.visible')

      ruleBlock(0).find('button[aria-label="Collapse rule"]').click()
      ruleBlock(0).contains('p', 'If').should('not.exist')

      ruleBlock(0).find('button[aria-label="Expand rule"]').click()
      ruleBlock(0).contains('p', 'If').should('be.visible')
    })

    it('removes a rule block with the delete button', () => {
      cy.contains('button', 'Add Rule').click()
      cy.contains('button', 'Add Rule').click()
      cy.get('input[placeholder="Rule name"]').should('have.length', 2)

      ruleBlock(0).find('button[aria-label="Delete rule"]').click()
      cy.get('input[placeholder="Rule name"]').should('have.length', 1)
    })
  })

  // ── Condition editing ─────────────────────────────────────────────────────

  describe('Condition editing', () => {
    beforeEach(() => {
      cy.contains('button', 'Add Rule').click()
    })

    it('shows one condition row by default in a new rule block', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').should('have.length.gte', 2)
        cy.contains('button', 'Add condition').should('be.visible')
      })
    })

    it('adds a second AND condition', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add condition').click()
        cy.contains('IF').should('be.visible')
        cy.contains('AND').should('be.visible')
      })
    })

    it('adds a third AND condition', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add condition').click()
        cy.contains('button', 'Add condition').click()
        cy.contains('AND').should('be.visible')
        cy.get('.rounded-lg.border').first().find('[class*="divide-y"] > div').should('have.length', 3)
      })
    })

    it('removes a condition when there are multiple', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add condition').click()
        cy.contains('AND').should('be.visible')

        cy.get('select.cond-select').first()
          .closest('[class*="flex"][class*="flex-wrap"]')
          .find('button')
          .click()

        cy.contains('AND').should('not.exist')
      })
    })

    it('shows enum value dropdown for an enum-type field', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').should('have.length', 3)
        cy.get('input[type="number"]').should('not.exist')
      })
    })

    it('shows numeric input and extra comparison operators for amount field', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('amount')
        cy.get('select.cond-select').eq(1).within(() => {
          cy.get('option').should('have.length.gte', 4)
        })
        cy.get('input[type="number"]').should('be.visible')
        cy.get('select.cond-select').should('have.length', 2)
      })
    })

    it('shows human-readable labels in the field dropdown', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).within(() => {
          cy.get('option').each(($opt) => {
            expect($opt.text()).to.not.match(/_/)
          })
        })
      })
    })

    it('shows human-readable labels in the enum value dropdown', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).within(() => {
          cy.get('option').each(($opt) => {
            const text = $opt.text()
            if (text !== 'select...') expect(text).to.not.match(/_/)
          })
        })
      })
    })

    it('can select a different field and choose a value', () => {
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('currency')
        cy.get('select.cond-select').eq(2).should('be.visible')
        cy.get('select.cond-select').eq(2).find('option').should('have.length.gt', 1)
      })
    })
  })

  // ── OR group management ───────────────────────────────────────────────────

  describe('OR group management', () => {
    beforeEach(() => {
      cy.contains('button', 'Add Rule').click()
    })

    it('does not show Remove group when only one group exists', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Remove group').should('not.exist')
      })
    })

    it('adds an OR group when clicking Add OR group', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add OR group').click()
        cy.get('button').filter(':contains("Add condition")').should('have.length', 2)
        cy.contains('span', 'or').should('be.visible')
      })
    })

    it('shows OR separator between groups', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add OR group').click()
        cy.contains('span', 'or').should('be.visible')
      })
    })

    it('shows Remove group button when multiple groups exist', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add OR group').click()
        cy.contains('button', 'Remove group').should('be.visible')
      })
    })

    it('removes an OR group', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add OR group').click()
        cy.get('button').filter(':contains("Add condition")').should('have.length', 2)
        cy.contains('button', 'Remove group').first().click()
        cy.get('button').filter(':contains("Add condition")').should('have.length', 1)
        cy.contains('button', 'Remove group').should('not.exist')
      })
    })

    it('can configure each OR group independently', () => {
      ruleBlock(0).within(() => {
        cy.contains('button', 'Add OR group').click()

        cy.get('button').filter(':contains("Add condition")').eq(0)
          .closest('[class*="rounded-lg"]')
          .within(() => {
            cy.get('select.cond-select').eq(0).select('currency')
          })

        cy.get('button').filter(':contains("Add condition")').eq(1)
          .closest('[class*="rounded-lg"]')
          .within(() => {
            cy.get('select.cond-select').eq(0).then(($sel) => {
              expect($sel.val()).to.not.eq('currency')
            })
          })
      })
    })
  })

  // ── Gateway output ────────────────────────────────────────────────────────

  describe('Gateway output', () => {
    beforeEach(() => {
      cy.contains('button', 'Add Rule').click()
    })

    it('adds a gateway to the priority output', () => {
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      ruleBlock(0).contains('stripe').should('be.visible')
    })

    it('adds multiple gateways and shows them in order', () => {
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addGatewayToBlock(0, 'adyen', 'mca_adyen')
      ruleBlock(0).within(() => {
        cy.contains('1. stripe').should('be.visible')
        cy.contains('2. adyen').should('be.visible')
      })
    })

    it('removes a gateway from the priority list', () => {
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addGatewayToBlock(0, 'adyen', 'mca_adyen')
      ruleBlock(0).contains('1. stripe').closest('div').within(() => {
        cy.get('button').click()
      })
      ruleBlock(0).contains('stripe').should('not.exist')
      ruleBlock(0).contains('1. adyen').should('be.visible')
    })

    it('shows gateway name suggestions from other entries', () => {
      addFallbackGateway('stripe', 'mca_stripe')
      ruleBlock(0).within(() => {
        cy.get('input[placeholder="Gateway name"]').then(($input) => {
          const listId = $input.attr('list')
          cy.get(`datalist#${listId} option[value="stripe"]`).should('exist')
        })
      })
    })
  })

  // ── Default Fallback ──────────────────────────────────────────────────────

  describe('Default Fallback', () => {
    it('adds a gateway to the default fallback', () => {
      addFallbackGateway('checkout', 'mca_checkout')
      cy.contains('p', 'Default Fallback')
        .closest('.rounded-xl')
        .contains('checkout')
        .should('be.visible')
    })

    it('shows correct description text', () => {
      cy.contains('Used when no rule matches').should('be.visible')
      cy.contains('fallback_output').should('be.visible')
    })
  })

  // ── Preview JSON ──────────────────────────────────────────────────────────

  describe('Preview JSON', () => {
    it('toggles the JSON preview panel', () => {
      cy.contains('button', 'Preview JSON').click()
      cy.contains('h2', 'JSON Preview').should('be.visible')
      cy.contains('button', 'Hide JSON').click()
      cy.contains('h2', 'JSON Preview').should('not.exist')
    })

    it('reflects the rule name in the JSON preview', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Preview JSON').click()
      cy.contains('pre', ruleName).should('be.visible')
    })

    it('reflects added gateways in the JSON preview', () => {
      cy.contains('button', 'Add Rule').click()
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Preview JSON').click()
      cy.get('pre').should('contain.text', 'stripe')
    })

    it('reflects conditions in the JSON preview', () => {
      cy.contains('button', 'Add Rule').click()
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Preview JSON').click()
      cy.get('pre').should('contain.text', 'statements')
      cy.get('pre').should('contain.text', 'condition')
    })
  })

  // ── Validation ────────────────────────────────────────────────────────────

  describe('Validation', () => {
    it('blocks submission and shows an error when rule name is empty', () => {
      cy.get('input[placeholder="my-rule"]').should('have.value', '')
      cy.contains('button', 'Create Rule').click()
      cy.contains('Rule name is required').should('be.visible')
    })
  })
})
