describe('Forgot password UI', () => {
  const clearAuthStorage = {
    onBeforeLoad(win) {
      win.localStorage.removeItem('auth-store')
      win.localStorage.removeItem('merchant-store')
    },
  }

  beforeEach(() => {
    cy.waitForService()
  })

  it('links from the login page to the forgot-password form', () => {
    cy.visitAppPath('/login', clearAuthStorage)

    cy.contains('a', 'Forgot password?').should('be.visible').click()
    cy.location('pathname').should('include', '/forgot-password')
    cy.contains('Forgot your password?').should('be.visible')
    cy.contains('button', 'Send reset link').should('be.visible')
  })

  it('shows the generic confirmation after requesting a reset link', () => {
    cy.intercept('POST', '**/decision-engine-api/auth/forgot-password', {
      statusCode: 200,
      body: { message: 'If an account exists for that email, a password reset link has been sent.' },
    }).as('forgotPassword')

    cy.visitAppPath('/forgot-password', clearAuthStorage)

    cy.get('input[type="email"]').type('someone@example.com')
    cy.contains('button', 'Send reset link').click()

    cy.wait('@forgotPassword')
    cy.contains('Check your inbox').should('be.visible')
    cy.contains('someone@example.com').should('be.visible')
    cy.contains('expires in 30 minutes').should('be.visible')
  })

  it('shows the missing-token card when opened without a token', () => {
    cy.visitAppPath('/reset-password', clearAuthStorage)

    cy.contains('Reset link is incomplete').should('be.visible')
    cy.contains('button', 'Request a new link').click()
    cy.location('pathname').should('include', '/forgot-password')
  })

  it('strips the token from the URL and blocks mismatched passwords without a request', () => {
    cy.intercept('POST', '**/decision-engine-api/auth/reset-password', cy.spy().as('resetCall'))

    cy.visitAppPath('/reset-password?token=test-token-123', clearAuthStorage)

    cy.contains('Choose a new password').should('be.visible')
    cy.location('search').should('not.contain', 'token')

    cy.get('input[type="password"]').first().type('ValidPass1!')
    cy.get('input[type="password"]').eq(1).type('DifferentPass1!')
    cy.contains('button', 'Reset password').click()

    cy.contains('Passwords do not match.').should('be.visible')
    cy.get('@resetCall').should('not.have.been.called')
  })

  it('redirects to login with a success notice after resetting', () => {
    cy.intercept('POST', '**/decision-engine-api/auth/reset-password', {
      statusCode: 200,
      body: { message: 'Password reset successfully. You can now sign in with your new password.' },
    }).as('resetPassword')

    cy.visitAppPath('/reset-password?token=test-token-123', clearAuthStorage)

    cy.get('input[type="password"]').first().type('ValidPass1!')
    cy.get('input[type="password"]').eq(1).type('ValidPass1!')
    cy.contains('button', 'Reset password').click()

    cy.wait('@resetPassword')
      .its('request.body')
      .should('deep.include', { token: 'test-token-123', new_password: 'ValidPass1!' })
    cy.location('pathname').should('include', '/login')
    cy.contains('Password reset successfully. Sign in with your new password.').should('be.visible')
  })

  it('offers a fresh link when the token is invalid or expired', () => {
    cy.intercept('POST', '**/decision-engine-api/auth/reset-password', {
      statusCode: 400,
      body: { message: 'Invalid or expired password reset link' },
    }).as('resetPassword')

    cy.visitAppPath('/reset-password?token=used-token', clearAuthStorage)

    cy.get('input[type="password"]').first().type('ValidPass1!')
    cy.get('input[type="password"]').eq(1).type('ValidPass1!')
    cy.contains('button', 'Reset password').click()

    cy.wait('@resetPassword')
    cy.contains('Invalid or expired password reset link').should('be.visible')
    cy.contains('button', 'Request a new link').should('be.visible').click()
    cy.location('pathname').should('include', '/forgot-password')
  })
})
