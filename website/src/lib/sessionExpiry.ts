/**
 * Local reading of a session token's lifetime, so a token that has already lapsed is never sent.
 *
 * Presentation only: nothing here verifies the token. The backend checks the signature, expiry and
 * revocation of every request, so a wrong answer here costs at most a request the server refuses
 * (if we think a dead token is live) or an early hand-back to the expiry flow (if we think a live
 * one is dead) — never access.
 *
 * Expiry is judged on the server's clock, not the device's. When a token arrives, its `iat` (the
 * server's time at mint) is compared with the device clock to get the skew between them, and that
 * skew is stored alongside the token. A device clock running hours fast therefore cannot make a
 * freshly minted token read as expired, which would otherwise send an embedded session into a
 * re-mint that can never succeed.
 */

interface TokenTimes {
  iat: number
  exp: number
}

/** `iat` and `exp` from the token's payload, or `null` when it is not a JWT carrying both. */
function readTokenTimes(token: string): TokenTimes | null {
  const payload = token.split('.')[1]
  if (!payload) return null
  try {
    const base64 = payload.replace(/-/g, '+').replace(/_/g, '/')
    const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4)
    const claims = JSON.parse(atob(padded)) as { iat?: unknown; exp?: unknown }
    const { iat, exp } = claims
    if (typeof iat !== 'number' || typeof exp !== 'number') return null
    if (!Number.isFinite(iat) || !Number.isFinite(exp) || exp <= iat) return null
    return { iat, exp }
  } catch {
    return null
  }
}

function deviceNowSeconds(): number {
  return Date.now() / 1000
}

/**
 * Seconds the server's clock is ahead of this device's, measured from a token that has just
 * arrived. `null` when the token carries no readable `iat`/`exp`, in which case expiry is left
 * entirely to the server.
 */
export function measureClockSkew(token: string, nowSeconds = deviceNowSeconds()): number | null {
  const times = readTokenTimes(token)
  return times ? times.iat - nowSeconds : null
}

/**
 * Whether `token` has expired on the server's clock.
 *
 * Answers `false` whenever it cannot be sure — an unreadable token, or no skew recorded for it (a
 * session stored before skew was tracked) — so that case falls back to the server's 401, exactly
 * as before.
 */
export function isTokenExpired(
  token: string,
  clockSkew: number | null | undefined,
  nowSeconds = deviceNowSeconds(),
): boolean {
  if (typeof clockSkew !== 'number' || !Number.isFinite(clockSkew)) return false
  const times = readTokenTimes(token)
  if (!times) return false
  return nowSeconds + clockSkew >= times.exp
}
