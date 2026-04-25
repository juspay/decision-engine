// All API calls use relative URLs so nginx/vite-proxy can handle routing
import { tokenRef } from './tokenRef'

const DEBUG_API = true
const DEFAULT_TENANT_ID = import.meta.env.VITE_DEFAULT_TENANT_ID ?? 'public'
const API_BASE_PATH = (import.meta.env.VITE_API_BASE_PATH ?? '/decision-engine-api').replace(/\/$/, '')
const FEATURE_HEADER = import.meta.env.VITE_FEATURE_HEADER ?? 'decision-engine'

function resolveApiPath(path: string) {
  if (/^https?:\/\//.test(path)) return path
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  if (normalizedPath.startsWith(`${API_BASE_PATH}/`) || normalizedPath === API_BASE_PATH) {
    return normalizedPath
  }
  return `${API_BASE_PATH}${normalizedPath}`
}

function logRequest(method: string, path: string, body?: unknown) {
  if (!DEBUG_API) return
  console.log('\n' + '='.repeat(80))
  console.log(`[API REQUEST] ${new Date().toISOString()}`)
  console.log(`Method: ${method}`)
  console.log(`Path: ${path}`)
  if (body !== undefined) {
    console.log('Body:', JSON.stringify(body, null, 2))
  }
  console.log('='.repeat(80))
}

function logResponse(path: string, status: number, statusText: string, body: string) {
  if (!DEBUG_API) return
  console.log('\n' + '-'.repeat(80))
  console.log(`[API RESPONSE] ${new Date().toISOString()}`)
  console.log(`Path: ${path}`)
  console.log(`Status: ${status} ${statusText}`)
  console.log('Response Body:', body)
  console.log('-'.repeat(80) + '\n')
}

function logError(path: string, error: unknown) {
  if (!DEBUG_API) return
  console.log('\n' + '!'.repeat(80))
  console.log(`[API ERROR] ${new Date().toISOString()}`)
  console.log(`Path: ${path}`)
  if (error instanceof Error) {
    console.log('Error:', error.message)
    console.log('Stack:', error.stack)
  } else {
    console.log('Error:', error)
  }
  console.log('!'.repeat(80) + '\n')
}

function valueAsString(value: unknown): string | null {
  if (typeof value === 'string' && value.trim()) return value.trim()
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  return null
}

function extractErrorMessageFromJson(value: unknown): string | null {
  if (!value || typeof value !== 'object') return valueAsString(value)

  const record = value as Record<string, unknown>
  const directKeys = [
    'message',
    'error_message',
    'user_message',
    'developer_message',
    'error',
    'detail',
    'details',
  ]

  for (const key of directKeys) {
    const message = valueAsString(record[key])
    if (message) return message
  }

  for (const key of ['data', 'error_info', 'context']) {
    const nested = extractErrorMessageFromJson(record[key])
    if (nested) return nested
  }

  return null
}

function buildApiErrorMessage(status: number, statusText: string, responseText: string) {
  const trimmed = responseText.trim()
  let detail = ''

  if (trimmed) {
    try {
      detail = extractErrorMessageFromJson(JSON.parse(trimmed)) || trimmed
    } catch {
      detail = trimmed
    }
  }

  return `API error ${status}: ${detail || statusText || 'Request failed'}`
}

export async function apiFetch<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const method = options?.method || 'GET'
  const body = options?.body ? JSON.parse(options.body as string) : undefined
  const requestPath = resolveApiPath(path)

  logRequest(method, requestPath, body)

  try {
    const token = tokenRef.get()
    const headers = new Headers(options?.headers)
    headers.set('Content-Type', 'application/json')
    headers.set('x-tenant-id', DEFAULT_TENANT_ID)
    headers.set('x-feature', FEATURE_HEADER)
    if (token) {
      headers.set('Authorization', `Bearer ${token}`)
    }

    const res = await fetch(requestPath, {
      ...options,
      headers,
    })

    const responseText = await res.text()
    let responseBody: string

    try {
      const json = JSON.parse(responseText)
      responseBody = JSON.stringify(json, null, 2)
    } catch {
      responseBody = responseText
    }

    logResponse(requestPath, res.status, res.statusText, responseBody)

    // Only clear session when the JWT itself is confirmed invalid/expired.
    // A generic 401 (e.g. missing API key on a protected route) must NOT wipe the session.
    if (res.status === 401 && !path.startsWith('/auth/')) {
      let isTokenExpiry = false
      try {
        const json = JSON.parse(responseText)
        const message = `${json.message ?? ''}`.toLowerCase()
        isTokenExpiry =
          message.includes('expired') ||
          message.includes('invalid or expired')
      } catch {
        // Ignore non-JSON 401s; not every unauthorized response should clear the session.
      }

      if (isTokenExpiry) {
        tokenRef.set(null)
        import('../store/authStore').then(({ useAuthStore }) => {
          useAuthStore.getState().clearAuth()
        })
        window.location.href = `${import.meta.env.BASE_URL}login`
        throw new Error('Session expired')
      }
    }

    if (!res.ok) {
      const error = new Error(buildApiErrorMessage(res.status, res.statusText, responseText)) as Error & {
        status?: number
        responseText?: string
      }
      error.status = res.status
      error.responseText = responseText
      logError(requestPath, error)
      throw error
    }

    if (!responseText.trim()) {
      return undefined as T
    }

    return JSON.parse(responseText) as T
  } catch (error) {
    logError(requestPath, error)
    throw error
  }
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: 'POST',
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
}

export async function fetcher<T>(url: string): Promise<T> {
  return apiFetch<T>(url)
}
