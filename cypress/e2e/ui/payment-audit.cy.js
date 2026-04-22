const factory = require('../../support/test-data-factory')

function seedAuditData(merchantId) {
  const decisionPaymentId = factory.paymentId('audit_decision')
  const previewPaymentId = factory.paymentId('audit_preview')
  const advancedPayload = factory.advancedRoutingPayload(merchantId, {
    name: factory.ruleName('audit_adv'),
  })

  cy.ensureMerchantAccount(merchantId)
  cy.createSuccessRateConfig(merchantId)
  cy.createRoutingAlgorithm(advancedPayload).then(({ response }) => {
    cy.activateRoutingAlgorithm(merchantId, response.rule_id)
  })

  cy.decideGateway(
    factory.srDecideGatewayRequest({
      merchantId,
      paymentInfo: { paymentId: decisionPaymentId },
    }),
  ).then(({ response }) => {
    cy.updateGatewayScore(
      factory.updateGatewayScoreRequest({
        merchantId,
        gateway: response.decided_gateway,
        paymentId: decisionPaymentId,
        status: 'FAILURE',
      }),
    )
  })

  cy.evaluateRoutingAlgorithm(
    factory.ruleEvaluatePayload(
      merchantId,
      {
        payment_method: { type: 'enum_variant', value: 'card' },
        amount: { type: 'number', value: 250 },
      },
      { payment_id: previewPaymentId },
    ),
  )

  return { decisionPaymentId, previewPaymentId }
}

describe('Payment Audit UI', () => {
  let merchantId
  let seeded

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('audit_ui')
    seeded = seedAuditData(merchantId)
    cy.pollRequest(
      () =>
        cy.fetchPaymentAudit({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
          payment_id: seeded.decisionPaymentId,
        }),
      ({ response }) =>
        Array.isArray(response.timeline) &&
        response.timeline.some((event) => event.flow_type === 'decide_gateway_decision'),
      { errorMessage: 'Transaction audit seed data did not reach payment audit in time' },
    )
    cy.pollRequest(
      () =>
        cy.fetchPreviewTrace({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
          payment_id: seeded.previewPaymentId,
        }),
      ({ response }) =>
        Array.isArray(response.timeline) &&
        response.timeline.some((event) => event.flow_type === 'routing_evaluate_advanced'),
      { errorMessage: 'Preview audit seed data did not reach preview trace in time' },
    )
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('searches transaction and rule-based audit trails from the UI', () => {
    cy.visitWithMerchant('/audit', merchantId)

    cy.contains('Search Decision Trail').should('be.visible')
    cy.get('input[placeholder="Payment ID"]').clear().type(seeded.decisionPaymentId)
    cy.contains('button', 'Search').click()
    cy.contains(seeded.decisionPaymentId, { timeout: 20000 }).should('exist')
    cy.contains('button', 'View payload').should('be.visible')

    cy.visitWithMerchant('/audit?mode=rule_based', merchantId)
    cy.contains('Search Rule Preview Trail', { timeout: 20000 }).should('exist')
    cy.get('input[placeholder="Payment ID"]').clear().type(seeded.previewPaymentId)
    cy.contains('button', 'Search').click()
    cy.contains(seeded.previewPaymentId, { timeout: 20000 }).should('exist')
  })
})
