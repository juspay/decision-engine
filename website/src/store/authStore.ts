import { create } from 'zustand'
import { clearDashboardHandoff } from '../lib/dashboardHandoff'
import { persist } from 'zustand/middleware'
import { tokenRef } from '../lib/tokenRef'
import { measureClockSkew } from '../lib/sessionExpiry'

/**
 * One scope this session can switch to.
 *
 * `merchant_id` is the routing scope and `merchant_name` its flat one-line label; the `hs_*` and
 * `profile_name` fields are the same scope split into account-tree levels, which is what the
 * switcher groups and searches on. All of them are absent for a scope with no Hyperswitch
 * ancestry — every DE-native merchant — so each consumer falls back to the flat label.
 *
 * A sub-merchant carries `parent_merchant_id` instead, and is listed right after its
 * merchant with the role held there.
 */
export interface MerchantInfo {
  merchant_id: string
  merchant_name: string
  role: string
  parent_merchant_id?: string
  profile_name?: string
  hs_merchant_id?: string
  hs_merchant_name?: string
  hs_org_id?: string
  hs_org_name?: string
}

/**
 * Where a scope sits in the Hyperswitch account tree, for scopes that came from one.
 *
 * A Decision Engine scope is a Hyperswitch *profile*; the org and merchant above it are ancestry
 * Hyperswitch owns and syncs down. Absent for merchants created directly in Decision Engine and
 * for profiles not yet synced, so every consumer must handle it being missing.
 */
export interface AccountHierarchy {
  hs_org_id: string
  hs_org_name?: string
  hs_merchant_id: string
  hs_merchant_name?: string
  profile_name?: string
  synced_at?: string
}

/**
 * One thing a session is allowed to do, in the wire spelling the backend uses.
 *
 * Open-ended on purpose: a Decision Engine newer than this dashboard may report a permission that
 * is not listed here, and it should arrive intact rather than be dropped in transit.
 */
export type Permission = 'routing:read' | 'routing:write' | (string & {})

export interface AuthUser {
  userId: string
  email: string
  merchantId: string
  role: string
  isRedirectSession: boolean
  /**
   * What this session may do. Undefined only in the moment between minting a token and the first
   * `/auth/me`, where it is read as unrestricted — the backend refuses the request either way, so
   * the cost of guessing wrong here is a button that turns out not to work, never a bypass.
   */
  permissions?: Permission[]
  // This user's email is on the platform super-admin roster (may enter any merchant by ID).
  isSuperAdmin?: boolean
  // The current session is a super-admin viewing another merchant's dashboard.
  isSuperAdminView?: boolean
  // Hyperswitch ancestry for `merchantId`, when it has any.
  hierarchy?: AccountHierarchy
}

interface AuthStore {
  token: string | null
  /**
   * Seconds the server's clock was ahead of this device's when `token` arrived, so its expiry can
   * be read on the server's clock (see `lib/sessionExpiry`). `null` for a session stored before this
   * was tracked; such a session is left to the server to expire.
   */
  tokenClockSkew: number | null
  user: AuthUser | null
  merchants: MerchantInfo[]
  hasHydrated: boolean
  setAuth: (token: string, user: AuthUser, merchants?: MerchantInfo[]) => void
  updateMerchant: (token: string, merchantId: string, merchants: MerchantInfo[]) => void
  clearAuth: () => void
  setHasHydrated: (hasHydrated: boolean) => void
}

/**
 * Whether the session holds `permission`.
 *
 * Presentation only — it decides which controls are offered, never whether a request is allowed.
 * The backend re-checks every request against the same permission, so a stale or missing value here
 * cannot become access.
 */
export function sessionAllows(user: AuthUser | null, permission: Permission): boolean {
  if (!user?.permissions) return true
  return user.permissions.includes(permission)
}

/** Whether routing rules can be created, edited, or activated in this session. */
export function useCanEditRouting(): boolean {
  return useAuthStore((s) => sessionAllows(s.user, 'routing:write'))
}

/**
 * The skew to keep for `token`: measured afresh for a new token, kept for the one already held (the
 * `/auth/me` refresh re-sets the same token long after it arrived, and re-measuring then would
 * understate the skew).
 */
function skewFor(token: string, current: { token: string | null; tokenClockSkew: number | null }) {
  return token === current.token ? current.tokenClockSkew : measureClockSkew(token)
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set, get) => ({
      token: null,
      tokenClockSkew: null,
      user: null,
      merchants: [],
      hasHydrated: false,
      setAuth: (token, user, merchants = []) => {
        const tokenClockSkew = skewFor(token, get())
        tokenRef.set(token, tokenClockSkew)
        set({ token, tokenClockSkew, user, merchants })
      },
      updateMerchant: (token, merchantId, merchants) => {
        const tokenClockSkew = skewFor(token, get())
        tokenRef.set(token, tokenClockSkew)
        set((state) => ({
          token,
          tokenClockSkew,
          merchants,
          user: state.user ? { ...state.user, merchantId } : null,
        }))
      },
      clearAuth: () => {
        tokenRef.set(null)
        set({ token: null, tokenClockSkew: null, user: null, merchants: [] })
        clearDashboardHandoff()
      },
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
    }),
    {
      name: 'auth-store',
      onRehydrateStorage: () => (state) => {
        if (state?.token) {
          tokenRef.restore(state.token, state.tokenClockSkew ?? null)
        }
        state?.setHasHydrated(true)
      },
    }
  )
)
