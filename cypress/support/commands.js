// ***********************************************
// This example commands.js shows you how to
// create various custom commands and overwrite
// existing commands.
//
// For more comprehensive examples of custom
// commands please read more here:
// https://on.cypress.io/custom-commands
// ***********************************************

const { v4: uuidv4 } = require('uuid')

// Helper function to generate unique IDs
function generateUniqueId(prefix = '') {
  const timestamp = Date.now()
  const random = Math.floor(Math.random() * 1000)
  return `${prefix}${timestamp}${random}`
}

// Helper function to get API base URL
function getApiBaseUrl() {
  return Cypress.env('API_BASE_URL') || 'http://localhost:8082'
}

/**
 * Create a merchant account
 * @param {string} merchantId - Optional merchant ID, will generate if not provided
 */
Cypress.Commands.add('createMerchantAccount', (merchantId = null) => {
  const id = merchantId || generateUniqueId(Cypress.env('DEFAULT_MERCHANT_ID_PREFIX'))
  
  return cy.request({
    method: 'POST',
    url: `${getApiBaseUrl()}/merchant-account/create`,
    headers: {
      'Content-Type': 'application/json'
    },
    body: {
      merchant_id: id
    }
  }).then((response) => {
    expect(response.status).to.eq(200)
    return cy.wrap({ merchantId: id, response: response.body })
  })
})

/**
 * Create a routing rule
 * @param {string} merchantId - Merchant ID
 * @param {object} ruleConfig - Rule configuration object
 */
Cypress.Commands.add('createRoutingRule', (merchantId, ruleConfig) => {
  const defaultConfig = {
    type: "successRate",
    data: {
      defaultLatencyThreshold: 90,
      defaultSuccessRate: 0.5,
      defaultBucketSize: 200,
      defaultHedgingPercent: 5,
      txnLatency: {
        gatewayLatency: 5000
      },
      subLevelInputConfig: [
        {
          paymentMethodType: "upi",
          paymentMethod: "upi_collect",
          bucketSize: 250,
          hedgingPercent: 1
        }
      ]
    }
  }

  const config = { ...defaultConfig, ...ruleConfig }

  return cy.request({
    method: 'POST',
    url: `${getApiBaseUrl()}/rule/create`,
    headers: {
      'Content-Type': 'application/json'
    },
    body: {
      merchant_id: merchantId,
      config: config
    }
  }).then((response) => {
    expect(response.status).to.eq(200)
    return cy.wrap({ ruleConfig: config, response: response.body })
  })
})

/**
 * Decide gateway for a payment
 * @param {object} decisionRequest - Gateway decision request object
 */
Cypress.Commands.add('decideGateway', (decisionRequest) => {
  const request = {
    merchantId: decisionRequest.merchantId || generateUniqueId(Cypress.env('DEFAULT_MERCHANT_ID_PREFIX')),
    eligibleGatewayList: decisionRequest.eligibleGatewayList || Cypress.env('DEFAULT_GATEWAYS'),
    rankingAlgorithm: decisionRequest.rankingAlgorithm || Cypress.env('ROUTING_ALGORITHMS').SUCCESS_RATE,
    eliminationEnabled: decisionRequest.eliminationEnabled || true,
    paymentInfo: {
      paymentId: decisionRequest.paymentInfo.paymentId || generateUniqueId(Cypress.env('DEFAULT_PAYMENT_ID_PREFIX')),
      amount: decisionRequest.paymentInfo.amount || 100.50,
      currency: decisionRequest.paymentInfo.currency || 'USD',
      customerId: decisionRequest.paymentInfo.customerId || generateUniqueId(Cypress.env('DEFAULT_CUSTOMER_ID_PREFIX') || 'CUST'),
      udfs: decisionRequest.paymentInfo.udfs || null,
      preferredGateway: decisionRequest.paymentInfo.preferredGateway || null,
      paymentType: decisionRequest.paymentInfo.paymentType || "ORDER_PAYMENT",
      metadata: decisionRequest.paymentInfo.metadata || null,
      internalMetadata: decisionRequest.paymentInfo.internalMetadata || null,
      isEmi: decisionRequest.paymentInfo.isEmi || false,
      emiBank: decisionRequest.paymentInfo.emiBank || null,
      emiTenure: decisionRequest.paymentInfo.emiTenure || null,
      paymentMethodType: decisionRequest.paymentInfo.paymentMethodType || Cypress.env('PAYMENT_METHODS').UPI.type,
      paymentMethod: decisionRequest.paymentInfo.paymentMethod || Cypress.env('PAYMENT_METHODS').UPI.method,
      paymentSource: decisionRequest.paymentInfo.paymentSource || null,
      authType: decisionRequest.paymentInfo.authType || null,
      cardIssuerBankName: decisionRequest.paymentInfo.cardIssuerBankName || null,
      cardIsin: decisionRequest.paymentInfo.cardIsin || null,
      cardType: decisionRequest.paymentInfo.cardType || null,
      cardSwitchProvider: decisionRequest.paymentInfo.cardSwitchProvider || null
    }
  }

  return cy.request({
    method: 'POST',
    url: `${getApiBaseUrl()}/decide-gateway`,
    headers: {
      'Content-Type': 'application/json'
    },
    body: request
  }).then((response) => {
    expect(response.status).to.eq(200)
    return cy.wrap({ request, response: response.body })
  })
})

/**
 * Update gateway score
 * @param {object} scoreUpdate - Score update object
 */
Cypress.Commands.add('updateGatewayScore', (scoreUpdate) => {
  const defaultUpdate = {
    merchantId: scoreUpdate.merchantId || generateUniqueId(Cypress.env('DEFAULT_MERCHANT_ID_PREFIX')),
    gateway: scoreUpdate.gateway || "GatewayC",
    gatewayReferenceId: scoreUpdate.gatewayReferenceId || null,
    status: scoreUpdate.status || "AUTHORIZED",
    paymentId: scoreUpdate.paymentId || generateUniqueId(Cypress.env('DEFAULT_PAYMENT_ID_PREFIX')),
    enforceDynamicRoutingFailure: null,
    txnLatency: scoreUpdate.txnLatency.gatewayLatency || {
      gatewayLatency: 6000
    }
  }

  const update = { ...defaultUpdate, ...scoreUpdate }

  return cy.request({
    method: 'POST',
    url: `${getApiBaseUrl()}/update-gateway-score`,
    headers: {
      'Content-Type': 'application/json'
    },
    body: update
  }).then((response) => {
    expect(response.status).to.eq(200)
    return cy.wrap({ update, response: response.body })
  })
})

/**
 * Create a success rate routing rule
 * @param {string} merchantId - Merchant ID
 * @param {object} options - Configuration options
 */
Cypress.Commands.add('createSuccessRateRule', (merchantId, options = {}) => {
  const ruleConfig = {
    type: "successRate",
    data: {
      defaultLatencyThreshold: options.latencyThreshold || 90,
      defaultSuccessRate: options.successRate || 0.5,
      defaultBucketSize: options.bucketSize || 200,
      defaultHedgingPercent: options.hedgingPercent || 5,
      txnLatency: {
        gatewayLatency: options.gatewayLatency || 5000
      },
      subLevelInputConfig: options.subLevelConfig || [
        {
          paymentMethodType: "upi",
          paymentMethod: "upi_collect",
          bucketSize: 250,
          hedgingPercent: 1
        }
      ]
    }
  }

  return cy.createRoutingRule(merchantId, ruleConfig)
})

/**
 * Wait for service to be ready
 */
Cypress.Commands.add('waitForService', () => {
  return cy.request({
    method: 'GET',
    url: `${getApiBaseUrl()}/health`,
    failOnStatusCode: false,
    timeout: 30000
  }).then((response) => {
    if (response.status !== 200) {
      cy.wait(2000)
      cy.waitForService()
    }
  })
})

/**
 * Clean up test data (if cleanup endpoints exist)
 * @param {string} merchantId - Merchant ID to clean up
 */
Cypress.Commands.add('cleanupTestData', (merchantId) => {
  // This would depend on cleanup endpoints being available
  // For now, just log the cleanup attempt
  cy.log(`Cleaning up test data for merchant: ${merchantId}`)
})
