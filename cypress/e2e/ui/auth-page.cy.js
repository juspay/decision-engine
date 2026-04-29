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

    cy.contains('h2', 'Manage routing, analytics, and audits from one dashboard.').should('be.visible')
    cy.contains('Welcome back').should('be.visible')
    cy.contains('button', 'Enter workspace').should('be.visible')
    cy.visitWithSession('/', merchantId)
    cy.contains(email, { timeout: 20000 }).should('be.visible')
    cy.contains(merchantId).should('be.visible')
  })

  it('keeps the sign-up tab active across refresh', () => {
    cy.visitAppPath('/login', {
      onBeforeLoad(win) {
        win.localStorage.removeItem('auth-store')
        win.localStorage.removeItem('merchant-store')
      },
    })

    cy.contains('button', 'Sign up').click()
    cy.location('pathname').should('include', '/signup')
    cy.contains('Create account').should('be.visible')

    cy.reload()

    cy.location('pathname').should('include', '/signup')
    cy.contains('Create account').should('be.visible')
    cy.contains('button', 'Create account').should('be.visible')
  })

  it('switches duplicate sign-up attempts to sign-in with email preserved', () => {
    const duplicateEmail = `duplicate-${merchantId}@example.com`

    cy.intercept('POST', '**/decision-engine-api/auth/signup', {
      statusCode: 409,
      body: { message: 'Email already registered' },
    }).as('duplicateSignup')

    cy.visitAppPath('/login', {
      onBeforeLoad(win) {
        win.localStorage.removeItem('auth-store')
        win.localStorage.removeItem('merchant-store')
      },
    })

    cy.contains('button', 'Sign up').click()
    cy.get('input[type="email"]').clear().type(duplicateEmail)
    cy.get('input[placeholder="e.g. Acme Corp"]').clear().type('Venom')
    cy.get('input[placeholder="Enter your password"]').clear().type('ValidPass1!')
    cy.contains('button', 'Create account').click()

    cy.wait('@duplicateSignup')
    cy.contains('Welcome back').should('be.visible')
    cy.contains('Account already exists. Sign in with this email.').should('be.visible')
    cy.get('input[type="email"]').should('have.value', duplicateEmail)
    cy.get('input[placeholder="Enter your password"]').should('be.focused')
  })
})
