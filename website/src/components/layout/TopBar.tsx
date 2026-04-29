import { useState, useEffect, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'
import { ChevronDown, Building2, Check, Plus } from 'lucide-react'

interface SwitchMerchantResponse {
  token: string
  merchant_id: string
  role: string
  merchants: { merchant_id: string; merchant_name: string; role: string }[]
}

export function TopBar() {
  const navigate = useNavigate()
  const { user, merchants, updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const [merchantOpen, setMerchantOpen] = useState(false)
  const [switching, setSwitching] = useState<string | null>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setMerchantOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

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

  return (
    <header className="flex h-[78px] shrink-0 items-center justify-between border-b border-slate-200 bg-white px-6 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#06080d] relative z-10">
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
      </div>
    </header>
  )
}
