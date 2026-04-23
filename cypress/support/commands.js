const factory = require('./test-data-factory')

function getApiBaseUrl() {
  return Cypress.env('API_BASE_URL') || 'http://localhost:8080'
}

function getUiBaseUrl() {
  return Cypress.env('UI_BASE_URL') || 'http://localhost:5173'
}

function getAdminSecret() {
  return Cypress.env('ADMIN_SECRET') || 'test_admin'
}

function resolveApiUrl(path) {
  if (/^https?:\/\//.test(path)) return path
  return `${getApiBaseUrl()}${path}`
}

function requestApi(method, path, options = {}) {
  const { body, failOnStatusCode = true, headers = {}, qs } = options

  return cy.request({
    method,
    url: resolveApiUrl(path),
    failOnStatusCode,
    qs,
    headers: {
      'Content-Type': 'application/json',
      'x-tenant-id': 'public',
      'x-admin-secret': getAdminSecret(),
      ...headers,
    },
    body,
  })
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

Cypress.Commands.add('waitForService', () => {
  return requestApi('GET', '/health').its('status').should('eq', 200)
})

Cypress.Commands.add('cleanupTestData', (merchantId) => {
  if (!merchantId) return cy.wrap(null)
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

  function poll() {
    return requestFactory().then((result) => {
      if (predicate(result)) {
        return cy.wrap(result)
      }

      if (Date.now() - startedAt >= timeout) {
        throw new Error(options.errorMessage || 'Timed out waiting for condition')
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

  return requestApi('POST', '/auth/signup', {
    failOnStatusCode: false,
    body: {
      email,
      password,
      merchant_id: merchantId,
    },
  }).then((response) => {
    if (response.status === 200) {
      return cy.wrap({
        token: response.body.token,
        user: {
          userId: response.body.user_id,
          email: response.body.email,
          merchantId: response.body.merchant_id,
          role: response.body.role,
        },
      })
    }

    return requestApi('POST', '/auth/login', {
      body: { email, password },
    }).then((loginResponse) =>
      cy.wrap({
        token: loginResponse.body.token,
        user: {
          userId: loginResponse.body.user_id,
          email: loginResponse.body.email,
          merchantId: loginResponse.body.merchant_id,
          role: loginResponse.body.role,
        },
      }),
    )
  })
})

Cypress.Commands.add('visitWithMerchant', (path = '/', merchantId, options = {}) => {
  const id = merchantId || factory.merchantId('ui')
  const targetUrl = /^https?:\/\//.test(path) ? path : `${getUiBaseUrl()}${path}`

  return cy.ensureMerchantAccount(id).then(() =>
    cy.ensureDashboardSession(id).then((session) =>
      cy.visit(targetUrl, {
        ...options,
        onBeforeLoad(win) {
          win.localStorage.setItem('merchant-store', merchantStoreState(id))
          win.localStorage.setItem('auth-store', authStoreState(session))
          if (typeof options.onBeforeLoad === 'function') {
            options.onBeforeLoad(win)
          }
        },
      }),
    ),
  )
})

Cypress.Commands.add('setMerchantFromTopBar', (merchantId) => {
  return cy.get('input[placeholder="Set Merchant ID"]').clear().type(merchantId).type('{enter}')
})
