/**
 * euclid-rules-advanced.cy.js
 *
 * Tests for advanced rule builder features:
 *   1. Multi-value enum operators — "is one of" / "is not one of" → enum_variant_array
 *   2. Nested AND+OR branches (parent condition + nested OR sub-statements)
 *   3. Volume split output per rule (routing_type: volume_split)
 *   4. Volume split priority output per rule (routing_type: volume_split_priority)
 *
 * NOTE: cy.find() is a child command and CANNOT be used standalone inside .within().
 * Use cy.get() inside .within(), or chain .find() directly from the subject element.
 */

const factory = require('../../support/test-data-factory')
const {
  ruleBlock,
  thenSection,
  switchOutputType,
  addGatewayToBlock,
  addVolumeSplitEntry,
  addVolumeSplitPriorityRow,
  addGatewayToSplitRow,
  addFallbackGateway,
} = require('../../support/euclid-helpers')

describe('Advanced routing rule features', () => {
  let merchantId
  let ruleName

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('euclid_adv')
    ruleName = factory.ruleName('adv_rule')
    cy.ensureMerchantAccount(merchantId)

    // Intercept routing keys BEFORE visiting so the alias is registered in time.
    // Without this, "Add Rule" clicked while keys are still loading produces
    // default conditions with value='' which the backend rejects.
    cy.intercept('GET', '**/config/routing-keys').as('routingKeys')

    cy.visitWithMerchant('/routing/rules', merchantId)
    cy.contains('h1', 'Rule-Based Routing').should('be.visible')

    // Wait for routing keys to resolve before adding a rule block.
    cy.wait('@routingKeys', { timeout: 15000 })

    cy.contains('button', 'Add Rule').click()
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  // ── 1. "is one of" / "is not one of" operator (enum_variant_array) ─────────

  describe('"is one of" / "is not one of" operator', () => {
    beforeEach(() => {
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

  // ── 2. Nested AND+OR branches ─────────────────────────────────────────────

  describe('Nested AND+OR branches', () => {
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

  // ── 3. Volume split output ────────────────────────────────────────────────

  describe('Volume split output', () => {
    beforeEach(() => {
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

  // ── 4. Volume split priority output ──────────────────────────────────────

  describe('Volume split priority output', () => {
    beforeEach(() => {
      switchOutputType(0, 'Split + Priority')
    })

    it('switches the THEN section to split+priority mode', () => {
      thenSection(0).within(() => {
        cy.get('input[placeholder="Split %"]').should('be.visible')
        cy.contains('button', 'Add split').should('be.visible')
      })
    })

    it('adds a split row labelled "Split 1:"', () => {
      addVolumeSplitPriorityRow(0, 60)
      thenSection(0).within(() => {
        cy.contains('Split 1:').should('be.visible')
        cy.contains('Priority list for this split').should('be.visible')
      })
    })

    it('each split row has its own independent priority gateway editor', () => {
      addVolumeSplitPriorityRow(0, 60)
      addVolumeSplitPriorityRow(0, 40)
      thenSection(0).within(() => {
        cy.contains('Split 1:').should('be.visible')
        cy.contains('Split 2:').should('be.visible')
        cy.get('p').filter(':contains("Priority list for this split")').should('have.length', 2)
      })
    })

    it('adds gateways to a split row in priority order', () => {
      addVolumeSplitPriorityRow(0, 60)
      addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
      addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
      thenSection(0).within(() => {
        cy.contains('1. stripe').should('be.visible')
        cy.contains('2. adyen').should('be.visible')
      })
    })

    it('split rows maintain independent gateway lists', () => {
      addVolumeSplitPriorityRow(0, 60)
      addVolumeSplitPriorityRow(0, 40)
      addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
      addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
      thenSection(0).within(() => {
        cy.get('p').filter(':contains("Priority list for this split")').eq(0)
          .closest('[class*="p-3"]')
          .should('contain.text', 'stripe')
          .and('not.contain.text', 'checkout')
        cy.get('p').filter(':contains("Priority list for this split")').eq(1)
          .closest('[class*="p-3"]')
          .should('contain.text', 'checkout')
          .and('not.contain.text', 'stripe')
      })
    })

    it('shows running total and ✓ when splits sum to 100%', () => {
      addVolumeSplitPriorityRow(0, 60)
      addVolumeSplitPriorityRow(0, 40)
      thenSection(0).within(() => {
        cy.contains('Total: 100%').should('be.visible')
        cy.contains('✓').should('be.visible')
      })
    })

    it('shows warning when splits do not sum to 100%', () => {
      addVolumeSplitPriorityRow(0, 60)
      thenSection(0).contains('must equal 100%').should('be.visible')
    })

    it('removes a split row via its delete button', () => {
      addVolumeSplitPriorityRow(0, 60)
      addVolumeSplitPriorityRow(0, 40)
      thenSection(0).within(() => {
        cy.contains('Split 1:').closest('.rounded-lg.border').find('button').first().click()
        cy.get('p').filter(':contains("Priority list for this split")').should('have.length', 1)
      })
    })

    it('JSON preview emits routing_type: volume_split_priority with correct structure', () => {
      addVolumeSplitPriorityRow(0, 60)
      addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
      addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
      addVolumeSplitPriorityRow(0, 40)
      addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      cy.contains('button', 'Preview JSON').click()
      cy.get('pre').should('contain.text', '"routing_type": "volume_split_priority"')
      cy.get('pre').should('contain.text', '"volume_split_priority": [')
      cy.get('pre').should('contain.text', '"split": 60')
      cy.get('pre').should('contain.text', '"split": 40')
    })
  })

  // ── 5. End-to-end creation with API validation ────────────────────────────

  describe('End-to-end creation', () => {
    beforeEach(() => {
      cy.intercept('POST', '**/routing/create').as('createRule')
    })

    it('creates a rule using "is one of" — backend receives enum_variant_array', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(1).select('is one of')
        cy.get('input[type="checkbox"]').eq(0).check()
        cy.get('input[type="checkbox"]').eq(1).check()
      })
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addFallbackGateway('adyen', 'mca_adyen')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
        const condition = interception.request.body?.algorithm?.data?.rules?.[0]?.statements?.[0]?.condition?.[0]
        expect(condition.value.type).to.eq('enum_variant_array')
        expect(condition.value.value).to.be.an('array').with.length(2)
        expect(condition.comparison).to.eq('equal')
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule using "is not one of" — backend receives not_equal + enum_variant_array', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(1).select('is not one of')
        cy.get('input[type="checkbox"]').eq(0).check()
      })
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addFallbackGateway('adyen', 'mca_adyen')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(interception.response.statusCode).to.eq(200)
        const condition = interception.request.body?.algorithm?.data?.rules?.[0]?.statements?.[0]?.condition?.[0]
        expect(condition.value.type).to.eq('enum_variant_array')
        expect(condition.comparison).to.eq('not_equal')
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with one nested AND+OR branch — backend receives nested array', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('amount')
        cy.get('select.cond-select').eq(1).select('greater than')
        cy.get('input[type="number"]').type('10')
      })
      ruleBlock(0).contains('button', 'Add nested branch').click()
      ruleBlock(0).find('.border-l-2.border-sky-200').eq(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      addGatewayToBlock(0, 'rbl', 'mca_rbl')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
        const statement = interception.request.body?.algorithm?.data?.rules?.[0]?.statements?.[0]
        expect(statement.condition[0].lhs).to.eq('amount')
        expect(statement.nested).to.be.an('array').with.length(1)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with two nested OR branches — backend receives nested array of length 2', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('amount')
        cy.get('select.cond-select').eq(1).select('greater than')
        cy.get('input[type="number"]').type('10')
      })
      ruleBlock(0).contains('button', 'Add nested branch').click()
      ruleBlock(0).find('.border-l-2.border-sky-200').eq(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      ruleBlock(0).contains('button', 'Add nested branch').click()
      ruleBlock(0).find('.border-l-2.border-sky-200').eq(1).within(() => {
        cy.get('select.cond-select').eq(0).select('currency')
        // pick the first available currency value dynamically
        cy.get('select.cond-select').eq(2).find('option').eq(1).then(($opt) => {
          cy.get('select.cond-select').eq(2).select($opt.val())
        })
      })
      addGatewayToBlock(0, 'rbl', 'mca_rbl')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(interception.response.statusCode).to.eq(200)
        const statement = interception.request.body?.algorithm?.data?.rules?.[0]?.statements?.[0]
        expect(statement.nested).to.be.an('array').with.length(2)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with volume split output — backend receives routing_type: volume_split', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      switchOutputType(0, 'Volume Split')
      addVolumeSplitEntry(0, 60, 'stripe', 'mca_stripe')
      addVolumeSplitEntry(0, 40, 'adyen', 'mca_adyen')
      addFallbackGateway('checkout', 'mca_checkout')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(
          interception.response.statusCode,
          `POST /routing/create failed: ${JSON.stringify(interception.response.body)}`
        ).to.eq(200)
        const rule = interception.request.body?.algorithm?.data?.rules?.[0]
        expect(rule.routing_type).to.eq('volume_split')
        expect(rule.output.volume_split).to.be.an('array').with.length(2)
        expect(rule.output.volume_split[0].split).to.eq(60)
        expect(rule.output.volume_split[0].output.gateway_name).to.eq('stripe')
        expect(rule.output.volume_split[1].split).to.eq(40)
        expect(rule.output.volume_split[1].output.gateway_name).to.eq('adyen')
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule with volume split priority output — backend receives routing_type: volume_split_priority', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      switchOutputType(0, 'Split + Priority')
      addVolumeSplitPriorityRow(0, 60)
      addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
      addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
      addVolumeSplitPriorityRow(0, 40)
      addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
      addFallbackGateway('stripe', 'mca_stripe')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(interception.response.statusCode).to.eq(200)
        const rule = interception.request.body?.algorithm?.data?.rules?.[0]
        expect(rule.routing_type).to.eq('volume_split_priority')
        expect(rule.output.volume_split_priority).to.be.an('array').with.length(2)
        expect(rule.output.volume_split_priority[0].split).to.eq(60)
        expect(rule.output.volume_split_priority[0].output).to.be.an('array').with.length(2)
        expect(rule.output.volume_split_priority[1].output).to.be.an('array').with.length(1)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule combining nested AND+OR with volume split output', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('amount')
        cy.get('select.cond-select').eq(1).select('greater than')
        cy.get('input[type="number"]').type('100')
      })
      ruleBlock(0).contains('button', 'Add nested branch').click()
      ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(2).select('card')
      })
      switchOutputType(0, 'Volume Split')
      addVolumeSplitEntry(0, 70, 'stripe', 'mca_stripe')
      addVolumeSplitEntry(0, 30, 'adyen', 'mca_adyen')
      addFallbackGateway('checkout', 'mca_checkout')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(interception.response.statusCode).to.eq(200)
        const rule = interception.request.body?.algorithm?.data?.rules?.[0]
        expect(rule.routing_type).to.eq('volume_split')
        expect(rule.statements[0].nested).to.be.an('array').with.length(1)
        expect(rule.output.volume_split).to.be.an('array').with.length(2)
      })
      cy.contains('Rule created').should('be.visible')
    })

    it('creates a rule combining "is one of" with nested AND+OR', () => {
      cy.get('input[placeholder="my-rule"]').type(ruleName)
      ruleBlock(0).within(() => {
        cy.get('select.cond-select').eq(0).select('payment_method')
        cy.get('select.cond-select').eq(1).select('is one of')
        cy.get('input[type="checkbox"]').eq(0).check()
        cy.get('input[type="checkbox"]').eq(1).check()
      })
      ruleBlock(0).contains('button', 'Add nested branch').click()
      ruleBlock(0).find('.border-l-2.border-sky-200').first().within(() => {
        cy.get('select.cond-select').eq(0).select('currency')
        // pick the first available currency value dynamically
        cy.get('select.cond-select').eq(2).find('option').eq(1).then(($opt) => {
          cy.get('select.cond-select').eq(2).select($opt.val())
        })
      })
      addGatewayToBlock(0, 'stripe', 'mca_stripe')
      addFallbackGateway('adyen', 'mca_adyen')

      cy.contains('button', 'Create Rule').click()
      cy.wait('@createRule', { timeout: 15000 }).then((interception) => {
        expect(interception.response.statusCode).to.eq(200)
        const statement = interception.request.body?.algorithm?.data?.rules?.[0]?.statements?.[0]
        expect(statement.condition[0].value.type).to.eq('enum_variant_array')
        expect(statement.nested).to.be.an('array').with.length(1)
      })
      cy.contains('Rule created').should('be.visible')
    })
  })
})
