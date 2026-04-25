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
  const setAuth = useAuthStore((s) => s.setAuth)
  const clearAuth = useAuthStore((s) => s.clearAuth)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)
  const [status, setStatus] = useState<'checking' | 'ready' | 'invalid'>(
    token ? 'checking' : 'invalid'
  )

  useEffect(() => {
    let cancelled = false

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
      .catch(() => {
        if (cancelled) return
        clearAuth()
        setMerchantId('')
        setStatus('invalid')
      })

    return () => {
      cancelled = true
    }
  }, [token, setAuth, clearAuth, setMerchantId])

  if (!token) return <Navigate to="/login" replace />
  if (status === 'checking') {
    return (
      <div className="dark flex min-h-screen items-center justify-center bg-[#030507] text-white">
        <div className="flex items-center gap-3 rounded-2xl border border-[#1d1d23] bg-[#111318] px-5 py-4 text-sm text-[#c7cfdb]">
          <Loader2 size={16} className="animate-spin text-[#7ea4ff]" />
          Validating session
        </div>
      </div>
    )
  }
  if (status === 'invalid') return <Navigate to="/login" replace />
  return <Outlet />
}
