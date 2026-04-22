const factory = require('../../support/test-data-factory')

describe('Single Connector Routing API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('single_rule')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates, activates, lists, and evaluates a single connector algorithm', () => {
    const payload = factory.singleRoutingPayload(merchantId, {
      name: factory.ruleName('single_rule'),
      gateway: 'stripe',
    })
    let routingAlgorithmId

    cy.createRoutingAlgorithm(payload).then(({ response }) => {
      expect(response).to.haveValidRoutingAlgorithmCreateResponse()
      routingAlgorithmId = response.rule_id
      return cy.listRoutingAlgorithms(merchantId)
    }).then(({ response }) => {
      expect(response).to.haveValidRoutingAlgorithmList()
      expect(response.some((rule) => rule.id === routingAlgorithmId)).to.eq(true)
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).its('status').should('eq', 200)

    cy.listActiveRoutingAlgorithms(merchantId).then(({ response }) => {
      expect(response).to.haveValidRoutingAlgorithmList()
      expect(response.some((rule) => rule.id === routingAlgorithmId)).to.eq(true)
      return cy.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(merchantId))
    }).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.type).to.eq('straight_through')
      expect(response.output.connector.gateway_name).to.eq('stripe')
    })
  })
})
