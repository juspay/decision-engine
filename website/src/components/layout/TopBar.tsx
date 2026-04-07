import { useState, useEffect } from 'react'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { Building2, ArrowRight, Loader2, Moon, Sun } from 'lucide-react'

export function TopBar() {
  const { merchantId, setMerchantId } = useMerchantStore()
  const [draft, setDraft] = useState(merchantId)
  const [creating, setCreating] = useState(false)

  // Theme management
  const [isDark, setIsDark] = useState(() => {
    return localStorage.getItem('theme') === 'dark'
  })

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

  return (
    <header className="h-[76px] bg-white dark:bg-black border-b border-slate-200 dark:border-[#151515] flex items-center justify-between px-8 shrink-0 relative z-10 transition-colors duration-300">
      <div />
      <div className="flex items-center gap-6">
        <div className="relative">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSetMerchant()}
            placeholder="Set Merchant ID"
            className="w-72 bg-slate-50 dark:bg-[#0f0f11] border border-slate-200 dark:border-[#222222] rounded-full px-4 py-2 text-sm text-slate-900 dark:text-white placeholder-slate-400 dark:placeholder-[#66666e] focus:outline-none focus:border-slate-400 dark:focus:border-[#444444] transition-colors"
          />
          <button
            onClick={handleSetMerchant}
            disabled={creating}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-2 text-slate-400 hover:text-brand-500 dark:text-[#66666e] dark:hover:text-white transition-colors"
          >
            {creating ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
          </button>
        </div>

        {merchantId && (
          <div className="flex items-center gap-2 pl-6 ml-2 border-l border-slate-200 dark:border-[#222222] transition-colors duration-300">
            <Building2 size={16} className="text-brand-500 dark:text-[#66666e]" />
            <span className="text-sm text-slate-800 dark:text-white font-medium">
              {merchantId}
            </span>
          </div>
        )}

        {/* Theme Toggle */}
        <button
          onClick={() => setIsDark(!isDark)}
          className="p-2.5 rounded-full bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-[#151515] dark:text-[#a1a1aa] dark:hover:text-white dark:hover:bg-[#222222] transition-colors duration-200"
          aria-label="Toggle theme"
        >
          {isDark ? <Sun size={18} /> : <Moon size={18} />}
        </button>
      </div>
    </header>
  )
}