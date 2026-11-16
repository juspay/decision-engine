import { tokenRef } from './tokenRef'
import { discardLocalSession } from './api'

// Where `useAuthStore`'s `persist` keeps the session
const AUTH_STORAGE_KEY = 'auth-store'

function tokenIn(raw: string | null): string | null {
  if (!raw) return null
  try {
    const snapshot = JSON.parse(raw) as { state?: { token?: string | null } }
    return snapshot.state?.token ?? null
  } catch {
    return null
  }
}

/**
 * Call `onSignedOut` when another tab signs out, after dropping the token here. Returns the
 * unsubscribe.
 *
 * Both tabs hold the same token and `/auth/logout` revokes it for both, so without this the other
 * tab keeps polling with a token the server now refuses. A stored token that merely changed is a
 * scope switch or a fresh login elsewhere, which leaves this tab's own token valid.
 */
export function watchForSignOutInAnotherTab(onSignedOut: () => void): () => void {
  const onStorage = (event: StorageEvent) => {
    if (event.storageArea && event.storageArea !== window.localStorage) return
    // A null key is `localStorage.clear()`
    if (event.key !== null && event.key !== AUTH_STORAGE_KEY) return
    // Nothing to end, and what stops two tabs echoing clears at each other
    if (!tokenRef.get()) return
    if (tokenIn(event.newValue)) return
    discardLocalSession()
    onSignedOut()
  }
  window.addEventListener('storage', onStorage)
  return () => window.removeEventListener('storage', onStorage)
}
