/**
 * Shared helpers for euclid-rules Cypress tests.
 * These are plain functions that call cy.* commands — require this file
 * directly from any spec that needs it.
 */

/** Scope to the nth rule block (0-indexed). */
function ruleBlock(index = 0) {
  return cy
    .get('input[placeholder="Rule name"]')
    .eq(index)
    .closest('.rounded-xl.overflow-hidden')
}

/**
 * Scope to the THEN section of the nth rule block.
 * Works regardless of the current output type (priority / volume_split / etc).
 */
function thenSection(blockIndex) {
  return ruleBlock(blockIndex)
    .contains('p', 'Then route')
    .closest('div[class*="py-4"]')
}

/** Switch the output type toggle for a rule block.
 *  typeLabel: 'Priority' | 'Volume Split' | 'Split + Priority'
 */
function switchOutputType(blockIndex, typeLabel) {
  thenSection(blockIndex).within(() => {
    cy.contains('button', typeLabel).click()
  })
}

/** Add a gateway to a rule block in Priority output mode. */
function addGatewayToBlock(blockIndex, gatewayName, gatewayId = '') {
  thenSection(blockIndex).within(() => {
    cy.get('input[placeholder="Gateway name"]').type(gatewayName)
    if (gatewayId) cy.get('input[placeholder="Gateway ID (optional)"]').type(gatewayId)
    cy.contains('button', 'Add').click()
  })
}

/** Add an entry to a rule block's Volume Split output.
 *  @param {number} split - percentage (e.g. 60)
 */
function addVolumeSplitEntry(blockIndex, split, gatewayName, gatewayId = '') {
  thenSection(blockIndex).within(() => {
    cy.get('input[placeholder="Split %"]').type(String(split))
    cy.get('input[placeholder="Gateway name"]').type(gatewayName)
    if (gatewayId) cy.get('input[placeholder="Gateway ID (optional)"]').type(gatewayId)
    cy.contains('button', 'Add').click()
  })
}

/** Add a split row to a rule block's Volume Split Priority output.
 *  @param {number} split - percentage for this split row (e.g. 60)
 */
function addVolumeSplitPriorityRow(blockIndex, split) {
  thenSection(blockIndex).within(() => {
    cy.get('input[placeholder="Split %"]').type(String(split))
    cy.contains('button', 'Add split').click()
  })
}

/** Add a gateway to the nth split row inside a Volume Split Priority output.
 *  @param {number} rowIndex - 0-indexed split row
 */
function addGatewayToSplitRow(blockIndex, rowIndex, gatewayName, gatewayId = '') {
  thenSection(blockIndex).within(() => {
    // cy.contains() returns only the first match — use cy.get().filter() to get all,
    // then .eq() to select the nth split row's priority list.
    cy.get('p').filter(':contains("Priority list for this split")')
      .eq(rowIndex)
      .closest('[class*="p-3"]')
      .within(() => {
        cy.get('input[placeholder="Gateway name"]').type(gatewayName)
        if (gatewayId) cy.get('input[placeholder="Gateway ID (optional)"]').type(gatewayId)
        cy.contains('button', 'Add').click()
      })
  })
}

/** Add a gateway to the Default Fallback section. */
function addFallbackGateway(gatewayName, gatewayId = '') {
  cy.contains('p', 'Default Fallback').closest('.rounded-xl').within(() => {
    cy.get('input[placeholder="Gateway name"]').type(gatewayName)
    if (gatewayId) cy.get('input[placeholder="Gateway ID (optional)"]').type(gatewayId)
    cy.contains('button', 'Add').click()
  })
}

/**
 * Select a routing-key (LHS) from the SearchableSelect dropdown.
 * Works inside or outside a cy.within() scope.
 * @param {number} index - 0-based index of cond-lhs within current scope
 * @param {string} value - routing key value to search and select (e.g. 'payment_method')
 */
function selectCondLhs(index, value) {
  cy.get('[data-cy="cond-lhs"]').eq(index).within(() => {
    cy.get('button.cond-select').click()
    cy.get('input[placeholder="Search…"]').clear().type(value)
    cy.get('button:not(.cond-select)').first().click()
  })
}

/**
 * Select an enum value from the SearchableSelect value dropdown.
 * Works inside or outside a cy.within() scope.
 * @param {number} index - 0-based index of cond-val within current scope
 * @param {string} value - enum value to search and select (e.g. 'card')
 */
function selectCondVal(index, value) {
  cy.get('[data-cy="cond-val"]').eq(index).within(() => {
    cy.get('button.cond-select').click()
    cy.get('input[placeholder="Search…"]').clear().type(value)
    cy.get('button:not(.cond-select)').first().click()
  })
}

module.exports = {
  ruleBlock,
  thenSection,
  switchOutputType,
  addGatewayToBlock,
  addVolumeSplitEntry,
  addVolumeSplitPriorityRow,
  addGatewayToSplitRow,
  addFallbackGateway,
  selectCondLhs,
  selectCondVal,
}
