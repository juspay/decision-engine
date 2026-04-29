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
      .parents('div.space-y-8')
      .first()
      .within(() => {
        cy.contains('button', /^Auth-rate based$/).should('be.visible')
        cy.contains('button', /^Rule based \/ Volume based$/).should('be.visible')
      })

    cy.intercept('GET', '**/analytics/overview*', (req) => {
      req.continue((res) => res.setDelay(1200))
    }).as('overviewRefresh')
    cy.intercept('GET', '**/analytics/routing-stats*', (req) => {
      req.continue((res) => res.setDelay(1200))
    }).as('routingRefresh')

    cy.contains('button', '1w').click()
    cy.contains('button', 'Refresh').click()
    // The analytics page doesn't show a "Refreshing" text, but the network requests are intercepted
    cy.wait('@overviewRefresh')
    cy.wait('@routingRefresh')
    // Wait for the view to settle and API data to load
    cy.contains('button', /^Auth-rate based$/).should('have.class', 'bg-white')

    // Wait for analytics data to load (looking for endpoint hit cards)
    cy.contains('Decide Gateway').should('be.visible')

    // Switch to rule-based view
    cy.contains('h1', 'Analytics')
      .parents('div.space-y-8')
      .first()
      .within(() => {
        cy.contains('button', /^Rule based \/ Volume based$/).scrollIntoView().click({ force: true })
      })
    cy.contains(
      'Routing decisions from /routing/evaluate, kept separate from auth-rate transaction routing and gateway scoring.',
    ).should('exist')
  })
})
