import { useEffect, useRef, useState } from 'react'
import { Navigate, Outlet } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import useSWR from 'swr'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { fetcher } from '../../lib/api'
import { refreshSessionScopedSWRCache } from '../../lib/swrCache'

interface MeResponse {
  user_id: string
  email: string
  merchant_id: string
  role: string
  email_verified: boolean
  merchants: Array<{
    merchant_id: string
    merchant_name: string
    role: string
  }>
}

function SessionSpinner({ label }: { label: string }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="flex items-center gap-3 rounded-2xl border border-slate-200 bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_16px_40px_-30px_rgba(15,23,42,0.35)] dark:border-[#1d1d23] dark:bg-[#111318] dark:text-[#c7cfdb] dark:shadow-none">
        <Loader2 size={16} className="animate-spin text-brand-600 dark:text-[#7ea4ff]" />
        {label}
      </div>
    </div>
  )
}

// An HS-redirect (SSO) session has a synthetic user with no password, so the DE login page is a
// dead end once its short-lived token expires. Send the user back to Hyperswitch instead, where a
// fresh session is minted on demand.
function RedirectSessionExpired() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-white px-4 text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="flex max-w-sm flex-col items-center gap-3 rounded-2xl border border-slate-200 bg-white px-6 py-6 text-center shadow-[0_16px_40px_-30px_rgba(15,23,42,0.35)] dark:border-[#1d1d23] dark:bg-[#111318] dark:shadow-none">
        <p className="text-sm font-medium text-slate-900 dark:text-white">Your routing session has expired</p>
        <p className="text-sm text-slate-600 dark:text-[#c7cfdb]">
          Reopen it from the Hyperswitch dashboard to continue.
        </p>
        <button
          type="button"
          onClick={() => window.close()}
          className="mt-1 rounded-lg bg-brand-600 px-4 py-2 text-sm font-medium text-white hover:bg-brand-700 dark:bg-[#2d4b8f] dark:hover:bg-[#33569f]"
        >
          Close tab
        </button>
      </div>
    </div>
  )
}

export function AuthGuard() {
  const token = useAuthStore((s) => s.token)
  const user = useAuthStore((s) => s.user)
  const hasHydrated = useAuthStore((s) => s.hasHydrated)
  const setAuth = useAuthStore((s) => s.setAuth)
  const clearAuth = useAuthStore((s) => s.clearAuth)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)
  const previousTokenRef = useRef<string | null>(null)
  // A redirect (HS SSO) session that 401s can't recover on the DE login page — remember it so we
  // show the "reopen from Hyperswitch" screen instead of bouncing to /login.
  const [redirectSessionExpired, setRedirectSessionExpired] = useState(false)

  const { data: me, error, isValidating } = useSWR<MeResponse>(
    token && hasHydrated ? ['/auth/me', token] : null,
    ([url]) => fetcher(url),
    { revalidateOnFocus: false, shouldRetryOnError: false },
  )

  useEffect(() => {
    if (!hasHydrated || !token || previousTokenRef.current === token) return
    previousTokenRef.current = token
    refreshSessionScopedSWRCache()
  }, [hasHydrated, token])

  useEffect(() => {
    if (!me || !token) return
    const activeMerchantId = me.merchant_id || me.merchants[0]?.merchant_id || ''
    setAuth(token, { userId: me.user_id, email: me.email, merchantId: activeMerchantId, role: me.role, isRedirectSession: me.user_id.startsWith('hs_') }, me.merchants)
    setMerchantId(activeMerchantId)
  }, [me, token, setAuth, setMerchantId])

  useEffect(() => {
    // Don't clear auth while SWR is revalidating — the stale error may be from a previous
    // session and the fresh request (with the new token) could still succeed.
    if (!error || isValidating) return
    const statusCode = (error as { status?: number }).status
    if (statusCode === 401 || statusCode === 403) {
      if (user?.isRedirectSession) setRedirectSessionExpired(true)
      clearAuth()
      setMerchantId('')
    }
  }, [error, isValidating, clearAuth, setMerchantId, user])

  if (!hasHydrated) return <SessionSpinner label="Restoring session" />
  if (redirectSessionExpired) return <RedirectSessionExpired />
  if (!token) return <Navigate to="/login" replace />
  if (!me && !error) return <SessionSpinner label="Validating session" />

  if (error) {
    const statusCode = (error as { status?: number }).status
    if (statusCode === 401 || statusCode === 403) {
      // Wait for the in-flight revalidation before rejecting — the stale error may be from
      // a prior expired session and the current token could be valid.
      if (isValidating) return <SessionSpinner label="Validating session" />
      // A redirect session can't recover on /login (synthetic user, no password) — the effect
      // above also latches this, but guard here so the first render doesn't flash /login.
      if (user?.isRedirectSession) return <RedirectSessionExpired />
      return <Navigate to="/login" replace />
    }
    // Transient failure (network/5xx): keep the session, let the user through.
  }

  return <Outlet />
}
