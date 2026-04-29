const factory = require('../../support/test-data-factory')

describe('API Keys UI', () => {
  let merchantId

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    merchantId = factory.merchantId('api_keys_ui')
    cy.ensureMerchantAccount(merchantId)
    cy.createSuccessRateConfig(merchantId)
  })

  afterEach(() => {
    cy.cleanupTestData(merchantId)
  })

  it('renders the API keys page with create form and empty list', () => {
    cy.visitWithMerchant('/api-keys', merchantId)

    cy.contains('h1', 'API Keys').should('be.visible')
    cy.contains('x-api-key').should('be.visible')
    cy.get('input[placeholder*="Description"]').should('be.visible')
    cy.contains('button', 'Create API Key').should('be.visible')
    cy.contains('No active API keys').should('be.visible')
  })

  it('creates an API key, shows it once, and lists it', () => {
    cy.visitWithMerchant('/api-keys', merchantId)

    cy.get('input[placeholder*="Description"]').type('cypress-integration-key')
    cy.contains('button', 'Create API Key').click()

    cy.get('[data-testid="api-key-value"]', { timeout: 10000 }).should('be.visible').invoke('text').then((rawKey) => {
      expect(rawKey.trim()).to.match(/^DE_/)
    })

    cy.contains('API key created — copy it now').should('be.visible')
    cy.contains('cypress-integration-key').should('be.visible')
    cy.contains('button', 'Revoke').should('be.visible')
  })

  it('creates an API key via UI and uses it to call a routing endpoint', () => {
    cy.visitWithMerchant('/api-keys', merchantId)

    cy.get('input[placeholder*="Description"]').type('routing-test-key')
    cy.contains('button', 'Create API Key').click()

    cy.get('[data-testid="api-key-value"]', { timeout: 10000 })
      .should('be.visible')
      .invoke('text')
      .then((rawKey) => {
        const apiKey = rawKey.trim()
        expect(apiKey).to.match(/^DE_/)

        cy.decideGatewayWithApiKey(
          apiKey,
          factory.srDecideGatewayRequest({
            merchantId,
            paymentInfo: { paymentId: factory.paymentId('apikey') },
          }),
        ).then((response) => {
          expect(response.status).to.eq(200)
          expect(response.body).to.have.property('decided_gateway')
        })
      })
  })

  it('revokes an API key and verifies it disappears from the list', () => {
    cy.visitWithMerchant('/api-keys', merchantId)

    cy.get('input[placeholder*="Description"]').type('to-be-revoked')
    cy.contains('button', 'Create API Key').click()
    cy.get('[data-testid="api-key-value"]', { timeout: 10000 }).should('be.visible')

    // Find the specific row with our key and click its Revoke button
    cy.contains('td', 'to-be-revoked').parent('tr').within(() => {
      cy.contains('button', 'Revoke').click()
    })

    // Verify the key is removed from the table
    cy.contains('td', 'to-be-revoked').should('not.exist')
  })
})
