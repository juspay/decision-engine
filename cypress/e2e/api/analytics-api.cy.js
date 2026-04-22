const factory = require('../../support/test-data-factory')

describe('Analytics API', () => {
  let merchantId
  let decisionPaymentId
  let previewPaymentId

  beforeEach(() => {
    cy.waitForService()
    merchantId = factory.merchantId('analytics_api')
    decisionPaymentId = factory.paymentId('analytics_decision')
    previewPaymentId = factory.paymentId('analytics_preview')

    cy.ensureMerchantAccount(merchantId)
    cy.createSuccessRateConfig(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('returns populated overview, routing stats, payment audit, and preview trace after traffic is generated', () => {
    const advancedPayload = factory.advancedRoutingPayload(merchantId, {
      name: factory.ruleName('analytics_advanced'),
    })
    let routingAlgorithmId

    cy.createRoutingAlgorithm(advancedPayload).then(({ response }) => {
      routingAlgorithmId = response.rule_id
      return cy.activateRoutingAlgorithm(merchantId, routingAlgorithmId)
    }).then(() =>
      cy.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId,
          paymentInfo: {
            paymentId: decisionPaymentId,
          },
        }),
      ),
    ).then(({ response }) => {
      return cy.updateGatewayScore(
        factory.updateGatewayScoreRequest({
          merchantId,
          gateway: response.decided_gateway,
          paymentId: decisionPaymentId,
          status: 'AUTHORIZED',
          txnLatency: { gatewayLatency: 2500 },
        }),
      )
    }).then(() =>
      cy.evaluateRoutingAlgorithm(
        factory.ruleEvaluatePayload(
          merchantId,
          {
            payment_method: { type: 'enum_variant', value: 'card' },
            amount: { type: 'number', value: 250 },
          },
          { payment_id: previewPaymentId },
        ),
      ),
    ).then(({ response }) => {
      expect(response.output.type).to.eq('priority')
    })

    cy.pollRequest(
      () =>
        cy.fetchAnalyticsOverview({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
        }),
      ({ response }) =>
        response.route_hits.some((hit) => hit.route === '/decide_gateway') &&
        response.route_hits.some((hit) => hit.route === '/update_gateway') &&
        response.route_hits.some((hit) => hit.route === '/rule_evaluate'),
      { errorMessage: 'Analytics overview did not show decision/update traffic' },
    ).then(({ response }) => {
      expect(response).to.haveValidAnalyticsOverview()
    })

    cy.pollRequest(
      () =>
        cy.fetchAnalyticsRoutingStats({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
        }),
      ({ response }) =>
        Array.isArray(response.gateway_share) && response.gateway_share.length > 0,
      { errorMessage: 'Analytics routing stats did not populate gateway share' },
    ).then(({ response }) => {
      expect(response).to.haveValidRoutingStats()
    })

    cy.pollRequest(
      () =>
        cy.fetchPaymentAudit({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
          payment_id: decisionPaymentId,
        }),
      ({ response }) =>
        response.timeline.some((event) => event.flow_type === 'decide_gateway_decision') &&
        response.timeline.some((event) => event.flow_type === 'update_gateway_score_update'),
      { errorMessage: 'Payment audit did not include decision + gateway update' },
    ).then(({ response }) => {
      expect(response).to.haveValidPaymentAudit()
    })

    cy.pollRequest(
      () =>
        cy.fetchPreviewTrace({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
          payment_id: previewPaymentId,
        }),
      ({ response }) =>
        response.timeline.some((event) => event.flow_type === 'routing_evaluate_advanced'),
      { errorMessage: 'Preview trace did not include rule evaluation preview' },
    ).then(({ response }) => {
      expect(response).to.haveValidPaymentAudit()
    })
  })
})
