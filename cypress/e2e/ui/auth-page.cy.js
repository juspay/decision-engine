const factory = require('../../support/test-data-factory')

describe('Auth UI', () => {
  let merchantId
  let email

  beforeEach(() => {
    merchantId = factory.merchantId('auth_ui')
    email = `${merchantId}@example.com`
    cy.waitForService()
    cy.ensureMerchantAccount(merchantId).then(() => cy.ensureDashboardSession(merchantId))
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('renders the auth page and respects a valid seeded session', () => {
    cy.visitAppPath('/login')

    cy.window().then((win) => {
      win.localStorage.removeItem('auth-store')
      win.localStorage.removeItem('merchant-store')
    })

    cy.contains('h1', 'Hey there, Welcome back!').should('be.visible')
    cy.contains('button', 'Continue').should('be.visible')
    cy.visitWithSession('/', merchantId)
    cy.contains(email, { timeout: 20000 }).should('be.visible')
    cy.contains(merchantId).should('be.visible')
  })
})
