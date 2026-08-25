import { useState, useEffect, useRef } from 'react'
import { useAuthStore } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'
import { ChevronDown, ShieldCheck, ArrowRight } from 'lucide-react'
import { NotificationBell } from './NotificationBell'
import { GlobalSearch } from './GlobalSearch'
import { ScopeSwitcher } from './ScopeSwitcher'

interface EnterMerchantResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
}

interface LookupMember {
  email: string
  role: string
}

interface LookupResult {
  merchant_id: string
  merchant_name: string
  members: LookupMember[]
  hs_merchant_name?: string
  hs_org_name?: string
}

export function TopBar() {
  const { user, setAuth } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const [superAdminOpen, setSuperAdminOpen] = useState(false)
  const [enterId, setEnterId] = useState('')
  const [entering, setEntering] = useState(false)
  const [enterError, setEnterError] = useState<string | null>(null)
  const [lookupQuery, setLookupQuery] = useState('')
  const [lookupResults, setLookupResults] = useState<LookupResult[]>([])
  const [searching, setSearching] = useState(false)
  const superAdminRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (superAdminRef.current && !superAdminRef.current.contains(e.target as Node)) {
        setSuperAdminOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  // Debounced lookup by email or merchant name while the popover is open.
  useEffect(() => {
    if (!superAdminOpen) return
    const q = lookupQuery.trim()
    if (!q) { setLookupResults([]); setSearching(false); return }
    let cancelled = false
    setSearching(true)
    const timer = setTimeout(async () => {
      try {
        const res = await apiFetch<LookupResult[]>('/auth/super-admin/lookup', {
          method: 'POST',
          body: JSON.stringify({ query: q }),
        })
        if (!cancelled) setLookupResults(res)
      } catch {
        if (!cancelled) setLookupResults([])
      } finally {
        if (!cancelled) setSearching(false)
      }
    }, 300)
    return () => { cancelled = true; clearTimeout(timer) }
  }, [lookupQuery, superAdminOpen])

  async function handleEnterMerchant(merchantId: string) {
    if (!merchantId || entering) return
    setEntering(true)
    setEnterError(null)
    try {
      const res = await apiFetch<EnterMerchantResponse>('/auth/super-admin/enter-merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_id: merchantId }),
      })
      // Optimistically flag the session as a super-admin view; AuthGuard's /auth/me revalidation
      // (triggered by the token change) confirms it.
      setAuth(
        res.token,
        {
          userId: res.user_id,
          email: res.email,
          merchantId: res.merchant_id,
          role: res.role,
          isRedirectSession: false,
          isSuperAdmin: true,
          isSuperAdminView: true,
        },
        [],
      )
      setMerchantId(res.merchant_id)
      setSuperAdminOpen(false)
      setEnterId('')
      setLookupQuery('')
      setLookupResults([])
    } catch (err) {
      const status = (err as { status?: number }).status
      setEnterError(status === 404 ? 'No merchant with that ID.' : 'Could not enter that merchant.')
    } finally {
      setEntering(false)
    }
  }

  return (
    <header className="flex h-[78px] shrink-0 items-center gap-4 border-b border-slate-200 bg-white px-6 transition-colors duration-300 dark:border-[#22262f] dark:bg-[#06080d] relative z-10">
      <div className="flex-1" />

      {/* The header sits right of the 256px (w-64) sidebar, so centering within
          it lands 128px right of the screen center. Shift left by half the
          sidebar (-translate-x-32 = -8rem = -128px) to center against the
          viewport — i.e. the user's screen — not just the content area. */}
      <div className="flex min-w-0 flex-[2] justify-center lg:-translate-x-32">
        <GlobalSearch />
      </div>

      <div className="flex flex-1 items-center justify-end gap-2">
        <NotificationBell />

        {/* Super-admin: enter any merchant by ID. Hidden inside a view session — Exit first (via the
            banner) so the single-level session model stays intact. */}
        {user?.isSuperAdmin && !user?.isSuperAdminView && (
          <div className="relative" ref={superAdminRef}>
            <button
              onClick={() => setSuperAdminOpen((v) => !v)}
              className="flex items-center gap-2 h-8 px-3 rounded-lg border border-amber-300/70 dark:border-amber-500/30 bg-amber-50 dark:bg-amber-500/10 hover:bg-amber-100 dark:hover:bg-amber-500/20 transition-colors text-amber-700 dark:text-amber-300"
            >
              <ShieldCheck size={13} className="shrink-0" />
              <span className="text-[12px] font-medium">Super admin</span>
              <ChevronDown size={12} className="shrink-0" />
            </button>

            {superAdminOpen && (
              <div className="absolute right-0 top-10 w-80 bg-white dark:bg-[#0c0c10] border border-[#e6e6ee] dark:border-[#1a1a24] rounded-lg shadow-lg p-3 z-50">
                <p className="text-[10px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500 mb-2">
                  View a merchant dashboard
                </p>

                {/* Search by person or company */}
                <input
                  value={lookupQuery}
                  onChange={(e) => setLookupQuery(e.target.value)}
                  placeholder="Search by email or merchant name"
                  autoFocus
                  className="w-full h-8 px-2.5 rounded-md border border-[#e6e6ee] dark:border-[#1a1a24] bg-white dark:bg-[#121218] text-[13px] text-slate-700 dark:text-slate-200 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-amber-400/40"
                />

                {lookupQuery.trim() && (
                  <div className="mt-2 max-h-64 overflow-y-auto">
                    {searching && lookupResults.length === 0 ? (
                      <p className="px-1 py-2 text-[12px] text-slate-400">Searching…</p>
                    ) : lookupResults.length === 0 ? (
                      <p className="px-1 py-2 text-[12px] text-slate-400">No matches.</p>
                    ) : (
                      lookupResults.map((m) => (
                        <button
                          key={m.merchant_id}
                          onClick={() => handleEnterMerchant(m.merchant_id)}
                          disabled={entering}
                          className="w-full text-left px-2 py-2 rounded-md hover:bg-slate-50 dark:hover:bg-[#13131a] transition-colors disabled:opacity-50"
                        >
                          {/* Where this scope sits, above its own name: a search for "default"
                              returns one row per merchant, and the parent is the only thing
                              telling them apart. */}
                          {(m.hs_org_name || m.hs_merchant_name) && (
                            <p className="text-[10.5px] text-slate-400 truncate">
                              {[m.hs_org_name, m.hs_merchant_name].filter(Boolean).join(' › ')}
                            </p>
                          )}
                          <p className="text-[13px] font-medium text-slate-700 dark:text-slate-200 truncate">
                            {m.merchant_name}
                          </p>
                          <p className="text-[11px] text-slate-400 truncate">{m.merchant_id}</p>
                          {m.members.length > 0 && (
                            <p className="text-[11px] text-slate-400 truncate">
                              {m.members.map((mem) => mem.email).join(', ')}
                            </p>
                          )}
                        </button>
                      ))
                    )}
                  </div>
                )}

                {/* Fallback: enter an exact ID */}
                <div className="mt-2 pt-2 border-t border-[#e6e6ee] dark:border-[#1a1a24]">
                  <input
                    value={enterId}
                    onChange={(e) => { setEnterId(e.target.value); setEnterError(null) }}
                    onKeyDown={(e) => { if (e.key === 'Enter') handleEnterMerchant(enterId.trim()) }}
                    placeholder="…or paste an exact merchant ID"
                    className="w-full h-8 px-2.5 rounded-md border border-[#e6e6ee] dark:border-[#1a1a24] bg-white dark:bg-[#121218] text-[13px] text-slate-700 dark:text-slate-200 placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-amber-400/40"
                  />
                  {enterError && (
                    <p className="mt-1.5 text-[11px] text-red-500">{enterError}</p>
                  )}
                  <button
                    onClick={() => handleEnterMerchant(enterId.trim())}
                    disabled={!enterId.trim() || entering}
                    className="mt-2 w-full flex items-center justify-center gap-1.5 h-8 rounded-md bg-amber-600 hover:bg-amber-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-[13px] font-medium text-white"
                  >
                    {entering ? 'Entering…' : (<>Enter dashboard <ArrowRight size={13} /></>)}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        <ScopeSwitcher />
      </div>
    </header>
  )
}
