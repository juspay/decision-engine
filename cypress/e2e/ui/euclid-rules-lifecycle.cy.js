/**
 * euclid-rules-lifecycle.cy.js
 *
 * Tests for rule creation flows and existing-rules panel management:
 * creating rules via the form, activating, deactivating, and viewing rules.
 * These tests make API calls and assert on backend state.
 */

const factory = require('../../support/test-data-factory')
const { ruleBlock, addGatewayToBlock, addFallbackGateway } = require('../../support/euclid-helpers')

describe('Rule Lifecycle — creation and management', () => {
  let merchantId
  let ruleName

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('euclid_ui')
    ruleName = factory.ruleName('ui_rule')
    cy.ensureMerchantAccount(merchantId)
    cy.intercept('GET', '**/config/routing-keys').as('routingKeys')
    cy.intercept('POST', '**/routing/create').as('createRule')
    cy.visitWithMerchant('/routing/rules', merchantId)
    // Give the app time to initialize
    cy.wait(1000)
    cy.contains(/Rule-Based|Routing|Euclid/).should('exist')
    cy.wait('@routingKeys', { timeout: 15000 })
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  // ── Rule creation — full flows ────────────────────────────────────────────

  describe('Rule creation', () => {
    it('creates a minimal rule with name and default fallback only', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      addFallbackGateway('stripe', 'mca_stripe')
      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with one condition and one gateway', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.get('input[placeholder="Optional description"]').type('Cypress test rule')

      cy.contains('button', 'Add Rule').click()
      ruleBlock(0).find('input[placeholder="Rule name"]').clear().type('card-rule')
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })

      addGatewayToBlock(0, 'adyen', 'mca_adyen')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
      cy.contains(ruleName).should('exist')
    })

    it('creates a rule with two AND conditions', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Add Rule').click()

      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')

        cy.contains('button', 'Add condition').click()
        cy.get('select.cond-select').eq(3).select('currency')
        cy.get('select.cond-select').eq(5).select('USD')

        cy.contains('AND').should('be.visible')
      })

      addGatewayToBlock(0, 'checkout', 'mca_checkout')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with two OR groups', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Add Rule').click()

      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')

        cy.contains('button', 'Add OR group').click()

        cy.get('button').filter(':contains("Add condition")').eq(1)
          .closest('[class*="rounded-lg"]')
          .within(() => {
            cy.get('select.cond-select').eq(0).select('currency')
            cy.get('select.cond-select').eq(2).select('USD')
          })

        cy.contains('span', 'or').should('be.visible')
      })

      addGatewayToBlock(0, 'adyen', 'mca_adyen')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates two rule blocks each targeting a different gateway', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)

      cy.contains('button', 'Add Rule').click()
      ruleBlock(0).find('input[placeholder="Rule name"]').clear().type('card-rule')
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      addGatewayToBlock(0, 'adyen', 'mca_adyen')

      cy.contains('button', 'Add Rule').click()
      ruleBlock(1).find('input[placeholder="Rule name"]').clear().type('upi-rule')
      ruleBlock(1).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('upi')
      })
      addGatewayToBlock(1, 'razorpay', 'mca_razorpay')

      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with an amount (integer) condition', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Add Rule').click()

      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('amount')
        cy.get('select.cond-select').eq(1).select('greater than')
        cy.get('input[type="number"]').type('100')
      })

      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addFallbackGateway('adyen', 'mca_adyen')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
      })
      cy.contains('Rule created').should('be.visible')
    })
  })

  // ── Existing rules panel ──────────────────────────────────────────────────

  describe('Existing rules panel', () => {
    beforeEach(() => {
      cy.createRoutingAlgorithm(
        factory.advancedRoutingPayload(merchantId, { name: ruleName }),
      ).then((res) => {
        expect(res.status).to.eq(200)
      })
      cy.visitWithMerchant('/routing/rules', merchantId)
      cy.contains('h1', 'Rule-Based Routing').should('be.visible')
    })

    it('shows the created rule as Inactive', () => {
      cy.contains(ruleName).should('be.visible')
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('Inactive')
        .should('be.visible')
    })

    it('shows a condition summary under the rule name', () => {
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .find('p.text-xs')
        .should('not.be.empty')
    })

    it('expands rule JSON when View is clicked', () => {
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('button', 'View')
        .click()
      cy.contains('Configuration').should('be.visible')
      cy.get('pre').should('be.visible')
    })

    it('hides rule JSON when Hide is clicked', () => {
      cy.contains(ruleName).closest('[class*="flex-col"]').contains('button', 'View').click()
      cy.contains('Configuration').should('be.visible')
      cy.contains(ruleName).closest('[class*="flex-col"]').contains('button', 'Hide').click()
      cy.contains('Configuration').should('not.exist')
    })

    it('activates the rule', () => {
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('button', 'Activate')
        .click()
      cy.contains('Rule activated successfully.').should('be.visible')
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('Active')
        .should('be.visible')
    })

    it('deactivates an active rule', () => {
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('button', 'Activate')
        .click()
      cy.contains('Rule activated successfully.').should('be.visible')

      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('button', 'Deactivate')
        .click()
      cy.contains('Rule deactivated successfully.').should('be.visible')
      cy.contains(ruleName)
        .closest('[class*="flex-col"]')
        .contains('Inactive')
        .should('be.visible')
    })

    it('shows Activate Now immediately after creating from the form', () => {
      cy.get('input[placeholder="my-rule"]').type(factory.ruleName('quick'))
      addFallbackGateway('stripe', 'mca_stripe')
      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).its('response.statusCode').should('eq', 200)
      cy.contains('button', 'Activate Now').should('be.visible')
    })

    it('activates a newly created rule via Activate Now', () => {
      const quickRule = factory.ruleName('quick')
      cy.get('input[placeholder="my-rule"]').type(quickRule)
      addFallbackGateway('stripe', 'mca_stripe')
      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).its('response.statusCode').should('eq', 200)
      cy.contains('button', 'Activate Now').click()
      cy.contains('Rule activated successfully.', { timeout: 15000 }).should('be.visible')
      cy.contains(quickRule)
        .closest('[class*="flex-col"]')
        .contains('Active')
        .should('be.visible')
    })
  })
})
