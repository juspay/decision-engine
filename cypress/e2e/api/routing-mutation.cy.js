const factory = require('../../support/test-data-factory')

describe('Routing Mutation Regression API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('routing_mutation')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('changes the selected connector after the active routing rule is replaced', () => {
    const firstPayload = factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName('single_stripe'),
      gateway: 'stripe',
    })
    const secondPayload = factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName('single_checkout'),
      gateway: 'checkout',
    })
    let firstRuleId
    let secondRuleId

    cy.createRoutingAlgorithm(firstPayload).then(({ response }) => {
      firstRuleId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, firstRuleId)
    }).then(() =>
      cy.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(merchantId)),
    ).then(({ response }) => {
      expect(response.output.type).to.eq('straight_through')
      expect(response.output.connector.gateway_name).to.eq('stripe')
      return cy.createRoutingAlgorithm(secondPayload)
    }).then(({ response }) => {
      secondRuleId = response.rule_id
      expect(secondRuleId).to.not.eq(firstRuleId)
      return cy.activateRoutingAlgorithm(merchantId, secondRuleId)
    }).then(() =>
      cy.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(merchantId)),
    ).then(({ response }) => {
      expect(response.output.type).to.eq('straight_through')
      expect(response.output.connector.gateway_name).to.eq('checkout')
    })
  })
})
