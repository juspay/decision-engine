const factory = require('../../support/test-data-factory')

function seedAnalyticsUiData(merchantId) {
  const decisionPaymentId = factory.paymentId('analytics_ui_decision')
  const previewPaymentId = factory.paymentId('analytics_ui_preview')
  const advancedPayload = factory.advancedRoutingPayload(merchantId, {
    name: factory.ruleName('analytics_ui_adv'),
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
        status: 'AUTHORIZED',
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

describe('Analytics UI', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('analytics_ui')
    seedAnalyticsUiData(merchantId)
    cy.pollRequest(
      () =>
        cy.fetchAnalyticsOverview(
          {
            range: '1h',
          },
          { merchantId },
        ),
      ({ response }) =>
        response.route_hits.some((hit) => hit.route === '/decide_gateway') &&
        response.route_hits.some((hit) => hit.route === '/update_gateway'),
      { errorMessage: 'Analytics UI seed data did not reach overview in time' },
    )
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('renders transaction and rule-based analytics with refresh state', () => {
    cy.visitWithMerchant('/analytics', merchantId)

    cy.contains('h1', 'Analytics').should('be.visible')
    cy.contains('h1', 'Analytics')
      .parents('div.space-y-6')
      .first()
      .within(() => {
        cy.contains('button', /^Transactions$/).should('be.visible')
        cy.contains('button', /^Rule-Based$/).should('be.visible')
      })

    cy.intercept('GET', '**/analytics/overview*', (req) => {
      req.continue((res) => res.setDelay(1200))
    }).as('overviewRefresh')
    cy.intercept('GET', '**/analytics/routing-stats*', (req) => {
      req.continue((res) => res.setDelay(1200))
    }).as('routingRefresh')

    cy.get('select').first().select('Last 1 week')
    cy.contains('button', 'Refresh').click()
    cy.contains(/Refreshing transaction analytics for/i).should('be.visible')
    cy.wait('@overviewRefresh')
    cy.wait('@routingRefresh')

    cy.contains('Gateway share over time').should('be.visible')
    cy.contains('Connector success rate over time').should('be.visible')

    cy.contains('h1', 'Analytics')
      .parents('div.space-y-6')
      .first()
      .within(() => {
        cy.contains('button', /^Rule-Based$/).scrollIntoView().click({ force: true })
      })
    cy.contains(
      'Preview-only activity for rule-based routing, separate from transaction decisions and score updates.',
    ).should('exist')
  })
})
