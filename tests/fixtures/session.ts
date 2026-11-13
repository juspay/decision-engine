import type { ApiClient } from './api-client'

export interface Session {
  token: string
  user: {
    userId?: string
    email?: string
    merchantId?: string
    role?: string
  }
}

// Per-worker cache so repeated merchant setups within a worker reuse the session.
const sessionCache = new Map<string, Session>()

/**
 * Drop a merchant's cached session. Call from fixture teardown alongside `cleanupTestData` — the
 * Cypress original evicts here too (cypress/support/commands.js `cleanupTestData`), and without it the
 * map grows unbounded across a long UI run.
 */
export function clearSession(merchantId: string): void {
  sessionCache.delete(merchantId)
}

function toSession(body: any): Session {
  return {
    token: body.token,
    user: {
      userId: body.user_id,
      email: body.email,
      merchantId: body.merchant_id,
      role: body.role,
    },
  }
}

/**
 * Port of Cypress `ensureDashboardSession` (commands.js). Establishes a dashboard session for a
 * merchant: POST /auth/signup, falling back to POST /auth/login when the account already exists.
 * Sets the bearer token on the client so subsequent protected calls authenticate automatically.
 */
export async function ensureDashboardSession(client: ApiClient, merchantId: string): Promise<Session> {
  const cached = sessionCache.get(merchantId)
  if (cached) {
    client.token = cached.token
    return cached
  }

  const email = `${merchantId}@example.com`
  const password = 'Password123!'

  const signup = await client.raw('POST', '/auth/signup', {
    failOnStatusCode: false,
    body: { email, password, merchant_id: merchantId },
  })

  let session: Session
  if (signup.status === 200 && signup.body?.token) {
    session = toSession(signup.body)
  } else {
    const login = await client.raw('POST', '/auth/login', { body: { email, password } })
    session = toSession(login.body)
  }

  sessionCache.set(merchantId, session)
  client.token = session.token
  return session
}
