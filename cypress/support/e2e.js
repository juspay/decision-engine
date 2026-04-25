import './commands'
import '@cypress/grep/src/support'

Cypress.on('uncaught:exception', () => false)

chai.use(function (_chai, utils) {
  function assertObject(obj) {
    new _chai.Assertion(obj).to.be.an('object')
  }

  _chai.Assertion.addMethod('haveValidMerchantCreateResponse', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('message', 'Merchant account created successfully')
    expect(obj).to.have.property('merchant_id').that.is.a('string')
  })

  _chai.Assertion.addMethod('haveValidMerchantGetResponse', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('merchant_id').that.is.a('string')
    expect(obj).to.have.property('gateway_success_rate_based_decider_input')
  })

  _chai.Assertion.addMethod('haveValidMerchantDeleteResponse', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('message', 'Merchant account deleted successfully')
    expect(obj).to.have.property('merchant_id').that.is.a('string')
  })

  _chai.Assertion.addMethod('haveValidGatewayResponse', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('decided_gateway').that.is.a('string')
    expect(obj).to.have.property('gateway_priority_map').that.is.an('object')
    expect(obj).to.have.property('routing_approach').that.is.a('string')
  })

  _chai.Assertion.addMethod('haveValidScoreUpdate', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('message', 'Gateway score updated successfully')
    expect(obj).to.have.property('merchant_id').that.is.a('string')
    expect(obj).to.have.property('gateway').that.is.a('string')
    expect(obj).to.have.property('payment_id').that.is.a('string')
  })

  _chai.Assertion.addMethod('haveValidRuleConfigResponse', function (expectedType) {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('merchant_id').that.is.a('string')
    expect(obj).to.have.property('config').that.is.an('object')
    if (expectedType) {
      expect(obj.config).to.have.property('type', expectedType)
    }
  })

  _chai.Assertion.addMethod('haveValidRoutingAlgorithmCreateResponse', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('rule_id').that.is.a('string')
    expect(obj).to.have.property('name').that.is.a('string')
  })

  _chai.Assertion.addMethod('haveValidRoutingAlgorithmList', function () {
    const obj = utils.flag(this, 'object')
    expect(obj).to.be.an('array')
    obj.forEach((item) => {
      expect(item).to.have.property('id').that.is.a('string')
      expect(item).to.have.property('name').that.is.a('string')
      expect(item).to.have.property('created_by').that.is.a('string')
    })
  })

  _chai.Assertion.addMethod('haveValidAnalyticsOverview', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('merchant_id').that.is.a('string')
    expect(obj).to.have.property('kpis').that.is.an('array')
    expect(obj).to.have.property('route_hits').that.is.an('array')
  })

  _chai.Assertion.addMethod('haveValidRoutingStats', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('merchant_id').that.is.a('string')
    expect(obj).to.have.property('gateway_share').that.is.an('array')
    expect(obj).to.have.property('sr_trend').that.is.an('array')
  })

  _chai.Assertion.addMethod('haveValidPaymentAudit', function () {
    const obj = utils.flag(this, 'object')
    assertObject(obj)
    expect(obj).to.have.property('results').that.is.an('array')
    expect(obj).to.have.property('page')
    expect(obj).to.have.property('page_size')
    expect(obj).to.have.property('total_results')
  })
})
