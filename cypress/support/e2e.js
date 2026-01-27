// ***********************************************************
// This example support/e2e.js is processed and
// loaded automatically before your test files.
//
// This is a great place to put global configuration and
// behavior that modifies Cypress.
//
// You can change the location of this file or turn off
// automatically serving support files with the
// 'supportFile' configuration option.
//
// You can read more here:
// https://on.cypress.io/configuration
// ***********************************************************

// Import commands.js using ES2015 syntax:
import './commands'
import '@cypress/grep/src/support'

// Alternatively you can use CommonJS syntax:
// require('./commands')

// Global configuration
Cypress.on('uncaught:exception', (err, runnable) => {
  // returning false here prevents Cypress from
  // failing the test on uncaught exceptions
  return false
})

// Add custom assertions
chai.use(function (chai, utils) {
  chai.Assertion.addMethod('haveValidGatewayResponse', function () {
    const obj = this._obj
    
    expect(obj).to.have.property('decided_gateway')
    expect(obj).to.have.property('gateway_priority_map')
    expect(obj.gateway_priority_map).to.be.an('object')
  })

  chai.Assertion.addMethod('haveValidScoreUpdate', function () {
    const obj = this._obj
    
    expect(obj).to.have.property('message')
    expect(obj.message).to.equal('Success')
  })
})
