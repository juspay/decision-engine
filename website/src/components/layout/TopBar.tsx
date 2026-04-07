import { useState } from 'react'
import { useMerchantStore } from '../../store/merchantStore'
import { Building2 } from 'lucide-react'

export function TopBar() {
  const { merchantId, setMerchantId } = useMerchantStore()
  const [draft, setDraft] = useState(merchantId)

  return (
    <header className="h-14 bg-[#060609] border-b border-[#14141c] flex items-center justify-between px-6 shrink-0">
      <div />
      <div className="flex items-center gap-3">
        <Building2 size={14} className="text-gray-500" />
        <span className="text-xs font-medium text-gray-500 tracking-wide uppercase">Merchant</span>
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && setMerchantId(draft.trim())}
          placeholder="merchant_id"
          className="border rounded-lg px-3 py-1.5 text-xs w-44 font-mono"
        />
        <button
          onClick={() => setMerchantId(draft.trim())}
          className="px-3 py-1.5 bg-brand-500 text-white rounded-lg text-xs font-semibold tracking-wide hover:bg-brand-600 transition-colors shadow-[0_0_12px_rgba(22,104,227,0.3)] hover:shadow-[0_0_16px_rgba(22,104,227,0.4)]"
        >
          Set
        </button>
        {merchantId && (
          <div className="flex items-center gap-1.5 pl-2 border-l border-[#1c1c24]">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.6)]" />
            <span className="text-xs text-emerald-400 font-mono">{merchantId}</span>
          </div>
        )}
      </div>
    </header>
  )
}
