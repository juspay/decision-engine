const { defineConfig } = require('cypress')

module.exports = defineConfig({
  e2e: {
    baseUrl: 'http://localhost:8080',
    supportFile: 'cypress/support/e2e.js',
    specPattern: 'cypress/e2e/**/*.cy.{js,jsx,ts,tsx}',
    viewportWidth: 1280,
    viewportHeight: 720,
    video: false,
    screenshotOnRunFailure: true,
    defaultCommandTimeout: 10000,
    requestTimeout: 10000,
    responseTimeout: 10000,
    env: {
      // Default configuration - can be overridden via environment variables
      API_BASE_URL: 'http://localhost:8080',
      DEFAULT_MERCHANT_ID_PREFIX: 'merc_',
      DEFAULT_PAYMENT_ID_PREFIX: 'PAY_',
      DEFAULT_CUSTOMER_ID_PREFIX: 'CUST',
      // Test data configuration
      DEFAULT_GATEWAYS: ['GatewayA', 'GatewayB', 'GatewayC'],
      DEFAULT_AMOUNT: 100.50,
      DEFAULT_CURRENCY: 'USD',
      // Routing algorithm types
      ROUTING_ALGORITHMS: {
        SUCCESS_RATE: 'SR_BASED_ROUTING',
        PAYMENT_LATENCY: 'PL_BASED_ROUTING',
        COST_BASED: 'COST_BASED_ROUTING'
      },
      // Payment method types for testing
      PAYMENT_METHODS: {
        UPI: {
          type: 'UPI',
          method: 'UPI_PAY'
        },
        UPI_COLLECT: {
          type: 'upi',
          method: 'upi_collect'
        },
        CARD: {
          type: 'CARD',
          method: 'CARD_PAY'
        }
      }
    },
    setupNodeEvents(on, config) {
      // implement node event listeners here
      require('@cypress/grep/src/plugin')(config)
      return config
    },
  },
})
