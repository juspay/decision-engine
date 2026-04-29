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
        cy.fetchAnalyticsOverview(
          {
            range: '1h',
          },
          { merchantId },
        ),
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

    // Wait for initial analytics to load before triggering refresh
    cy.contains('Top gateway').should('exist')

    cy.intercept('GET', '**/analytics/overview*', (req) => {
      req.continue((res) => res.setDelay(3000))
    }).as('overviewRefresh')
    cy.intercept('GET', '**/analytics/routing-stats*', (req) => {
      req.continue((res) => res.setDelay(3000))
    }).as('routingRefresh')

    cy.contains('button', '1 week').click()
    // Check for the "Loading" badge that appears during refresh
    cy.contains('Loading').should('be.visible')
    cy.wait('@overviewRefresh')
    cy.wait('@routingRefresh')

    cy.contains('Top gateway').scrollIntoView().should('be.visible')
    cy.contains('button', 'Analytics').click()
    cy.url().should('include', '/analytics')
    cy.contains('h1', 'Analytics').should('exist')
  })
})
