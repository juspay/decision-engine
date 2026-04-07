import { useState } from 'react'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { Building2, Search, Loader2 } from 'lucide-react'

export function TopBar() {
  const { merchantId, setMerchantId } = useMerchantStore()
  const [draft, setDraft] = useState(merchantId)
  const [creating, setCreating] = useState(false)

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
    <header className="h-[72px] bg-black border-b border-[#151515] flex items-center justify-between px-8 shrink-0 relative z-10">
      <div />
      <div className="flex items-center gap-4">
        <div className="relative">
          <input
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSetMerchant()}
            placeholder="Merchant ID"
            className="w-64 bg-[#0f0f11] border border-[#222222] rounded-full px-4 py-2 text-sm text-white placeholder-[#66666e] focus:outline-none focus:border-[#444444] transition-colors"
          />
          <button
            onClick={handleSetMerchant}
            disabled={creating}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 text-[#66666e] hover:text-white transition-colors"
          >
            {creating ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}
          </button>
        </div>

        {merchantId && (
          <div className="flex items-center gap-2 pl-4 ml-2 border-l border-[#222222]">
            <Building2 size={16} className="text-[#66666e]" />
            <span className="text-sm text-white font-medium">
              {merchantId}
            </span>
          </div>
        )}
      </div>
    </header>
  )
}