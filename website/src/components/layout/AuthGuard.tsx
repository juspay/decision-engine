import { useEffect, useState } from 'react'
import { Navigate, Outlet } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'

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

export function AuthGuard() {
  const token = useAuthStore((s) => s.token)
  const hasHydrated = useAuthStore((s) => s.hasHydrated)
  const setAuth = useAuthStore((s) => s.setAuth)
  const clearAuth = useAuthStore((s) => s.clearAuth)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)
  const [status, setStatus] = useState<'checking' | 'ready' | 'invalid'>(
    token ? 'checking' : 'invalid'
  )

  useEffect(() => {
    let cancelled = false

    if (!hasHydrated) {
      setStatus('checking')
      return
    }

    if (!token) {
      setStatus('invalid')
      return
    }

    setStatus('checking')

    apiFetch<MeResponse>('/auth/me')
      .then((me) => {
        if (cancelled) return

        const activeMerchantId =
          me.merchant_id || me.merchants[0]?.merchant_id || ''

        setAuth(
          token,
          {
            userId: me.user_id,
            email: me.email,
            merchantId: activeMerchantId,
            role: me.role,
          },
          me.merchants
        )
        setMerchantId(activeMerchantId)
        setStatus('ready')
      })
      .catch((error: unknown) => {
        if (cancelled) return
        const statusCode = typeof error === 'object' && error ? (error as { status?: number }).status : undefined

        if (statusCode === 401 || statusCode === 403) {
          clearAuth()
          setMerchantId('')
          setStatus('invalid')
          return
        }

        // Keep the local session on transient backend/network failures. A refresh
        // should not log the operator out unless the token is actually rejected.
        setStatus('ready')
      })

    return () => {
      cancelled = true
    }
  }, [hasHydrated, token, setAuth, clearAuth, setMerchantId])

  if (!hasHydrated) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
        <div className="flex items-center gap-3 rounded-2xl border border-slate-200 bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_16px_40px_-30px_rgba(15,23,42,0.35)] dark:border-[#1d1d23] dark:bg-[#111318] dark:text-[#c7cfdb] dark:shadow-none">
          <Loader2 size={16} className="animate-spin text-brand-600 dark:text-[#7ea4ff]" />
          Restoring session
        </div>
      </div>
    )
  }
  if (!token) return <Navigate to="/login" replace />
  if (status === 'checking') {
    return (
      <div className="flex min-h-screen items-center justify-center bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
        <div className="flex items-center gap-3 rounded-2xl border border-slate-200 bg-white px-5 py-4 text-sm text-slate-600 shadow-[0_16px_40px_-30px_rgba(15,23,42,0.35)] dark:border-[#1d1d23] dark:bg-[#111318] dark:text-[#c7cfdb] dark:shadow-none">
          <Loader2 size={16} className="animate-spin text-brand-600 dark:text-[#7ea4ff]" />
          Validating session
        </div>
      </div>
    )
  }
  if (status === 'invalid') return <Navigate to="/login" replace />
  return <Outlet />
}
