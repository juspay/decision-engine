// Dev uses the Vite proxy; production uses the hosted dashboard API path.
import { tokenRef } from './tokenRef'

const DEBUG_API = import.meta.env.DEV
const DEFAULT_TENANT_ID = import.meta.env.VITE_DEFAULT_TENANT_ID ?? 'public'
const DEFAULT_API_BASE_PATH = import.meta.env.PROD ? '/decision-engine/api' : '/decision-engine-api'
const API_BASE_PATH = (import.meta.env.VITE_API_BASE_PATH ?? DEFAULT_API_BASE_PATH).replace(/\/$/, '')
const FEATURE_HEADER = import.meta.env.VITE_FEATURE_HEADER ?? 'decision-engine'

function resolveApiPath(path: string) {
  if (/^https?:\/\//.test(path)) return path
  const normalizedPath = path.startsWith('/') ? path : `/${path}`
  if (normalizedPath.startsWith(`${API_BASE_PATH}/`) || normalizedPath === API_BASE_PATH) {
    return normalizedPath
  }
  return `${API_BASE_PATH}${normalizedPath}`
}

/**
 * Absolute URL for a backend path, for showing a caller-facing endpoint (a connector webhook, say)
 * that someone will paste elsewhere. Deliberately not `API_BASE_PATH`: that is *this browser's*
 * route to the API — a Vite proxy prefix in dev — which means nothing to a connector calling in
 * from outside. Callers show these on the hosted dashboard only, so the default is the gateway's
 * prefix; set `VITE_PUBLIC_API_BASE_URL` when a deployment answers external callers elsewhere.
 */
export function publicApiUrl(path: string) {
  const override = import.meta.env.VITE_PUBLIC_API_BASE_URL?.trim()
  const base = (override || `${window.location.origin}/decision-engine/api`).replace(/\/$/, '')
  return `${base}${path.startsWith('/') ? path : `/${path}`}`
}

// function logRequest(method: string, path: string, body?: unknown) {
//   if (!DEBUG_API) return
//   console.log('\n' + '='.repeat(80))
//   console.log(`[API REQUEST] ${new Date().toISOString()}`)
//   console.log(`Method: ${method}`)
//   console.log(`Path: ${path}`)
//   if (body !== undefined) {
//     console.log('Body:', JSON.stringify(body, null, 2))
//   }
//   console.log('='.repeat(80))
// }

// function logResponse(path: string, status: number, statusText: string, body: string) {
//   if (!DEBUG_API) return
//   console.log('\n' + '-'.repeat(80))
//   console.log(`[API RESPONSE] ${new Date().toISOString()}`)
//   console.log(`Path: ${path}`)
//   console.log(`Status: ${status} ${statusText}`)
//   console.log('Response Body:', body)
//   console.log('-'.repeat(80) + '\n')
// }

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

/** Longest server message worth showing verbatim. Past this it is a dump, not a sentence. */
const MAX_DETAIL_LENGTH = 200

const GENERIC_ERROR = 'Something went wrong. Please try again.'

/**
 * The message a user sees when a request fails.
 *
 * Only a body that reads like a sentence written for a person is shown verbatim — our API's own
 * errors ("ingestion not found", "unknown connector: foo") are exactly that, and are far more useful
 * than a generic line. Everything else collapses to [`GENERIC_ERROR`]:
 *
 *  - **HTML** — the request was answered by a CDN, ingress, or the marketing site rather than the
 *    API (every backend error is plain text or JSON), so the body describes someone else's 404 page.
 *    It was previously pasted into the UI in full, kilobytes of markup and consent-banner scripts.
 *  - **Anything over [`MAX_DETAIL_LENGTH`]** — a stack trace or a serialized blob, not a message.
 *
 * The raw body is never lost: it goes to the console with the status, so a failure stays debuggable
 * without putting a wall of markup in front of the user.
 *
 * No `API error <status>:` prefix — a status code is not information a merchant can act on, and it
 * invited callers to re-parse the message. Use [`apiErrorMessage`] to display a failure and
 * [`apiErrorStatus`] to branch on the code.
 */
function buildApiErrorMessage(status: number, statusText: string, responseText: string) {
  const trimmed = responseText.trim()
  if (trimmed) {
    console.error(`[api] ${status} ${statusText || ''}`.trim(), trimmed.slice(0, 2000))
  }

  const isHtml = /^\s*(<!doctype html|<html\b|<\?xml\b)/i.test(trimmed)
  if (!trimmed || isHtml) return GENERIC_ERROR

  let detail = ''
  try {
    detail = extractErrorMessageFromJson(JSON.parse(trimmed)) || trimmed
  } catch {
    detail = trimmed
  }
  detail = detail.trim()

  return detail && detail.length <= MAX_DETAIL_LENGTH ? detail : GENERIC_ERROR
}

/** HTTP status of a rejected API call, or `undefined` if it failed before getting one (network, abort). */
export function apiErrorStatus(err: unknown): number | undefined {
  return typeof err === 'object' && err ? (err as { status?: number }).status : undefined
}

/**
 * The display message for a rejected API call.
 *
 * [`buildApiErrorMessage`] has already reduced the response to something showable — a short server
 * sentence or [`GENERIC_ERROR`] — so this only unwraps the `Error` and supplies a caller-specific
 * fallback for a non-`Error` rejection. Callers must NOT re-parse the message: it once carried an
 * `API error <status>: ` prefix (and sometimes raw JSON) that every page peeled off by hand, which
 * is exactly the duplication this replaces. Read [`apiErrorStatus`] when you need the code.
 */
export function apiErrorMessage(err: unknown, fallback = 'Something went wrong'): string {
  const message = err instanceof Error ? err.message.trim() : ''
  return message || fallback
}

export async function apiFetch<T>(
  path: string,
  options?: RequestInit
): Promise<T> {
  const requestPath = resolveApiPath(path)

  // logRequest(method, requestPath, body)

  try {
    const token = tokenRef.get()
    const headers = new Headers(options?.headers)
    headers.set('Content-Type', 'application/json')
    headers.set('x-tenant-id', DEFAULT_TENANT_ID)
    headers.set('x-feature', FEATURE_HEADER)
    if (token) {
      headers.set('Authorization', `Bearer ${token}`)
    }

    let res: Response
    try {
      res = await fetch(requestPath, { ...options, headers })
    } catch (cause) {
      // `fetch` rejects only when no response arrived at all — offline, DNS/TLS failure, CORS
      // rejection, or the request being cut. The browser's own text ("Failed to fetch",
      // "NetworkError when attempting to fetch resource") names none of that usefully, and carries
      // no status for callers to branch on.
      const error = new Error('Could not reach the server. Check your connection and try again.') as
        Error & { cause?: unknown }
      error.cause = cause
      logError(requestPath, cause)
      throw error
    }

    const responseText = await res.text()
    // let responseBody: string

    // try {
    //   const json = JSON.parse(responseText)
    //   responseBody = JSON.stringify(json, null, 2)
    // } catch {
    //   responseBody = responseText
    // }

    // logResponse(requestPath, res.status, res.statusText, responseBody)

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

    try {
      return JSON.parse(responseText) as T
    } catch (cause) {
      // A 2xx whose body is not JSON. An edge (CDN, ingress, SPA fallback) can answer 200 with an
      // HTML page, and a bare `JSON.parse` failure surfaces as "Unexpected token < in JSON at
      // position 0" — which reads like an app bug rather than a request that never reached the API.
      const error = new Error(GENERIC_ERROR) as Error & {
        status?: number
        responseText?: string
        cause?: unknown
      }
      error.status = res.status
      error.responseText = responseText
      error.cause = cause
      console.error(`[api] ${res.status} non-JSON response`, responseText.slice(0, 2000))
      logError(requestPath, error)
      throw error
    }
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

export async function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: 'PUT',
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
}

export async function apiDelete<T = void>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: 'DELETE' })
}

/** Progress of a tracked upload. `phase` flips to `processing` once all bytes are sent and the
 * server is parsing/fitting — where the request can sit for minutes on a large report. */
export interface UploadProgress {
  loaded: number
  total: number
  phase: 'uploading' | 'processing'
}

/**
 * POST a raw binary body with upload-progress reporting. Uses `XMLHttpRequest` because `fetch`
 * exposes no upload-progress events. `onProgress` fires during transfer (`uploading`) and once more
 * when the last byte is sent (`processing`) — the server-side parse/fit is not observable from here.
 */
export function apiUploadWithProgress<T>(
  path: string,
  file: Blob,
  onProgress?: (p: UploadProgress) => void,
): Promise<T> {
  const requestPath = resolveApiPath(path)
  const token = tokenRef.get()

  return new Promise<T>((resolve, reject) => {
    const xhr = new XMLHttpRequest()
    xhr.open('POST', requestPath)
    xhr.setRequestHeader('Content-Type', 'application/octet-stream')
    xhr.setRequestHeader('x-tenant-id', DEFAULT_TENANT_ID)
    xhr.setRequestHeader('x-feature', FEATURE_HEADER)
    if (token) {
      xhr.setRequestHeader('Authorization', `Bearer ${token}`)
    }

    xhr.upload.onprogress = (e) => {
      if (e.lengthComputable) {
        onProgress?.({ loaded: e.loaded, total: e.total, phase: 'uploading' })
      }
    }
    // All bytes sent — the server is now parsing/staging/fitting.
    xhr.upload.onload = () => {
      onProgress?.({ loaded: file.size, total: file.size, phase: 'processing' })
    }

    xhr.onload = () => {
      const responseText = xhr.responseText ?? ''
      if (xhr.status >= 200 && xhr.status < 300) {
        if (!responseText.trim()) {
          resolve(undefined as T)
          return
        }
        try {
          resolve(JSON.parse(responseText) as T)
        } catch {
          reject(new Error('Invalid JSON in upload response'))
        }
        return
      }
      const error = new Error(
        buildApiErrorMessage(xhr.status, xhr.statusText, responseText),
      ) as Error & { status?: number; responseText?: string }
      error.status = xhr.status
      error.responseText = responseText
      logError(requestPath, error)
      reject(error)
    }
    xhr.timeout = 30 * 60 * 1000
    xhr.onerror = () => reject(new Error('Could not reach the server. Check your connection and try again.'))
    xhr.ontimeout = () => reject(new Error('Upload timed out. Please try again.'))
    // Without this the promise never settles when the transfer is cut (navigation, tab close, an
    // explicit abort), leaving the caller's spinner running forever.
    xhr.onabort = () => reject(new Error('Upload cancelled.'))

    xhr.send(file)
  })
}

export async function fetcher<T>(url: string): Promise<T> {
  return apiFetch<T>(url)
}
