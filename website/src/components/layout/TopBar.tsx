import { useState, useEffect, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'
import { Moon, Sun, LogOut, ChevronDown, Building2, Check, Plus } from 'lucide-react'

interface SwitchMerchantResponse {
  token: string
  merchant_id: string
  role: string
  merchants: { merchant_id: string; merchant_name: string; role: string }[]
}

export function TopBar() {
  const navigate = useNavigate()
  const { user, merchants, clearAuth, updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const [isDark, setIsDark] = useState(() => localStorage.getItem('theme') === 'dark')
  const [merchantOpen, setMerchantOpen] = useState(false)
  const [switching, setSwitching] = useState<string | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

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
    function handleClickOutside(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setMerchantOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  async function handleLogout() {
    try {
      await apiFetch('/auth/logout', { method: 'POST' })
    } catch {
      // best-effort - clear locally regardless
    }
    clearAuth()
    navigate('/login', { replace: true })
  }

  async function handleSwitchMerchant(merchantId: string) {
    if (merchantId === user?.merchantId || switching) return
    setSwitching(merchantId)
    try {
      const res = await apiFetch<SwitchMerchantResponse>('/auth/switch-merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_id: merchantId }),
      })
      updateMerchant(res.token, res.merchant_id, res.merchants)
      setMerchantId(res.merchant_id)
      setMerchantOpen(false)
      window.location.reload()
    } catch {
      // ignore
    } finally {
      setSwitching(null)
    }
  }

  const currentMerchant = merchants.find((m) => m.merchant_id === user?.merchantId)
  const initials = user?.email ? user.email.slice(0, 2).toUpperCase() : 'ME'

  return (
    <header className="h-14 bg-white dark:bg-[#0c0c10] border-b border-[#e6e6ee] dark:border-[#1a1a24] flex items-center justify-between px-6 shrink-0 relative z-10">
      <div />

      <div className="flex items-center gap-2">
        {/* Merchant switcher */}
        {merchants.length > 0 && (
          <div className="relative" ref={dropdownRef}>
            <button
              onClick={() => setMerchantOpen((v) => !v)}
              className="flex items-center gap-2 h-8 px-3 rounded-lg border border-[#e6e6ee] dark:border-[#1a1a24] bg-white dark:bg-[#121218] hover:bg-slate-50 dark:hover:bg-[#18181f] transition-colors text-slate-700 dark:text-slate-300"
            >
              <Building2 size={13} className="text-slate-400 shrink-0" />
              <span className="text-[12px] font-medium max-w-[140px] truncate">
                {currentMerchant?.merchant_name ?? user?.merchantId ?? 'Select merchant'}
              </span>
              <ChevronDown size={12} className="text-slate-400 shrink-0" />
            </button>

            {merchantOpen && (
              <div className="absolute right-0 top-10 w-60 bg-white dark:bg-[#0c0c10] border border-[#e6e6ee] dark:border-[#1a1a24] rounded-lg shadow-lg py-1 z-50">
                <p className="px-3 py-1.5 text-[10px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500">
                  Merchants
                </p>
                {merchants.map((m) => (
                  <button
                    key={m.merchant_id}
                    onClick={() => handleSwitchMerchant(m.merchant_id)}
                    disabled={switching === m.merchant_id}
                    className="w-full flex items-center gap-2.5 px-3 py-2 hover:bg-slate-50 dark:hover:bg-[#13131a] transition-colors text-left"
                  >
                    <div className="w-6 h-6 rounded-md bg-brand-50 flex items-center justify-center shrink-0">
                      <Building2 size={12} className="text-brand-600" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-[13px] font-medium text-slate-700 dark:text-slate-300 truncate">
                        {m.merchant_name}
                      </p>
                      <p className="text-[11px] text-slate-400 truncate">{m.merchant_id}</p>
                    </div>
                    {m.merchant_id === user?.merchantId && (
                      <Check size={13} className="text-brand-600 shrink-0" />
                    )}
                  </button>
                ))}
                <div className="border-t border-[#e6e6ee] dark:border-[#1a1a24] mt-1 pt-1">
                  <button
                    onClick={() => { setMerchantOpen(false); navigate('/onboarding') }}
                    className="w-full flex items-center gap-2.5 px-3 py-2 hover:bg-slate-50 dark:hover:bg-[#13131a] transition-colors text-left text-brand-600"
                  >
                    <Plus size={13} />
                    <span className="text-[13px] font-medium">Add merchant</span>
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Theme toggle */}
        <button
          onClick={() => setIsDark(!isDark)}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-[#1a1a24] transition-colors"
          aria-label="Toggle theme"
        >
          {isDark ? <Sun size={16} /> : <Moon size={16} />}
        </button>

        <div className="w-px h-5 bg-[#e6e6ee] dark:bg-[#1a1a24] mx-1" />

        {user && (
          <div className="flex items-center gap-2 pl-1">
            <div className="w-7 h-7 rounded-full bg-brand-600 flex items-center justify-center">
              <span className="text-[10px] font-semibold text-white">{initials}</span>
            </div>
            <div className="hidden sm:block">
              <p className="text-[13px] font-medium text-slate-700 dark:text-slate-300 leading-tight">
                {user.email}
              </p>
            </div>
          </div>
        )}

        <button
          onClick={handleLogout}
          className="w-8 h-8 flex items-center justify-center rounded-lg text-slate-500 hover:bg-red-50 hover:text-red-500 dark:text-slate-400 dark:hover:bg-red-950/30 dark:hover:text-red-400 transition-colors ml-1"
          aria-label="Sign out"
          title="Sign out"
        >
          <LogOut size={16} />
        </button>
      </div>
    </header>
  )
}
