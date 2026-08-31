import { useState } from 'react'
import { ShieldCheck, LogOut } from 'lucide-react'
import { useAuthStore, type MerchantInfo } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'

interface ExitResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
  merchants: MerchantInfo[]
}

// Shown only during a super-admin view session. Makes it unmistakable that the dashboard belongs to
// another merchant, and offers the one way back to the admin's own session.
export function SuperAdminBanner() {
  const { user, setAuth } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const [exiting, setExiting] = useState(false)

  if (!user?.isSuperAdminView) return null

  // The names the admin searched by, not the scope id they had to paste — the id is still one hover
  // away, and stays the label for a scope with no ancestry to name.
  const scopeLabel = [user.hierarchy?.hs_merchant_name, user.hierarchy?.profile_name]
    .filter(Boolean)
    .join(' / ') || user.merchantId

  async function handleExit() {
    if (exiting) return
    setExiting(true)
    try {
      const res = await apiFetch<ExitResponse>('/auth/super-admin/exit', { method: 'POST' })
      setAuth(
        res.token,
        {
          userId: res.user_id,
          email: res.email,
          merchantId: res.merchant_id,
          role: res.role,
          isRedirectSession: false,
          isSuperAdmin: true,
          isSuperAdminView: false,
        },
        res.merchants,
      )
      setMerchantId(res.merchant_id)
    } catch {
      // Leave the user in the view session; they can retry Exit.
    } finally {
      setExiting(false)
    }
  }

  return (
    <div
      title={`Scope id  ${user.merchantId}`}
      className="flex h-9 shrink-0 items-center justify-center gap-3 bg-amber-500 px-4 text-[12.5px] font-medium text-amber-950 leading-[17px]"
    >
      <span className="flex min-w-0 items-center gap-1.5">
        <ShieldCheck size={14} className="shrink-0" />
        Viewing
        <span className="min-w-0 truncate font-semibold">{scopeLabel}</span>
        as platform super admin
      </span>
      <button
        onClick={handleExit}
        disabled={exiting}
        className="flex items-center gap-1.5 rounded-md bg-amber-950/15 px-2.5 py-1 hover:bg-amber-950/25 disabled:opacity-60 transition-colors"
      >
        <LogOut size={12} />
        {exiting ? 'Exiting…' : 'Exit'}
      </button>
    </div>
  )
}
