import { useState, useEffect } from 'react'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { Building2, ArrowRight, Loader2, Moon, Sun } from 'lucide-react'

export function TopBar() {
  const { merchantId, setMerchantId } = useMerchantStore()
  const [draft, setDraft] = useState(merchantId)
  const [creating, setCreating] = useState(false)

  // Theme management
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
    <header className="relative z-10 flex h-[78px] shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#06080d] md:px-8">
      <div />
      <div className="flex items-center gap-6">
        <div className="relative">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSetMerchant()}
            placeholder="Set Merchant ID"
            className="w-72 rounded-full border border-slate-200 bg-white px-4 py-2 text-sm text-slate-900 shadow-[0_6px_20px_-16px_rgba(15,23,42,0.18)] transition-all placeholder-slate-400 focus:outline-none focus:border-[#3b82f6]/30 dark:border-[#22262f] dark:bg-[#11141b] dark:text-white dark:placeholder-[#6c7486] dark:shadow-none"
          />
          <button
            onClick={handleSetMerchant}
            disabled={creating}
            className="absolute right-2 top-1/2 -translate-y-1/2 rounded-full p-2 text-slate-400 transition-colors hover:text-slate-700 dark:text-[#7a8397] dark:hover:text-[#dbe7ff]"
          >
            {creating ? <Loader2 size={16} className="animate-spin" /> : <ArrowRight size={16} />}
          </button>
        </div>

        {merchantId && (
          <div className="ml-2 flex items-center gap-2 border-l border-slate-200 pl-6 transition-colors duration-300 dark:border-[#22262f]">
            <Building2 size={16} className="text-brand-500 dark:text-sky-300" />
            <span className="text-sm font-medium text-slate-800 dark:text-white">
              {merchantId}
            </span>
          </div>
        )}

        {/* Theme Toggle */}
        <button
          onClick={() => setIsDark(!isDark)}
          className="rounded-full border border-slate-200 bg-white p-2.5 text-slate-600 shadow-[0_6px_20px_-16px_rgba(15,23,42,0.18)] transition-colors duration-200 hover:bg-slate-50 hover:text-slate-900 dark:border-[#22262f] dark:bg-[#11141b] dark:text-[#aeb6c7] dark:shadow-none dark:hover:bg-[#171b23] dark:hover:text-white"
          aria-label="Toggle theme"
        >
          {isDark ? <Sun size={18} /> : <Moon size={18} />}
        </button>
      </div>
    </header>
  )
}
