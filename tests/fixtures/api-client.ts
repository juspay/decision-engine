import type { APIRequestContext } from '@playwright/test'
import factory from './factory'

/**
 * Playwright port of the Cypress `requestApi` command + the endpoint helpers
 * (cypress/support/commands.js). Talks directly to the decision-engine API on :8080,
 * independent of the Playwright project's `baseURL` (so the same client works in both the
 * `api` and `ui` projects). Auth is a mutable bearer token set by the session handshake.
 */

const API_BASE_URL = process.env.API_BASE_URL || 'http://localhost:8080'
const ADMIN_SECRET = process.env.ADMIN_SECRET || 'test_admin'

export interface ApiResponse<T = any> {
  status: number
  body: T
  headers: Record<string, string>
}

export interface RequestOptions {
  body?: unknown
  failOnStatusCode?: boolean
  headers?: Record<string, string>
  qs?: Record<string, string | number | boolean | undefined | null>
}

function resolveApiUrl(path: string): string {
  if (/^https?:\/\//.test(path)) return path
  return `${API_BASE_URL}${path}`
}

function cleanParams(qs?: RequestOptions['qs']): Record<string, string | number | boolean> | undefined {
  if (!qs) return undefined
  const out: Record<string, string | number | boolean> = {}
  for (const [k, v] of Object.entries(qs)) {
    if (v !== undefined && v !== null) out[k] = v
  }
  return out
}

export class ApiClient {
  readonly request: APIRequestContext
  /** Bearer token for the active session; auto-attached to every request once set. */
  token: string | null = null
  /**
   * Whether to send `x-admin-secret`. Since #345 that header is accepted as service-to-service auth on
   * protected routes, so an "anonymous" client must drop it too — otherwise it is still authenticated
   * and every auth-guard assertion silently passes for the wrong reason.
   */
  sendAdminSecret = true

  constructor(request: APIRequestContext) {
    this.request = request
  }

  /**
   * A client sharing this request context but carrying no session token — for auth-guard tests that
   * assert protected routes reject unauthenticated callers.
   *
   * Note it still sends `x-admin-secret`, which is what /auth/admin/* checks; only the bearer token is
   * dropped.
   */
  anonymous(): ApiClient {
    const client = new ApiClient(this.request)
    client.sendAdminSecret = false
    return client
  }

  /** Core request primitive — mirrors Cypress `requestApi`. */
  async raw<T = any>(method: string, path: string, options: RequestOptions = {}): Promise<ApiResponse<T>> {
    const { body, failOnStatusCode = true, headers = {}, qs } = options
    const requestHeaders: Record<string, string> = {
      'Content-Type': 'application/json',
      'x-tenant-id': 'public',
      ...(this.sendAdminSecret ? { 'x-admin-secret': ADMIN_SECRET } : {}),
      // Explicit Authorization always wins over the session token.
      ...(this.token && !headers.Authorization ? { Authorization: `Bearer ${this.token}` } : {}),
      ...headers,
    }

    const response = await this.request.fetch(resolveApiUrl(path), {
      method,
      headers: requestHeaders,
      params: cleanParams(qs),
      data: body === undefined || body === null ? undefined : (body as any),
    })

    const status = response.status()
    const text = await response.text()
    let parsed: any = text
    const contentType = response.headers()['content-type'] || ''
    if (contentType.includes('application/json')) {
      try {
        parsed = JSON.parse(text)
      } catch {
        parsed = text
      }
    }

    if (failOnStatusCode && (status < 200 || status >= 400)) {
      throw new Error(
        `API request failed (${method} ${path}) with status ${status}: ${JSON.stringify(parsed)}`,
      )
    }

    return { status, body: parsed, headers: response.headers() }
  }

  // ---- Merchant ----------------------------------------------------------

  ensureMerchantAccount(merchantId: string) {
    return this.raw('POST', '/merchant-account/create', {
      failOnStatusCode: false,
      body: { merchant_id: merchantId, gateway_success_rate_based_decider_input: null },
    })
  }

  getMerchantAccount(merchantId: string, options: RequestOptions = {}) {
    return this.raw('GET', `/merchant-account/${merchantId}`, options)
  }

  deleteMerchantAccount(merchantId: string, options: RequestOptions = {}) {
    return this.raw('DELETE', `/merchant-account/${merchantId}`, options)
  }

  cleanupTestData(merchantId: string) {
    if (!merchantId) return Promise.resolve(undefined)
    return this.raw('DELETE', `/merchant-account/${merchantId}`, { failOnStatusCode: false })
  }

  // ---- Rule config (SR / elimination) ------------------------------------

  createRuleConfig(merchantId: string, config: unknown, options: RequestOptions = {}) {
    return this.raw('POST', '/rule/create', { ...options, body: { merchant_id: merchantId, config } })
  }

  getRuleConfig(merchantId: string, algorithm: string, options: RequestOptions = {}) {
    return this.raw('POST', '/rule/get', { ...options, body: { merchant_id: merchantId, algorithm } })
  }

  updateRuleConfig(merchantId: string, config: unknown, options: RequestOptions = {}) {
    return this.raw('POST', '/rule/update', { ...options, body: { merchant_id: merchantId, config } })
  }

  deleteRuleConfig(merchantId: string, algorithm: string, options: RequestOptions = {}) {
    return this.raw('POST', '/rule/delete', { ...options, body: { merchant_id: merchantId, algorithm } })
  }

  createSuccessRateConfig(merchantId: string, overrides: Record<string, unknown> = {}, options: RequestOptions = {}) {
    return this.createRuleConfig(merchantId, { type: 'successRate', data: factory.srConfigData(overrides) }, options)
  }

  getSuccessRateConfig(merchantId: string, options: RequestOptions = {}) {
    return this.getRuleConfig(merchantId, 'successRate', options)
  }

  updateSuccessRateConfig(merchantId: string, overrides: Record<string, unknown> = {}, options: RequestOptions = {}) {
    return this.updateRuleConfig(merchantId, { type: 'successRate', data: factory.srConfigData(overrides) }, options)
  }

  deleteSuccessRateConfig(merchantId: string, options: RequestOptions = {}) {
    return this.deleteRuleConfig(merchantId, 'successRate', options)
  }

  createEliminationConfig(merchantId: string, overrides: Record<string, unknown> = {}, options: RequestOptions = {}) {
    return this.createRuleConfig(merchantId, { type: 'elimination', data: factory.eliminationConfigData(overrides) }, options)
  }

  getEliminationConfig(merchantId: string, options: RequestOptions = {}) {
    return this.getRuleConfig(merchantId, 'elimination', options)
  }

  updateEliminationConfig(merchantId: string, overrides: Record<string, unknown> = {}, options: RequestOptions = {}) {
    return this.updateRuleConfig(merchantId, { type: 'elimination', data: factory.eliminationConfigData(overrides) }, options)
  }

  deleteEliminationConfig(merchantId: string, options: RequestOptions = {}) {
    return this.deleteRuleConfig(merchantId, 'elimination', options)
  }

  // ---- Decision + feedback ----------------------------------------------

  decideGateway(decisionRequest: Record<string, any> = {}, options: RequestOptions = {}) {
    const request = {
      ...factory.srDecideGatewayRequest(),
      ...decisionRequest,
      paymentInfo: { ...factory.paymentInfo(), ...(decisionRequest.paymentInfo || {}) },
    }
    return this.raw('POST', '/decide-gateway', { ...options, body: request })
  }

  updateGatewayScore(scoreUpdate: Record<string, any> = {}, options: RequestOptions = {}) {
    const base = factory.updateGatewayScoreRequest()
    const request = {
      ...base,
      ...scoreUpdate,
      txnLatency: { ...base.txnLatency, ...(scoreUpdate.txnLatency || {}) },
    }
    return this.raw('POST', '/update-gateway-score', { ...options, body: request })
  }

  // ---- Routing algorithms (advanced/priority/volume-split) ---------------

  createRoutingAlgorithm(payload: unknown, options: RequestOptions = {}) {
    return this.raw('POST', '/routing/create', { ...options, body: payload })
  }

  listRoutingAlgorithms(createdBy: string, options: RequestOptions = {}) {
    return this.raw('POST', `/routing/list/${createdBy}`, options)
  }

  activateRoutingAlgorithm(createdBy: string, routingAlgorithmId: string, options: RequestOptions = {}) {
    return this.raw('POST', '/routing/activate', {
      ...options,
      body: { created_by: createdBy, routing_algorithm_id: routingAlgorithmId },
    })
  }

  listActiveRoutingAlgorithms(createdBy: string, options: RequestOptions = {}) {
    return this.raw('POST', `/routing/list/active/${createdBy}`, options)
  }

  evaluateRoutingAlgorithm(payload: unknown, options: RequestOptions = {}) {
    return this.raw('POST', '/routing/evaluate', { ...options, body: payload })
  }

  // ---- API keys ----------------------------------------------------------

  createApiKey(merchantId: string, description: string | null = null) {
    return this.raw('POST', '/api-key/create', { body: { merchant_id: merchantId, description } })
  }

  listApiKeys(merchantId: string) {
    return this.raw('GET', `/api-key/list/${merchantId}`)
  }
}
