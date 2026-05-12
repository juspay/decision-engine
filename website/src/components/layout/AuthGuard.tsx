import { useEffect } from 'react'
import { Navigate, Outlet } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import useSWR from 'swr'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { fetcher } from '../../lib/api'

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

export function AuthGuard() {
  const token = useAuthStore((s) => s.token)
  const hasHydrated = useAuthStore((s) => s.hasHydrated)
  const setAuth = useAuthStore((s) => s.setAuth)
  const clearAuth = useAuthStore((s) => s.clearAuth)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)

  const { data: me, error, isLoading } = useSWR<MeResponse>(
    token && hasHydrated ? '/auth/me' : null,
    fetcher,
    { revalidateOnFocus: false, shouldRetryOnError: false },
  )

  useEffect(() => {
    if (!me || !token) return
    const activeMerchantId = me.merchant_id || me.merchants[0]?.merchant_id || ''
    setAuth(token, { userId: me.user_id, email: me.email, merchantId: activeMerchantId, role: me.role }, me.merchants)
    setMerchantId(activeMerchantId)
  }, [me, token, setAuth, setMerchantId])

  useEffect(() => {
    if (!error) return
    const statusCode = (error as { status?: number }).status
    if (statusCode === 401 || statusCode === 403) {
      clearAuth()
      setMerchantId('')
    }
  }, [error, clearAuth, setMerchantId])

  if (!hasHydrated) return <SessionSpinner label="Restoring session" />
  if (!token) return <Navigate to="/login" replace />
  if (isLoading) return <SessionSpinner label="Validating session" />

  if (error) {
    const statusCode = (error as { status?: number }).status
    if (statusCode === 401 || statusCode === 403) return <Navigate to="/login" replace />
    // Transient failure (network/5xx): keep the session, let the user through.
  }

  return <Outlet />
}
