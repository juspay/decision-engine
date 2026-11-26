import { createHmac } from 'node:crypto'
import { test, expect, factory, poll } from '../../fixtures/test'
import type { ApiClient } from '../../fixtures/api-client'

/**
 * Two-factor authentication (src/routes/two_factor_auth.rs) on the instance playwright.config.ts starts
 * with `user_auth.two_factor_auth.mode = required` (API_BASE_URL_2FA), which sends its email to Mailpit.
 * Users are created on the suite's own server, where two-factor authentication is off — so every user
 * here starts as an existing account does when the requirement is switched on.
 *
 * Each test uses its own user: code replay, attempt counts and the resend interval are all per user,
 * so tests sharing one would trip over each other.
 */

const TFA = process.env.API_BASE_URL_2FA || 'http://localhost:8097'
const MAILPIT = process.env.MAILPIT_URL || 'http://localhost:8025'
const PASSWORD = 'Password123!'
const STEP_SECONDS = 30

test.beforeAll(async ({ request }) => {
  const reachable = await request.get(`${TFA}/health`).then((r) => r.ok(), () => false)
  // CI always starts the two-factor instance; a local run against a single server skips instead.
  if (!process.env.CI) test.skip(!reachable, `two-factor instance not running at ${TFA}`)
  expect(reachable, `two-factor instance not running at ${TFA}`).toBe(true)
})

// ---- TOTP (RFC 6238: SHA-1, 6 digits, 30-second steps — what the server uses) -------------------

function base32Decode(input: string): Buffer {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'
  let bits = 0
  let value = 0
  const out: number[] = []
  for (const char of input.replace(/=+$/, '').toUpperCase()) {
    value = (value << 5) | alphabet.indexOf(char)
    bits += 5
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff)
      bits -= 8
    }
  }
  return Buffer.from(out)
}

function codeAt(secret: string, step: number): string {
  const counter = Buffer.alloc(8)
  counter.writeBigUInt64BE(BigInt(step))
  const mac = createHmac('sha1', base32Decode(secret)).update(counter).digest()
  const offset = mac[mac.length - 1] & 0x0f
  return String((mac.readUInt32BE(offset) & 0x7fffffff) % 1_000_000).padStart(6, '0')
}

const currentStep = () => Math.floor(Date.now() / 1000 / STEP_SECONDS)

/**
 * The server refuses a code whose step was already used, and accepts one step either side of the
 * current one. Hands out a code for an unused step in that window, preferring the newest so it stays
 * valid the longest.
 */
class Authenticator {
  private used = new Set<number>()
  constructor(readonly secret: string) {}

  code(): string {
    const now = currentStep()
    const step = [now + 1, now, now - 1].find((s) => !this.used.has(s))
    if (step === undefined) throw new Error('every step in the window is used; wait for the next one')
    this.used.add(step)
    return codeAt(this.secret, step)
  }

  /** A code no step in the window produces. */
  wrongCode(): string {
    const now = currentStep()
    const valid = new Set([now - 1, now, now + 1].map((s) => codeAt(this.secret, s)))
    for (let n = 0; ; n++) {
      const candidate = String(n).padStart(6, '0')
      if (!valid.has(candidate)) return candidate
    }
  }
}

// ---- Mailpit -------------------------------------------------------------------------------------

/** The token from the newest two-factor verification email sent to `email`. */
async function emailedLinkToken(api: ApiClient, email: string): Promise<string> {
  const search = await poll(
    () =>
      api.raw('GET', `${MAILPIT}/api/v1/search`, {
        failOnStatusCode: false,
        qs: { query: `to:"${email}" subject:"two-factor"` },
      }),
    (r) => r.status === 200 && r.body.messages?.length > 0,
    { message: `no two-factor email reached Mailpit for ${email}`, timeout: 20_000, interval: 500 },
  )
  const message = await api.raw('GET', `${MAILPIT}/api/v1/message/${search.body.messages[0].ID}`)
  const token = /\/two-factor\/verify-email\?token=([A-Za-z0-9_]+)/.exec(message.body.HTML)?.[1]
  expect(token, 'the email carries a verification link').toBeTruthy()
  return token as string
}

// ---- Flow helpers --------------------------------------------------------------------------------

/** An account created where two-factor authentication is off: an existing user, to the 2FA instance. */
async function existingUser(api: ApiClient): Promise<string> {
  const email = `${factory.merchantId('tfa')}@example.com`.toLowerCase()
  const r = await api.anonymous().raw('POST', '/auth/signup', { body: { email, password: PASSWORD } })
  expect(r.body.token, 'signup where 2FA is off returns a session').toBeTruthy()
  return email
}

async function login(api: ApiClient, email: string) {
  const r = await api.anonymous().raw('POST', `${TFA}/auth/login`, { body: { email, password: PASSWORD } })
  expect(r.body.two_factor_required, JSON.stringify(r.body)).toBe(true)
  expect(r.body.token, 'a held login carries no session').toBeUndefined()
  return r.body as { flow_token: string; next_step: string; email: string }
}

/** Takes `email` from login through enrolment; returns its authenticator, recovery codes and session. */
async function enroll(api: ApiClient, email: string) {
  const challenge = await login(api, email)
  expect(challenge.next_step).toBe('verify_email')
  const anon = api.anonymous()

  const confirmed = await anon.raw('POST', `${TFA}/auth/2fa/email/confirm`, {
    body: { token: await emailedLinkToken(api, email) },
  })
  expect(confirmed.body.next_step).toBe('setup_totp')
  const flow = confirmed.body.flow_token

  const setup = await anon.raw('POST', `${TFA}/auth/2fa/totp/setup`, { body: { flow_token: flow } })
  const authenticator = new Authenticator(setup.body.secret)
  const enabled = await anon.raw('POST', `${TFA}/auth/2fa/totp/enable`, {
    body: { flow_token: flow, totp: authenticator.code() },
  })
  return {
    authenticator,
    recoveryCodes: enabled.body.recovery_codes as string[],
    session: enabled.body.session.token as string,
  }
}

const bearer = (token: string) => ({ Authorization: `Bearer ${token}` })

// ---- Specs ---------------------------------------------------------------------------------------

test.describe('Two-factor authentication (API, mode = required)', () => {
  test('an existing user re-verifies their email, and setup continues only from the link', async ({ api }) => {
    const email = await existingUser(api)
    const anon = api.anonymous()

    const challenge = await login(api, email)
    expect(challenge.next_step).toBe('verify_email')
    expect(challenge.email).toBe(email)

    // The flow token is not a session, and holding it — the password — does not reach setup.
    const me = await anon.raw('GET', `${TFA}/auth/me`, { failOnStatusCode: false, headers: bearer(challenge.flow_token) })
    expect(me.status).toBe(401)
    const early = await anon.raw('POST', `${TFA}/auth/2fa/totp/setup`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token },
    })
    expect(early.status).toBe(401)

    // Login already sent the email; another inside the minute is refused.
    const resend = await anon.raw('POST', `${TFA}/auth/2fa/email/resend`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token },
    })
    expect(resend.status).toBe(429)

    const link = await emailedLinkToken(api, email)
    const confirmed = await anon.raw('POST', `${TFA}/auth/2fa/email/confirm`, { body: { token: link } })
    expect(confirmed.body.next_step).toBe('setup_totp')
    expect(confirmed.body.flow_token).not.toBe(challenge.flow_token)

    // The link is single-use, and redeeming it ends the flow that only had the password.
    const again = await anon.raw('POST', `${TFA}/auth/2fa/email/confirm`, { failOnStatusCode: false, body: { token: link } })
    expect(again.status).toBe(401)
    const stale = await anon.raw('POST', `${TFA}/auth/2fa/email/resend`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token },
    })
    expect(stale.status).toBe(401)

    const flow = confirmed.body.flow_token
    const notStarted = await anon.raw('POST', `${TFA}/auth/2fa/totp/enable`, {
      failOnStatusCode: false,
      body: { flow_token: flow, totp: '000000' },
    })
    expect(notStarted.status).toBe(400)

    const setup = await anon.raw('POST', `${TFA}/auth/2fa/totp/setup`, { body: { flow_token: flow } })
    expect(setup.body.otpauth_url).toMatch(/^otpauth:\/\/totp\//)
    const authenticator = new Authenticator(setup.body.secret)

    const wrong = await anon.raw('POST', `${TFA}/auth/2fa/totp/enable`, {
      failOnStatusCode: false,
      body: { flow_token: flow, totp: authenticator.wrongCode() },
    })
    expect(wrong.status).toBe(401)

    const enabled = await anon.raw('POST', `${TFA}/auth/2fa/totp/enable`, {
      body: { flow_token: flow, totp: authenticator.code() },
    })
    expect(enabled.body.recovery_codes).toHaveLength(8)
    const session = enabled.body.session.token
    expect((await anon.raw('GET', `${TFA}/auth/me`, { headers: bearer(session) })).body.email).toBe(email)

    const status = await anon.raw('GET', `${TFA}/auth/2fa/status`, { headers: bearer(session) })
    expect(status.body).toEqual({ mode: 'required', enrolled: true, recovery_codes_remaining: 8 })
  })

  test('signup returns a challenge instead of a session', async ({ api }) => {
    const email = `${factory.merchantId('tfa_signup')}@example.com`.toLowerCase()
    const r = await api.anonymous().raw('POST', `${TFA}/auth/signup`, { body: { email, password: PASSWORD } })
    expect(r.body.two_factor_required).toBe(true)
    expect(r.body.next_step).toBe('verify_email')
    expect(r.body.token).toBeUndefined()
    await emailedLinkToken(api, email)
  })

  test('an enrolled user signs in with a code, and neither the code nor the flow works twice', async ({ api }) => {
    const email = await existingUser(api)
    const { authenticator } = await enroll(api, email)
    const anon = api.anonymous()

    const challenge = await login(api, email)
    expect(challenge.next_step).toBe('verify_totp')

    const code = authenticator.code()
    const signedIn = await anon.raw('POST', `${TFA}/auth/2fa/totp/verify`, {
      body: { flow_token: challenge.flow_token, totp: code },
    })
    expect(signedIn.body.token).toBeTruthy()
    expect(signedIn.body.email).toBe(email)

    const reusedFlow = await anon.raw('POST', `${TFA}/auth/2fa/totp/verify`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token, totp: authenticator.code() },
    })
    expect(reusedFlow.status).toBe(401)

    const next = await login(api, email)
    const replayed = await anon.raw('POST', `${TFA}/auth/2fa/totp/verify`, {
      failOnStatusCode: false,
      body: { flow_token: next.flow_token, totp: code },
    })
    expect(replayed.status).toBe(401)
  })

  test('a recovery code signs in once, however it is typed', async ({ api }) => {
    const email = await existingUser(api)
    const { recoveryCodes, session } = await enroll(api, email)
    const anon = api.anonymous()

    const first = await login(api, email)
    const signedIn = await anon.raw('POST', `${TFA}/auth/2fa/recovery-code/verify`, {
      body: { flow_token: first.flow_token, recovery_code: recoveryCodes[0].toUpperCase() },
    })
    expect(signedIn.body.token).toBeTruthy()

    const second = await login(api, email)
    const reused = await anon.raw('POST', `${TFA}/auth/2fa/recovery-code/verify`, {
      failOnStatusCode: false,
      body: { flow_token: second.flow_token, recovery_code: recoveryCodes[0] },
    })
    expect(reused.status).toBe(401)

    const status = await anon.raw('GET', `${TFA}/auth/2fa/status`, { headers: bearer(session) })
    expect(status.body.recovery_codes_remaining).toBe(7)
  })

  test('a signed-in user replaces recovery codes, but cannot turn 2FA off or enrol twice', async ({ api }) => {
    const email = await existingUser(api)
    const { authenticator, recoveryCodes, session } = await enroll(api, email)
    const anon = api.anonymous()

    const disable = await anon.raw('POST', `${TFA}/auth/2fa/disable`, {
      failOnStatusCode: false,
      headers: bearer(session),
      body: { totp: authenticator.code() },
    })
    expect(disable.status).toBe(403)

    const regenerated = await anon.raw('POST', `${TFA}/auth/2fa/recovery-codes`, {
      headers: bearer(session),
      body: { totp: authenticator.code() },
    })
    expect(regenerated.body.recovery_codes).toHaveLength(8)

    // The old set stops working.
    const challenge = await login(api, email)
    const old = await anon.raw('POST', `${TFA}/auth/2fa/recovery-code/verify`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token, recovery_code: recoveryCodes[1] },
    })
    expect(old.status).toBe(401)

    const enrollAgain = await anon.raw('POST', `${TFA}/auth/2fa/enroll`, { failOnStatusCode: false, headers: bearer(session) })
    expect(enrollAgain.status).toBe(409)
  })

  test('wrong codes are limited, and the limit holds even for a right one', async ({ api }) => {
    const email = await existingUser(api)
    const { authenticator } = await enroll(api, email)
    const anon = api.anonymous()
    const challenge = await login(api, email)

    const statuses: number[] = []
    for (let i = 0; i < 6; i++) {
      const r = await anon.raw('POST', `${TFA}/auth/2fa/totp/verify`, {
        failOnStatusCode: false,
        body: { flow_token: challenge.flow_token, totp: authenticator.wrongCode() },
      })
      statuses.push(r.status)
    }
    expect(statuses).toEqual([401, 401, 401, 401, 401, 429])

    const right = await anon.raw('POST', `${TFA}/auth/2fa/totp/verify`, {
      failOnStatusCode: false,
      body: { flow_token: challenge.flow_token, totp: authenticator.code() },
    })
    expect(right.status).toBe(429)
  })
})
