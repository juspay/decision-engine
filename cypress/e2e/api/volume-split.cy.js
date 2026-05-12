const factory = require('../../support/test-data-factory')

function extractConnector(response) {
  return (
    response.evaluated_output?.[0]?.gateway_name ||
    response.output.connector?.gateway_name ||
    response.output.connectors?.[0]?.gateway_name ||
    null
  )
}

describe('Volume Split Routing API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('volume_split')
    cy.ensureMerchantAccount(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('creates, activates, evaluates, and approximates configured volume split', () => {
    const payload = factory.volumeSplitRoutingPayload(merchantId, {
      name: factory.ruleName('volume_split'),
      data: [
        { split: 70, output: factory.gatewayConnector('stripe') },
        { split: 30, output: factory.gatewayConnector('paytm') },
      ],
    })
    let routingAlgorithmId
    const counts = new Map()

    cy.createRoutingAlgorithm(payload).then(({ response }) => {
      routingAlgorithmId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).then(() => {
      Cypress._.times(100, (index) => {
        cy.evaluateRoutingAlgorithm(
          factory.ruleEvaluatePayload(
            merchantId,
            {},
            { payment_id: factory.paymentId(`volume_eval_${index}`) },
          ),
        ).then(({ response }) => {
          expect(response.output.type).to.eq('volume_split')
          const connector = extractConnector(response)
          expect(connector).to.be.oneOf(['stripe', 'paytm'])
          counts.set(connector, (counts.get(connector) || 0) + 1)
        })
      })
    })

    cy.then(() => {
      const stripeCount = counts.get('stripe') || 0
      const paytmCount = counts.get('paytm') || 0

      // Tolerance of ±20 around the configured 70/30 split (≈ ±4σ for n=100,p=0.7)
      // keeps the failure rate below 0.001% while still catching a broken distribution.
      expect(stripeCount).to.be.within(50, 90)
      expect(paytmCount).to.be.within(10, 50)
    })
  })
})
