const factory = require('../../support/test-data-factory')
const {
  thenSection,
  switchOutputType,
  addVolumeSplitPriorityRow,
  addGatewayToSplitRow,
} = require('../../support/euclid-helpers')

describe('Volume split priority output', () => {
  let merchantId
  let ruleName

  before(() => {
    merchantId = factory.merchantId('euclid_vsp')
    cy.ensureMerchantAccount(merchantId)
  })

  after(() => {
    cy.cleanupTestData(merchantId)
  })

  beforeEach(() => {
    cy.waitForService()
    cy.viewport(1600, 1200)
    ruleName = factory.ruleName('adv_rule')
    cy.visitWithSession('/routing/rules', merchantId)
    cy.contains('Loading routing keys from backend...', { timeout: 15000 }).should('not.exist')
    cy.contains('button', 'Add Rule').click()
    switchOutputType(0, 'Split + Priority')
  })

  it('switches the THEN section to split+priority mode', () => {
    thenSection(0).within(() => {
      cy.get('input[placeholder="Split %"]').should('be.visible')
      cy.contains('button', 'Add split').should('be.visible')
    })
  })

  it('adds a split row labelled "Split 1:"', () => {
    addVolumeSplitPriorityRow(0, 60)
    thenSection(0).within(() => {
      cy.contains('Split 1:').should('be.visible')
      cy.contains('Priority list for this split').should('be.visible')
    })
  })

  it('each split row has its own independent priority gateway editor', () => {
    addVolumeSplitPriorityRow(0, 60)
    addVolumeSplitPriorityRow(0, 40)
    thenSection(0).within(() => {
      cy.contains('Split 1:').should('be.visible')
      cy.contains('Split 2:').should('be.visible')
      cy.get('p').filter(':contains("Priority list for this split")').should('have.length', 2)
    })
  })

  it('adds gateways to a split row in priority order', () => {
    addVolumeSplitPriorityRow(0, 60)
    addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
    addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
    thenSection(0).within(() => {
      cy.contains('1. stripe').should('be.visible')
      cy.contains('2. adyen').should('be.visible')
    })
  })

  it('split rows maintain independent gateway lists', () => {
    addVolumeSplitPriorityRow(0, 60)
    addVolumeSplitPriorityRow(0, 40)
    addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
    addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
    thenSection(0).within(() => {
      cy.get('p').filter(':contains("Priority list for this split")').eq(0)
        .closest('[class*="p-3"]')
        .should('contain.text', 'stripe')
        .and('not.contain.text', 'checkout')
      cy.get('p').filter(':contains("Priority list for this split")').eq(1)
        .closest('[class*="p-3"]')
        .should('contain.text', 'checkout')
        .and('not.contain.text', 'stripe')
    })
  })

  it('shows running total and ✓ when splits sum to 100%', () => {
    addVolumeSplitPriorityRow(0, 60)
    addVolumeSplitPriorityRow(0, 40)
    thenSection(0).within(() => {
      cy.contains('Total: 100%').should('be.visible')
      cy.contains('✓').should('be.visible')
    })
  })

  it('shows warning when splits do not sum to 100%', () => {
    addVolumeSplitPriorityRow(0, 60)
    thenSection(0).contains('must equal 100%').should('be.visible')
  })

  it('removes a split row via its delete button', () => {
    addVolumeSplitPriorityRow(0, 60)
    addVolumeSplitPriorityRow(0, 40)
    thenSection(0).within(() => {
      cy.contains('Split 1:').closest('.rounded-lg.border').find('button').first().click()
      cy.get('p').filter(':contains("Priority list for this split")').should('have.length', 1)
    })
  })

  it('JSON preview emits routing_type: volume_split_priority with correct structure', () => {
    addVolumeSplitPriorityRow(0, 60)
    addGatewayToSplitRow(0, 0, 'stripe', 'mca_stripe')
    addGatewayToSplitRow(0, 0, 'adyen', 'mca_adyen')
    addVolumeSplitPriorityRow(0, 40)
    addGatewayToSplitRow(0, 1, 'checkout', 'mca_checkout')
    cy.get('input[placeholder="my-rule"]').type(ruleName)
    cy.contains('button', 'Preview JSON').click()
    cy.get('pre').should('contain.text', '"routing_type": "volume_split_priority"')
    cy.get('pre').should('contain.text', '"volume_split_priority": [')
    cy.get('pre').should('contain.text', '"split": 60')
    cy.get('pre').should('contain.text', '"split": 40')
  })
})
