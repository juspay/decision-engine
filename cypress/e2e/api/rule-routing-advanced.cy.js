const factory = require('../../support/test-data-factory')

describe('Advanced Routing API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('advanced_rule')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('evaluates default-selection and matched rule paths for a simple advanced algorithm', () => {
    const payload = factory.advancedRoutingPayload(merchantId, {
      name: factory.ruleName('advanced_simple'),
    })
    let routingAlgorithmId

    cy.createRoutingAlgorithm(payload).then(({ response }) => {
      routingAlgorithmId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).then(() =>
      cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(merchantId, {
          payment_method: { type: 'enum_variant', value: 'card' },
          amount: { type: 'number', value: 150 },
        }),
      ),
    ).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.type).to.eq('priority')
      expect(response.output.connectors[0].gateway_name).to.eq('checkout')
      return cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(merchantId, {
          payment_method: { type: 'enum_variant', value: 'upi' },
          amount: { type: 'number', value: 50 },
        }),
      )
    }).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.type).to.eq('priority')
      expect(response.output.connectors[0].gateway_name).to.eq('stripe')
    })
  })

  it('supports nested AND/OR style routing evaluation via nested statements', () => {
    const payload = factory.advancedNestedAndOrRoutingPayload(merchantId, {
      name: factory.ruleName('advanced_nested'),
    })
    let routingAlgorithmId

    cy.createRoutingAlgorithm(payload).then(({ response }) => {
      routingAlgorithmId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).then(() =>
      cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(merchantId, {
          payment_method: { type: 'enum_variant', value: 'card' },
          card_network: { type: 'enum_variant', value: 'visa' },
        }),
      ),
    ).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.connectors[0].gateway_name).to.eq('stripe')
      return cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(merchantId, {
          payment_method: { type: 'enum_variant', value: 'card' },
          currency: { type: 'enum_variant', value: 'USD' },
        }),
      )
    }).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.connectors[0].gateway_name).to.eq('stripe')
      return cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(merchantId, {
          payment_method: { type: 'enum_variant', value: 'upi' },
          currency: { type: 'enum_variant', value: 'USD' },
        }),
      )
    }).then(({ response }) => {
      expect(response.status).to.be.oneOf(['success', 'default_selection'])
      expect(response.output.connectors[0].gateway_name).to.eq('checkout')
    })
  })
})
