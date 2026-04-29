const factory = require('./test-data-factory')
const dashboardSessionCache = new Map()

function getApiBaseUrl() {
  return Cypress.env('API_BASE_URL') || 'http://localhost:8080'
}

function getUiBaseUrl() {
  return Cypress.env('UI_BASE_URL') || 'http://localhost:5173'
}

function getDocsBaseUrl() {
  if (Cypress.env('DOCS_BASE_URL')) {
    return Cypress.env('DOCS_BASE_URL')
  }

  return getRuntimeMode() === 'docker' ? 'http://localhost:8081' : 'http://localhost:3000'
}

function getRuntimeMode() {
  return Cypress.env('RUNTIME_MODE') || 'manual'
}

function getClickHouseConfig() {
  return {
    baseUrl: Cypress.env('CLICKHOUSE_HTTP_URL') || 'http://localhost:8123',
    database: Cypress.env('CLICKHOUSE_DATABASE') || 'default',
    username: Cypress.env('CLICKHOUSE_USER') || 'decision_engine',
    password: Cypress.env('CLICKHOUSE_PASSWORD') || 'decision_engine',
  }
}

function getAdminSecret() {
  return Cypress.env('ADMIN_SECRET') || 'test_admin'
}

function resolveApiUrl(path) {
  if (/^https?:\/\//.test(path)) return path
  return `${getApiBaseUrl()}${path}`
}

function resolveUiUrl(path) {
  if (/^https?:\/\//.test(path)) return path
  return `${getUiBaseUrl()}${path}`
}

function resolveDocsUrl(path = '/introduction') {
  if (/^https?:\/\//.test(path)) return path
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  return `${getDocsBaseUrl()}${normalizedPath}`
}

function requestApi(method, path, options = {}) {
  const {
    body,
    failOnStatusCode = true,
    headers = {},
    qs,
    timeout,
    retries,
    retryInterval,
  } = options

  const shouldRetryTransientAnalyticsError = (response) => {
    if (!path.startsWith('/analytics/')) return false
    if (!['source', 'docker'].includes(getRuntimeMode())) return false
    if (response.status !== 500) return false
    return response.body?.code === 'TE_01'
  }

  const maxRetries = retries ?? 0
  const intervalMs = retryInterval ?? 2000
  const useNodeHttpTask =
    path.startsWith('/analytics/') && ['source', 'docker'].includes(getRuntimeMode())

  function send(attempt = 0) {
    const requestHeaders = {
      'Content-Type': 'application/json',
      'x-tenant-id': 'public',
      'x-admin-secret': getAdminSecret(),
      ...headers,
    }

    const requestChain = useNodeHttpTask
      ? cy.task('httpRequest', {
          method,
          url: resolveApiUrl(path),
          qs,
          timeout,
          headers: requestHeaders,
          body,
        })
      : cy.request({
          method,
          url: resolveApiUrl(path),
          failOnStatusCode: false,
          qs,
          timeout,
          headers: requestHeaders,
          body,
        })

    return requestChain
      .then((response) => {
        if (attempt < maxRetries && shouldRetryTransientAnalyticsError(response)) {
          return cy.wait(intervalMs).then(() => send(attempt + 1))
        }

        if (
          failOnStatusCode &&
          shouldRetryTransientAnalyticsError(response)
        ) {
          return response
        }

        if (failOnStatusCode && (response.status < 200 || response.status >= 400)) {
          throw new Error(
            `API request failed (${method} ${path}) with status ${response.status}: ${JSON.stringify(response.body)}`,
          )
        }

        return response
      })
  }

  return send()
}

function normalizeAnalyticsRequest(query = {}, options = {}) {
  const normalizedQuery = { ...query }
  const merchantId = options.merchantId || normalizedQuery.merchant_id

  delete normalizedQuery.merchant_id
  delete normalizedQuery.scope

  if (!merchantId) {
    throw new Error('Analytics requests require a merchantId to derive the auth context')
  }

  return { merchantId, normalizedQuery }
}

function merchantStoreState(merchantId) {
  return JSON.stringify({
    state: { merchantId },
    version: 0,
  })
}

function authStoreState(session) {
  return JSON.stringify({
    state: {
      token: session.token,
      user: session.user,
    },
    version: 0,
  })
}

function seedDashboardStorage(win, merchantId, session) {
  win.localStorage.setItem('merchant-store', merchantStoreState(merchantId))
  win.localStorage.setItem('auth-store', authStoreState(session))
}

let serviceVerified = false
Cypress.Commands.add('waitForService', () => {
  if (serviceVerified) return cy.wrap(null)
  return requestApi('GET', '/health', {
    timeout: Cypress.env('HEALTH_POLL_TIMEOUT_MS') || 120000,
  }).its('status').should('eq', 200).then(() => { serviceVerified = true })
})

Cypress.Commands.add('waitForDocs', () => {
  return cy
    .request({
      method: 'GET',
      url: resolveDocsUrl('/introduction'),
      timeout: Cypress.env('DOCS_POLL_TIMEOUT_MS') || 120000,
    })
    .its('status')
    .should('eq', 200)
})

Cypress.Commands.add('waitForAnalyticsInfra', () => {
  const expectedTables = Cypress.env('EXPECTED_CLICKHOUSE_TABLES') || []
  const quotedTables = expectedTables.map((table) => `'${table}'`).join(', ')
  const query = `SELECT name FROM system.tables WHERE database = currentDatabase() AND name IN (${quotedTables}) ORDER BY name FORMAT TSV`

  return cy.task('clickhouseQuery', {
    ...getClickHouseConfig(),
    query,
  }).then((result) => {
    const foundTables = new Set(
      `${result}`
        .split('\n')
        .map((value) => value.trim())
        .filter(Boolean),
    )

    expectedTables.forEach((table) => {
      expect(foundTables.has(table), `ClickHouse table ${table} should exist`).to.eq(true)
    })
  })
})

Cypress.Commands.add('waitForRuntimeSurface', () => {
  return cy.waitForService().then(() => cy.waitForDocs()).then(() => cy.waitForAnalyticsInfra())
})

Cypress.Commands.add('fetchDocsPage', (path = '/introduction', options = {}) => {
  return cy.request({
    method: 'GET',
    url: resolveDocsUrl(path),
    ...options,
  })
})

Cypress.Commands.add('runtimeContext', () => {
  return cy.task('runtimeContext')
})

Cypress.Commands.add('cleanupTestData', (merchantId) => {
  if (!merchantId) return cy.wrap(null)
  dashboardSessionCache.delete(merchantId)
  return requestApi('DELETE', `/merchant-account/${merchantId}`, { failOnStatusCode: false })
})

Cypress.Commands.add('ensureMerchantAccount', (merchantId) => {
  return requestApi('POST', '/merchant-account/create', {
    failOnStatusCode: false,
    body: {
      merchant_id: merchantId,
      gateway_success_rate_based_decider_input: null,
    },
  }).then((response) => {
    if (response.status === 200) {
      return cy.wrap({ merchantId, response: response.body })
    }

    return requestApi('GET', `/merchant-account/${merchantId}`).then((getResponse) =>
      cy.wrap({ merchantId, response: getResponse.body }),
    )
  })
})

Cypress.Commands.add('createMerchantAccount', (merchantId, options = {}) => {
  const id = merchantId || factory.merchantId('merchant')
  return requestApi('POST', '/merchant-account/create', {
    ...options,
    body: {
      merchant_id: id,
      gateway_success_rate_based_decider_input: null,
      ...(options.body || {}),
    },
  }).then((response) => cy.wrap({ merchantId: id, response: response.body, status: response.status }))
})

Cypress.Commands.add('getMerchantAccount', (merchantId, options = {}) => {
  return requestApi('GET', `/merchant-account/${merchantId}`, options).then((response) =>
    cy.wrap({ merchantId, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('getDebitRoutingFlag', (merchantId, options = {}) => {
  return requestApi('GET', `/merchant-account/${merchantId}/debit-routing`, options).then((response) =>
    cy.wrap({ merchantId, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('updateDebitRoutingFlag', (merchantId, enabled, options = {}) => {
  return requestApi('POST', `/merchant-account/${merchantId}/debit-routing`, {
    ...options,
    body: { enabled },
  }).then((response) => cy.wrap({ merchantId, response: response.body, status: response.status }))
})

Cypress.Commands.add('deleteMerchantAccount', (merchantId, options = {}) => {
  return requestApi('DELETE', `/merchant-account/${merchantId}`, options).then((response) =>
    cy.wrap({ merchantId, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('createRuleConfig', (merchantId, config, options = {}) => {
  return requestApi('POST', '/rule/create', {
    ...options,
    body: {
      merchant_id: merchantId,
      config,
    },
  }).then((response) => cy.wrap({ merchantId, config, response: response.body, status: response.status }))
})

Cypress.Commands.add('getRuleConfig', (merchantId, algorithm, options = {}) => {
  return requestApi('POST', '/rule/get', {
    ...options,
    body: {
      merchant_id: merchantId,
      algorithm,
    },
  }).then((response) =>
    cy.wrap({ merchantId, algorithm, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('updateRuleConfig', (merchantId, config, options = {}) => {
  return requestApi('POST', '/rule/update', {
    ...options,
    body: {
      merchant_id: merchantId,
      config,
    },
  }).then((response) => cy.wrap({ merchantId, config, response: response.body, status: response.status }))
})

Cypress.Commands.add('deleteRuleConfig', (merchantId, algorithm, options = {}) => {
  return requestApi('POST', '/rule/delete', {
    ...options,
    body: {
      merchant_id: merchantId,
      algorithm,
    },
  }).then((response) =>
    cy.wrap({ merchantId, algorithm, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('createSuccessRateConfig', (merchantId, overrides = {}, options = {}) => {
  return cy.createRuleConfig(
    merchantId,
    {
      type: 'successRate',
      data: factory.srConfigData(overrides),
    },
    options,
  )
})

Cypress.Commands.add('getSuccessRateConfig', (merchantId, options = {}) => {
  return cy.getRuleConfig(merchantId, 'successRate', options)
})

Cypress.Commands.add('updateSuccessRateConfig', (merchantId, overrides = {}, options = {}) => {
  return cy.updateRuleConfig(
    merchantId,
    {
      type: 'successRate',
      data: factory.srConfigData(overrides),
    },
    options,
  )
})

Cypress.Commands.add('deleteSuccessRateConfig', (merchantId, options = {}) => {
  return cy.deleteRuleConfig(merchantId, 'successRate', options)
})

Cypress.Commands.add('createEliminationConfig', (merchantId, overrides = {}, options = {}) => {
  return cy.createRuleConfig(
    merchantId,
    {
      type: 'elimination',
      data: factory.eliminationConfigData(overrides),
    },
    options,
  )
})

Cypress.Commands.add('getEliminationConfig', (merchantId, options = {}) => {
  return cy.getRuleConfig(merchantId, 'elimination', options)
})

Cypress.Commands.add('updateEliminationConfig', (merchantId, overrides = {}, options = {}) => {
  return cy.updateRuleConfig(
    merchantId,
    {
      type: 'elimination',
      data: factory.eliminationConfigData(overrides),
    },
    options,
  )
})

Cypress.Commands.add('deleteEliminationConfig', (merchantId, options = {}) => {
  return cy.deleteRuleConfig(merchantId, 'elimination', options)
})

Cypress.Commands.add('createDebitRoutingConfig', (merchantId, overrides = {}, options = {}) => {
  return cy.createRuleConfig(
    merchantId,
    {
      type: 'debitRouting',
      data: factory.debitRoutingConfigData(overrides),
    },
    options,
  )
})

Cypress.Commands.add('decideGateway', (decisionRequest, options = {}) => {
  const request = {
    ...factory.srDecideGatewayRequest(),
    ...decisionRequest,
    paymentInfo: {
      ...factory.paymentInfo(),
      ...(decisionRequest.paymentInfo || {}),
    },
  }

  return requestApi('POST', '/decide-gateway', {
    ...options,
    body: request,
  }).then((response) => cy.wrap({ request, response: response.body, status: response.status }))
})

Cypress.Commands.add('decideGatewayLegacy', (decisionRequest, options = {}) => {
  return cy.decideGateway(decisionRequest, options)
})

Cypress.Commands.add('updateGatewayScore', (scoreUpdate, options = {}) => {
  const request = {
    ...factory.updateGatewayScoreRequest(),
    ...scoreUpdate,
    txnLatency: {
      ...factory.updateGatewayScoreRequest().txnLatency,
      ...(scoreUpdate.txnLatency || {}),
    },
  }

  return requestApi('POST', '/update-gateway-score', {
    ...options,
    body: request,
  }).then((response) => cy.wrap({ request, response: response.body, status: response.status }))
})

Cypress.Commands.add('createRoutingAlgorithm', (payload, options = {}) => {
  return requestApi('POST', '/routing/create', {
    ...options,
    body: payload,
  }).then((response) => cy.wrap({ request: payload, response: response.body, status: response.status }))
})

Cypress.Commands.add('listRoutingAlgorithms', (createdBy, options = {}) => {
  return requestApi('POST', `/routing/list/${createdBy}`, options).then((response) =>
    cy.wrap({ createdBy, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('activateRoutingAlgorithm', (createdBy, routingAlgorithmId, options = {}) => {
  return requestApi('POST', '/routing/activate', {
    ...options,
    body: {
      created_by: createdBy,
      routing_algorithm_id: routingAlgorithmId,
    },
  }).then((response) =>
    cy.wrap({ createdBy, routingAlgorithmId, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('listActiveRoutingAlgorithms', (createdBy, options = {}) => {
  return requestApi('POST', `/routing/list/active/${createdBy}`, options).then((response) =>
    cy.wrap({ createdBy, response: response.body, status: response.status }),
  )
})

Cypress.Commands.add('evaluateRoutingAlgorithm', (payload, options = {}) => {
  return requestApi('POST', '/routing/evaluate', {
    ...options,
    body: payload,
  }).then((response) => cy.wrap({ request: payload, response: response.body, status: response.status }))
})

Cypress.Commands.add('fetchAnalyticsOverview', (query = {}, options = {}) => {
  const { merchantId, normalizedQuery } = normalizeAnalyticsRequest(query, options)

  return cy.ensureDashboardSession(merchantId).then((session) =>
    requestApi('GET', '/analytics/overview', {
      ...options,
      qs: normalizedQuery,
      headers: {
        ...(options.headers || {}),
        Authorization: `Bearer ${session.token}`,
      },
    }).then((response) =>
      cy.wrap({ merchantId, query: normalizedQuery, response: response.body, status: response.status }),
    ),
  )
})

Cypress.Commands.add('fetchAnalyticsRoutingStats', (query = {}, options = {}) => {
  const { merchantId, normalizedQuery } = normalizeAnalyticsRequest(query, options)

  return cy.ensureDashboardSession(merchantId).then((session) =>
    requestApi('GET', '/analytics/routing-stats', {
      ...options,
      qs: normalizedQuery,
      headers: {
        ...(options.headers || {}),
        Authorization: `Bearer ${session.token}`,
      },
    }).then((response) =>
      cy.wrap({ merchantId, query: normalizedQuery, response: response.body, status: response.status }),
    ),
  )
})

Cypress.Commands.add('fetchPaymentAudit', (query = {}, options = {}) => {
  const { merchantId, normalizedQuery } = normalizeAnalyticsRequest(query, options)

  return cy.ensureDashboardSession(merchantId).then((session) =>
    requestApi('GET', '/analytics/payment-audit', {
      ...options,
      qs: normalizedQuery,
      headers: {
        ...(options.headers || {}),
        Authorization: `Bearer ${session.token}`,
      },
    }).then((response) =>
      cy.wrap({ merchantId, query: normalizedQuery, response: response.body, status: response.status }),
    ),
  )
})

Cypress.Commands.add('fetchPreviewTrace', (query = {}, options = {}) => {
  const { merchantId, normalizedQuery } = normalizeAnalyticsRequest(query, options)

  return cy.ensureDashboardSession(merchantId).then((session) =>
    requestApi('GET', '/analytics/preview-trace', {
      ...options,
      qs: normalizedQuery,
      headers: {
        ...(options.headers || {}),
        Authorization: `Bearer ${session.token}`,
      },
    }).then((response) =>
      cy.wrap({ merchantId, query: normalizedQuery, response: response.body, status: response.status }),
    ),
  )
})

Cypress.Commands.add('pollRequest', (requestFactory, predicate, options = {}) => {
  const timeout = options.timeout ?? Cypress.env('ANALYTICS_POLL_TIMEOUT_MS') ?? 30000
  const interval = options.interval ?? Cypress.env('ANALYTICS_POLL_INTERVAL_MS') ?? 2000
  const startedAt = Date.now()
  let lastResult = null

  function poll() {
    return requestFactory().then((result) => {
      lastResult = result

      if (predicate(result)) {
        return cy.wrap(result)
      }

      if (Date.now() - startedAt >= timeout) {
        const context = lastResult
          ? ` Last result: ${JSON.stringify({
              status: lastResult.status,
              response: lastResult.response,
            }).slice(0, 1000)}`
          : ''
        throw new Error(`${options.errorMessage || 'Timed out waiting for condition'}.${context}`)
      }

      return cy.wait(interval).then(poll)
    })
  }

  return poll()
})

Cypress.Commands.add('setMerchantContext', (merchantId) => {
  return cy.window().then((win) => {
    win.localStorage.setItem('merchant-store', merchantStoreState(merchantId))
  })
})

Cypress.Commands.add('ensureDashboardSession', (merchantId) => {
  const email = `${merchantId}@example.com`
  const password = 'Password123!'
  const cachedSession = dashboardSessionCache.get(merchantId)

  if (cachedSession) {
    return cy.wrap(cachedSession)
  }

  return requestApi('POST', '/auth/signup', {
    failOnStatusCode: false,
    body: {
      email,
      password,
      merchant_id: merchantId,
    },
  }).then((response) => {
    if (response.status === 200) {
      const session = {
        token: response.body.token,
        user: {
          userId: response.body.user_id,
          email: response.body.email,
          merchantId: response.body.merchant_id,
          role: response.body.role,
        },
      }
      dashboardSessionCache.set(merchantId, session)
      return cy.wrap(session)
    }

    return requestApi('POST', '/auth/login', {
      body: { email, password },
    }).then((loginResponse) => {
      const session = {
        token: loginResponse.body.token,
        user: {
          userId: loginResponse.body.user_id,
          email: loginResponse.body.email,
          merchantId: loginResponse.body.merchant_id,
          role: loginResponse.body.role,
        },
      }
      dashboardSessionCache.set(merchantId, session)
      return cy.wrap(session)
    })
  })
})

Cypress.Commands.add('visitWithMerchant', (path = '/', merchantId, options = {}) => {
  const id = merchantId || factory.merchantId('ui')
  const targetUrl = resolveUiUrl(path)

  return cy.ensureMerchantAccount(id).then(() =>
    cy.ensureDashboardSession(id).then((session) =>
      cy.visit(targetUrl, {
        ...options,
        onBeforeLoad(win) {
          seedDashboardStorage(win, id, session)
          if (typeof options.onBeforeLoad === 'function') {
            options.onBeforeLoad(win)
          }
        },
      }),
    ),
  )
})

Cypress.Commands.add('visitWithSession', (path = '/', merchantId, options = {}) => {
  const id = merchantId || factory.merchantId('ui')
  const targetUrl = resolveUiUrl(path)

  return cy.ensureDashboardSession(id).then((session) =>
    cy.visit(targetUrl, {
      ...options,
      onBeforeLoad(win) {
        seedDashboardStorage(win, id, session)
        if (typeof options.onBeforeLoad === 'function') {
          options.onBeforeLoad(win)
        }
      },
    }),
  )
})

Cypress.Commands.add('visitAppPath', (path = '/', options = {}) => {
  return cy.visit(resolveUiUrl(path), options)
})

Cypress.Commands.add('setMerchantFromTopBar', (merchantId) => {
  return cy.get('input[placeholder="Set Merchant ID"]').clear().type(merchantId).type('{enter}')
})
