const factory = require('../../support/test-data-factory')

describe('Priority Routing API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('priority_rule')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates, activates, lists, and evaluates a priority algorithm preserving order', () => {
    const payload = factory.priorityRoutingPayload(merchantId, {
      name: factory.ruleName('priority_rule'),
      connectors: [
        factory.gatewayConnector('stripe'),
        factory.gatewayConnector('razorpay'),
        factory.gatewayConnector('adyen'),
      ],
    })
    let routingAlgorithmId

    cy.createRoutingAlgorithm(payload).then(({ response }) => {
      expect(response).to.haveValidRoutingAlgorithmCreateResponse()
      routingAlgorithmId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).its('status').should('eq', 200)

    cy.listRoutingAlgorithms(merchantId).then(({ response }) => {
      expect(response).to.haveValidRoutingAlgorithmList()
      const created = response.find((rule) => rule.id === routingAlgorithmId)
      expect(created).to.exist
      return cy.listActiveRoutingAlgorithms(merchantId)
    }).then(({ response }) => {
      expect(response.some((rule) => rule.id === routingAlgorithmId)).to.eq(true)
      return cy.evaluateRoutingAlgorithm(factory.ruleEvaluatePayload(merchantId))
    }).then(({ response }) => {
      expect(response.output.type).to.eq('priority')
      const gateways = response.output.connectors.map((connector) => connector.gateway_name)
      expect(gateways).to.deep.eq(['stripe', 'razorpay', 'adyen'])
    })
  })
})
