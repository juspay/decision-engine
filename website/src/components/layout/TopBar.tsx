import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiFetch, apiPost } from '../../lib/api'
import {
  Building2,
  ArrowRight,
  Loader2,
  Moon,
  Sun,
  LogOut,
  ChevronDown,
} from 'lucide-react'

export function TopBar() {
  const navigate = useNavigate()
  const { user, clearAuth } = useAuthStore()
  const { merchantId, setMerchantId } = useMerchantStore()
  const [draft, setDraft] = useState(merchantId)
  const [creating, setCreating] = useState(false)
  const [isDark, setIsDark] = useState(() => localStorage.getItem('theme') === 'dark')

  useEffect(() => {
    const root = window.document.documentElement
    if (isDark) {
      root.classList.add('dark')
      localStorage.setItem('theme', 'dark')
    } else {
      root.classList.remove('dark')
      localStorage.setItem('theme', 'light')
    }
  }, [isDark])

  useEffect(() => {
    setDraft(merchantId)
  }, [merchantId])

  async function handleSetMerchant() {
    const id = draft.trim()
    if (!id) return

    setMerchantId(id)
    setCreating(true)

    try {
      await apiPost('/merchant-account/create', {
        merchant_id: id,
        gateway_success_rate_based_decider_input: null,
      })
    } catch {
      // Merchant may already exist - that's fine
    } finally {
      setCreating(false)
    }
  }

  async function handleLogout() {
    try {
      await apiFetch('/auth/logout', { method: 'POST' })
    } catch {
      // best-effort - clear locally regardless
    }
    clearAuth()
    navigate('/login', { replace: true })
  }

  const initials = user?.email ? user.email.slice(0, 2).toUpperCase() : 'ME'

  return (
    <header className="relative z-10 flex h-[78px] shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#06080d] md:px-8">
      <div className="flex items-center gap-6">
        <div className="relative">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSetMerchant()}
            placeholder="Set Merchant ID"
            className="w-72 rounded-full border border-slate-200 bg-white px-4 py-2 text-sm text-slate-900 shadow-[0_6px_20px_-16px_rgba(15,23,42,0.18)] transition-all placeholder-slate-400 focus:border-[#3b82f6]/30 focus:outline-none dark:border-[#22262f] dark:bg-[#11141b] dark:text-white dark:placeholder-[#6c7486] dark:shadow-none"
          />
          <button
            onClick={handleSetMerchant}
            disabled={creating}
            className="absolute right-2 top-1/2 -translate-y-1/2 rounded-full p-2 text-slate-400 transition-colors hover:text-slate-700 dark:text-[#7a8397] dark:hover:text-[#dbe7ff]"
          >
            {creating ? (
              <Loader2 size={16} className="animate-spin" />
            ) : (
              <ArrowRight size={16} />
            )}
          </button>
        </div>

        {merchantId && (
          <div className="flex items-center gap-2 border-l border-slate-200 pl-6 transition-colors duration-300 dark:border-[#22262f]">
            <Building2 size={16} className="text-brand-500 dark:text-sky-300" />
            <span className="text-sm font-medium text-slate-800 dark:text-white">{merchantId}</span>
          </div>
        )}
      </div>

      <div className="flex items-center gap-2">
        <button
          onClick={() => setIsDark(!isDark)}
          className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-[#1a1a24]"
          aria-label="Toggle theme"
        >
          {isDark ? <Sun size={16} /> : <Moon size={16} />}
        </button>

        <div className="mx-1 h-5 w-px bg-[#e6e6ee] dark:bg-[#1a1a24]" />

        {user && (
          <div className="flex items-center gap-2 pl-1">
            <div className="flex h-7 w-7 items-center justify-center rounded-full bg-brand-600">
              <span className="text-[10px] font-semibold text-white">{initials}</span>
            </div>
            <div className="hidden sm:block">
              <p className="text-[13px] font-medium leading-tight text-slate-700 dark:text-slate-300">
                {user.email}
              </p>
              <p className="text-[11px] leading-tight text-slate-400 dark:text-slate-500">
                {merchantId || user.merchantId}
              </p>
            </div>
            <ChevronDown size={14} className="ml-0.5 text-slate-400 dark:text-slate-500" />
          </div>
        )}

        <button
          onClick={handleLogout}
          className="ml-1 flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition-colors hover:bg-red-50 hover:text-red-500 dark:text-slate-400 dark:hover:bg-red-950/30 dark:hover:text-red-400"
          aria-label="Sign out"
          title="Sign out"
        >
          <LogOut size={16} />
        </button>
      </div>
    </header>
  )
}
