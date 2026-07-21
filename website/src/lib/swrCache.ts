import { mutate } from 'swr'

function getCachePath(key: unknown): string | null {
  if (typeof key === 'string') return key
  if (Array.isArray(key) && typeof key[0] === 'string') return key[0]
  return null
}

function isSessionScopedApiKey(key: unknown): boolean {
  const path = getCachePath(key)
  if (!path?.startsWith('/')) return false

  return path !== '/auth/me' && !path.startsWith('/health')
}

export function refreshSessionScopedSWRCache() {
  void mutate(isSessionScopedApiKey, undefined, { revalidate: true }).catch(() => undefined)
}
