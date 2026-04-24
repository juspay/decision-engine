const factory = require('../../support/test-data-factory')

describe('Dynamic Routing API', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('dynamic_routing')
    cy.ensureMerchantAccount(merchantId)
    cy.createSuccessRateConfig(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('decides a gateway, updates connector feedback, and records analytics trail', () => {
    const firstPaymentId = factory.paymentId('dynamic_first')
    const secondPaymentId = factory.paymentId('dynamic_second')
    let chosenGateway
    let initialScore

    cy.decideGateway(
      factory.srDecideGatewayRequest({
        merchantId,
        paymentInfo: {
          paymentId: firstPaymentId,
          paymentMethodType: 'UPI',
          paymentMethod: 'UPI_PAY',
        },
      }),
    )
      .then(({ response }) => {
        expect(response).to.haveValidGatewayResponse()
        chosenGateway = response.decided_gateway
        initialScore = response.gateway_priority_map[chosenGateway]
        expect(chosenGateway).to.be.a('string')

        return cy.updateGatewayScore(
          factory.updateGatewayScoreRequest({
            merchantId,
            gateway: chosenGateway,
            paymentId: firstPaymentId,
            status: 'FAILURE',
            txnLatency: { gatewayLatency: 8000 },
          }),
        )
      })
      .then(({ response }) => {
        expect(response).to.haveValidScoreUpdate()
        expect(response.gateway).to.eq(chosenGateway)
        expect(response.payment_id).to.eq(firstPaymentId)

        return cy.decideGateway(
          factory.srDecideGatewayRequest({
            merchantId,
            paymentInfo: {
              paymentId: secondPaymentId,
              paymentMethodType: 'UPI',
              paymentMethod: 'UPI_PAY',
            },
          }),
        )
      })
      .then(({ response }) => {
        expect(response).to.haveValidGatewayResponse()
        expect(response.gateway_priority_map[chosenGateway]).to.be.at.most(initialScore)
      })
      .then(() =>
        cy.pollRequest(
          () =>
            cy.fetchPaymentAudit(
              {
                range: '1h',
                payment_id: firstPaymentId,
              },
              { merchantId },
            ),
          ({ response }) =>
            Array.isArray(response.timeline) &&
            response.timeline.some((event) => event.flow_type === 'decide_gateway_decision') &&
            response.timeline.some((event) => event.flow_type === 'update_gateway_score_update'),
          { errorMessage: 'Expected payment audit decision + gateway update trail' },
        ),
      )
      .then(({ response }) => {
        const flowTypes = response.timeline.map((event) => event.flow_type)
        expect(flowTypes).to.include('decide_gateway_decision')
        expect(flowTypes).to.include('update_gateway_score_update')
      })
      .then(() =>
        cy.pollRequest(
          () =>
            cy.fetchAnalyticsOverview(
              {
                range: '1h',
              },
              { merchantId },
            ),
          ({ response }) =>
            Array.isArray(response.route_hits) &&
            response.route_hits.some((hit) => hit.route === '/decide_gateway' && hit.count >= 2) &&
            response.route_hits.some((hit) => hit.route === '/update_gateway' && hit.count >= 1),
          { errorMessage: 'Expected dynamic routing route hits in analytics overview' },
        ),
      )
      .then(({ response }) => {
        expect(response).to.haveValidAnalyticsOverview()
      })
  })
})
