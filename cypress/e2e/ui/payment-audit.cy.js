const factory = require('../../support/test-data-factory')

function seedAuditData(merchantId) {
  const decisionPaymentId = factory.paymentId('audit_decision')
  const previewPaymentId = factory.paymentId('audit_preview')
  const advancedPayload = factory.advancedRoutingPayload(merchantId, {
    name: factory.ruleName('audit_adv'),
  })

  const seeded = { decisionPaymentId, previewPaymentId }

  return cy
    .ensureMerchantAccount(merchantId)
    .then(() => cy.createSuccessRateConfig(merchantId))
    .then(() => cy.createRoutingAlgorithm(advancedPayload))
    .then(({ response }) => cy.activateRoutingAlgorithm(merchantId, response.rule_id))
    .then(() =>
      cy.decideGateway(
        factory.srDecideGatewayRequest({
          merchantId,
          paymentInfo: { paymentId: decisionPaymentId },
        }),
      ),
    )
    .then(({ response }) =>
      cy.updateGatewayScore(
        factory.updateGatewayScoreRequest({
          merchantId,
          gateway: response.decided_gateway,
          paymentId: decisionPaymentId,
          status: 'FAILURE',
        }),
      ),
    )
    .then(() =>
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
    )
    .then(() => seeded)
}

describe('Payment Audit UI', () => {
  let merchantId
  let seeded

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('audit_ui')
    seedAuditData(merchantId)
      .then((data) => {
        seeded = data
      })
      .then(() =>
        cy.pollRequest(
          () =>
            cy.fetchPaymentAudit(
              {
                range: '1h',
                payment_id: seeded.decisionPaymentId,
              },
              { merchantId },
            ),
          ({ response }) =>
            Array.isArray(response.timeline) &&
            response.timeline.some((event) => event.flow_type === 'decide_gateway_decision'),
          { errorMessage: 'Transaction audit seed data did not reach payment audit in time' },
        ),
      )
      .then(() =>
        cy.pollRequest(
          () =>
            cy.fetchPreviewTrace(
              {
                range: '1h',
                payment_id: seeded.previewPaymentId,
              },
              { merchantId },
            ),
          ({ response }) =>
            Array.isArray(response.timeline) &&
            response.timeline.some((event) => event.flow_type === 'routing_evaluate_advanced'),
          { errorMessage: 'Preview audit seed data did not reach preview trace in time' },
        ),
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
