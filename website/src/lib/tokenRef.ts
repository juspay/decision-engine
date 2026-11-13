// Shared mutable ref to break circular dependency between api.ts and authStore.ts
import { isTokenExpired } from './sessionExpiry'

/**
 * A token arriving in this tab is never read as expired for this long, whatever the clocks say. A
 * token we just received was live a moment ago by definition, so this only ever bites when the
 * device clock jumps — and it guarantees a re-mint can never be answered by an immediate second
 * re-mint.
 */
const FRESH_TOKEN_GRACE_MS = 60_000

let _token: string | null = null
let _clockSkew: number | null = null
// Monotonic time the current token reached this tab; immune to device clock changes.
let _receivedAt = 0

export const tokenRef = {
  get: () => _token,
  /** Hold a token that has just arrived from the server. */
  set: (t: string | null, clockSkew: number | null = null) => {
    if (t !== _token) _receivedAt = performance.now()
    _token = t
    _clockSkew = t ? clockSkew : null
  },
  /**
   * Hold a token restored from storage. It did not just arrive, so it gets no grace: one that
   * lapsed while the tab was closed reads as expired straight away.
   */
  restore: (t: string, clockSkew: number | null) => {
    _receivedAt = Number.NEGATIVE_INFINITY
    _token = t
    _clockSkew = clockSkew
  },
  /** Whether the held token has lapsed on the server's clock. See `isTokenExpired`. */
  isExpired: () => {
    if (!_token) return false
    if (performance.now() - _receivedAt < FRESH_TOKEN_GRACE_MS) return false
    return isTokenExpired(_token, _clockSkew)
  },
}
