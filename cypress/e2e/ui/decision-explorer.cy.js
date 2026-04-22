const factory = require('../../support/test-data-factory')

function activateAdvancedRule(merchantId) {
  const payload = factory.advancedRoutingPayload(merchantId, {
    name: factory.ruleName('decision_explorer_adv'),
  })

  return cy.createRoutingAlgorithm(payload).then(({ response }) =>
    cy.activateRoutingAlgorithm(merchantId, response.rule_id),
  )
}

function activateVolumeRule(merchantId) {
  const payload = factory.volumeSplitRoutingPayload(merchantId, {
    name: factory.ruleName('decision_explorer_volume'),
    data: [
      { split: 50, output: factory.gatewayConnector('stripe') },
      { split: 50, output: factory.gatewayConnector('adyen') },
    ],
  })

  return cy.createRoutingAlgorithm(payload).then(({ response }) =>
    cy.activateRoutingAlgorithm(merchantId, response.rule_id),
  )
}

describe('Decision Explorer UI', () => {
  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
  })

  it('runs an auth-rate based simulation flow from the UI', () => {
    const merchantId = factory.merchantId('decision_explorer_batch')
    cy.ensureMerchantAccount(merchantId)
    cy.createSuccessRateConfig(merchantId)

    cy.visitWithMerchant('/decisions', merchantId)
    cy.contains('h1', 'Decision Explorer').should('exist')
    cy.contains('button', 'Auth-Rate Based Routing').click()
    cy.contains('button', 'Run Auth-Rate Simulation').click()
    cy.contains('View audit').should('be.visible')

    cy.cleanupTestData(merchantId)
  })

  it('runs rule-based evaluation from the UI', () => {
    const merchantId = factory.merchantId('decision_explorer_rule')
    cy.ensureMerchantAccount(merchantId)
    activateAdvancedRule(merchantId)

    cy.visitWithMerchant('/decisions', merchantId)
    cy.contains('button', 'Rule Based Routing').click()
    cy.contains('button', 'Evaluate Rules').click()
    cy.contains(/View preview trace|Rule Evaluation Preview/).should('be.visible')

    cy.cleanupTestData(merchantId)
  })

  it('runs volume split evaluation from the UI', () => {
    const merchantId = factory.merchantId('decision_explorer_volume')
    cy.ensureMerchantAccount(merchantId)
    activateVolumeRule(merchantId)

    cy.visitWithMerchant('/decisions', merchantId)
    cy.contains('button', 'Volume Based Routing').click()
    cy.get('input').filter('[value="100"]').first().clear().type('20')
    cy.contains('button', 'Run Volume Evaluation').click()
    cy.contains('Actual distribution').should('be.visible')

    cy.cleanupTestData(merchantId)
  })
})
