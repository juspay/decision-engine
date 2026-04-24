const { defineConfig } = require('cypress')

module.exports = defineConfig({
  e2e: {
    baseUrl: process.env.CYPRESS_UI_BASE_URL || 'http://localhost:5173',
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
      API_BASE_URL: process.env.CYPRESS_API_BASE_URL || 'http://localhost:8080',
      UI_BASE_URL: process.env.CYPRESS_UI_BASE_URL || 'http://localhost:5173',
      DOCS_BASE_URL: process.env.CYPRESS_DOCS_BASE_URL || 'http://localhost:3000',
      CLICKHOUSE_HTTP_URL: process.env.CYPRESS_CLICKHOUSE_HTTP_URL || 'http://localhost:8123',
      CLICKHOUSE_DATABASE: process.env.CYPRESS_CLICKHOUSE_DATABASE || 'default',
      CLICKHOUSE_USER: process.env.CYPRESS_CLICKHOUSE_USER || 'decision_engine',
      CLICKHOUSE_PASSWORD: process.env.CYPRESS_CLICKHOUSE_PASSWORD || 'decision_engine',
      RUNTIME_MODE: process.env.CYPRESS_RUNTIME_MODE || 'manual',
      HEALTH_POLL_TIMEOUT_MS: Number(process.env.CYPRESS_HEALTH_POLL_TIMEOUT_MS || 120000),
      HEALTH_POLL_INTERVAL_MS: Number(process.env.CYPRESS_HEALTH_POLL_INTERVAL_MS || 2000),
      DOCS_POLL_TIMEOUT_MS: Number(process.env.CYPRESS_DOCS_POLL_TIMEOUT_MS || 120000),
      DOCS_POLL_INTERVAL_MS: Number(process.env.CYPRESS_DOCS_POLL_INTERVAL_MS || 2000),
      EXPECTED_CLICKHOUSE_TABLES: [
        'analytics_api_events_queue',
        'analytics_domain_events_queue',
        'analytics_api_events',
        'analytics_domain_events',
        'analytics_payment_audit_summary_buckets',
        'analytics_payment_audit_lookup_summaries',
      ],
      DEFAULT_MERCHANT_ID_PREFIX: 'merc_',
      DEFAULT_PAYMENT_ID_PREFIX: 'PAY_',
      DEFAULT_CUSTOMER_ID_PREFIX: 'CUST',
      // Test data configuration
      DEFAULT_GATEWAYS: ['stripe', 'adyen', 'checkout'],
      DEFAULT_AMOUNT: 100.50,
      DEFAULT_CURRENCY: 'USD',
      ANALYTICS_POLL_INTERVAL_MS: 2000,
      ANALYTICS_POLL_TIMEOUT_MS: 60000,
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
      on('task', {
        runtimeContext() {
          return {
            runtimeMode: config.env.RUNTIME_MODE || 'manual',
            apiBaseUrl: config.env.API_BASE_URL,
            uiBaseUrl: config.env.UI_BASE_URL,
            docsBaseUrl: config.env.DOCS_BASE_URL,
          }
        },
        async httpRequest({ method, url, qs, headers, body, timeout }) {
          const requestUrl = new URL(url)
          if (qs) {
            Object.entries(qs).forEach(([key, value]) => {
              if (value !== undefined && value !== null) {
                requestUrl.searchParams.set(key, value)
              }
            })
          }

          const controller = new AbortController()
          const timeoutMs = Number(timeout || config.responseTimeout || 10000)
          const timer = setTimeout(() => controller.abort(), timeoutMs)

          try {
            const response = await fetch(requestUrl, {
              method,
              headers,
              body: body == null ? undefined : JSON.stringify(body),
              signal: controller.signal,
            })

            const text = await response.text()
            let parsedBody = text
            const contentType = response.headers.get('content-type') || ''
            if (contentType.includes('application/json')) {
              try {
                parsedBody = JSON.parse(text)
              } catch (_error) {
                parsedBody = text
              }
            }

            return {
              status: response.status,
              body: parsedBody,
              headers: Object.fromEntries(response.headers.entries()),
            }
          } finally {
            clearTimeout(timer)
          }
        },
        async clickhouseQuery({ query, baseUrl, database, username, password }) {
          const url = new URL(baseUrl || config.env.CLICKHOUSE_HTTP_URL || 'http://localhost:8123')
          const selectedDatabase = database || config.env.CLICKHOUSE_DATABASE || 'default'
          url.searchParams.set('database', selectedDatabase)
          url.searchParams.set('query', query)

          const authUser = username || config.env.CLICKHOUSE_USER || 'decision_engine'
          const authPass = password || config.env.CLICKHOUSE_PASSWORD || 'decision_engine'
          const headers = {}
          if (authUser || authPass) {
            headers.Authorization = `Basic ${Buffer.from(`${authUser}:${authPass}`).toString('base64')}`
          }

          const response = await fetch(url, { headers })
          const body = await response.text()
          if (!response.ok) {
            throw new Error(`ClickHouse query failed (${response.status}): ${body}`)
          }

          return body
        },
      })

      require('@cypress/grep/src/plugin')(config)
      return config
    },
  },
})
