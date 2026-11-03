import { useAuthStore, type MerchantInfo } from '../store/authStore'

/**
 * Where the session sits among Decision Engine merchants and their profiles.
 *
 * A profile has no members or API keys of its own: both live on its merchant and reach every one of
 * its profiles. Pages that manage either read this to say so, and to offer only what the backend
 * would accept — it re-checks every request regardless.
 */
export interface MerchantScope {
  /** The scope the session is on. */
  current: MerchantInfo | undefined
  /** The merchant that owns `current`: its parent for a profile, otherwise `current` itself. */
  merchant: MerchantInfo | undefined
  /** The session is on a profile of a Decision Engine merchant. */
  isProfile: boolean
  /** Hyperswitch owns this scope's tree, members and access; nothing here manages them. */
  isHyperswitch: boolean
  /** The session's role on `merchant` is admin. */
  isAdmin: boolean
}

export function useMerchantScope(): MerchantScope {
  const user = useAuthStore((s) => s.user)
  const merchants = useAuthStore((s) => s.merchants)
  const current = merchants.find((m) => m.merchant_id === user?.merchantId)
  const parentId = current?.parent_merchant_id
  const merchant = parentId ? merchants.find((m) => m.merchant_id === parentId) : current
  return {
    current,
    merchant,
    isProfile: Boolean(parentId),
    isHyperswitch: Boolean(user?.isRedirectSession || user?.hierarchy || current?.hs_merchant_id),
    // The list carries the role per scope; the session's own `role` is not refreshed by a switch.
    isAdmin: (current?.role ?? user?.role) === 'admin',
  }
}

/** A merchant's display name, falling back to its id. */
export function merchantLabel(merchant: MerchantInfo | undefined, fallback = ''): string {
  return merchant?.merchant_name || merchant?.merchant_id || fallback
}
