const factory = require('../../support/test-data-factory')
const {
  ruleBlock,
  switchOutputType,
  addGatewayToBlock,
  addVolumeSplitEntry,
  addVolumeSplitPriorityRow,
  addGatewayToSplitRow,
  addFallbackGateway,
} = require('../../support/euclid-helpers')

describe('End-to-end creation', () => {
  let merchantId
  let ruleName

  // Each test gets its own merchant so submitted rules never accumulate in
  // the page DOM across tests (which caused SIGSEGV on test 6 via re-render
  // of 5 previously saved rules when "Add Rule" was clicked).
  beforeEach(function () {
    cy.log(`[beforeEach] start — ${this.currentTest?.title}`)
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('euclid_e2e')
    ruleName = factory.ruleName('adv_rule')
    cy.ensureMerchantAccount(merchantId)
    cy.intercept('GET', '**/config/routing-keys').as('routingKeys')
    cy.intercept('POST', '**/routing/create').as('createRule')
    cy.log('[beforeEach] visiting page')
    cy.visitWithSession('/routing/rules', merchantId)
    cy.contains('h1', 'Rule-Based Routing').should('be.visible')
    cy.log('[beforeEach] waiting for routing keys')
    cy.wait('@routingKeys', { timeout: 15000 })
    cy.log('[beforeEach] clicking Add Rule')
    cy.contains('button', 'Add Rule').click()
    cy.log('[beforeEach] done')
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
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
    cy.log('[test] start')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.log('[test] typed rule name')
    ruleBlock(0).within(() => {
      cy.get('select.cond-select').eq(0).select('payment_method')
      cy.get('select.cond-select').eq(2).select('card')
    })
    cy.log('[test] set condition')
    switchOutputType(0, 'Split + Priority')
    cy.log('[test] switched to Split + Priority')
    addVolumeSplitPriorityRow(0, 60)
    cy.log('[test] added split row 1 (60%)')
    addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
    cy.log('[test] added stripe to split row 0')
    addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
    cy.log('[test] added adyen to split row 0')
    addVolumeSplitPriorityRow(0, 40)
    cy.log('[test] added split row 2 (40%)')
    addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
    cy.log('[test] added checkout to split row 1')
    addFallbackGateway('stripe', 'mca_stripe')
    cy.log('[test] added fallback — clicking Create Rule')

    cy.contains('button', 'Create Rule').click()
    cy.log('[test] waiting for API response')
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
