const factory = require('../../support/test-data-factory')

function seedOverviewData(merchantId) {
  const paymentId = factory.paymentId('overview')

  cy.ensureMerchantAccount(merchantId)
  cy.createSuccessRateConfig(merchantId)
  cy.decideGateway(
    factory.srDecideGatewayRequest({
      merchantId,
      paymentInfo: { paymentId },
    }),
  ).then(({ response }) => {
    cy.updateGatewayScore(
      factory.updateGatewayScoreRequest({
        merchantId,
        gateway: response.decided_gateway,
        paymentId,
        status: 'AUTHORIZED',
        txnLatency: { gatewayLatency: 2200 },
      }),
    )
  })
}

describe('Dashboard Overview UI', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('overview_ui')
    seedOverviewData(merchantId)
    cy.pollRequest(
      () =>
        cy.fetchAnalyticsOverview({
          scope: 'current',
          range: '1h',
          merchant_id: merchantId,
        }),
      ({ response }) => response.route_hits.some((hit) => hit.route === '/decide_gateway'),
      { errorMessage: 'Overview seed data did not reach analytics in time' },
    )
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('renders overview content and shows the refresh state on range change', () => {
    cy.visitWithMerchant('/', merchantId)

    cy.contains('h1', 'Overview').should('exist')
    cy.contains('Current setup').should('exist')
    cy.contains('Gateway activity').should('exist')
    cy.contains(merchantId).should('be.visible')

    cy.intercept('GET', '**/analytics/overview*', (req) => {
      req.continue((res) => res.setDelay(1000))
    }).as('overviewRefresh')
    cy.intercept('GET', '**/analytics/routing-stats*', (req) => {
      req.continue((res) => res.setDelay(1000))
    }).as('routingRefresh')

    cy.contains('button', '18 months').click()
    cy.contains(/Refreshing overview analytics for/i).should('be.visible')
    cy.wait('@overviewRefresh')
    cy.wait('@routingRefresh')

    cy.contains('Top gateway').should('be.visible')
    cy.contains('button', 'Analytics').click()
    cy.url().should('include', '/analytics')
    cy.contains('h1', 'Analytics').should('exist')
  })
})
