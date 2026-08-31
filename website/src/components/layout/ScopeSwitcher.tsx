import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Building2, Check, ChevronDown, ChevronRight, Layers, Plus, Search, Store } from 'lucide-react'
import { useAuthStore, type MerchantInfo } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiFetch } from '../../lib/api'
import { CopyButton } from '../ui/CopyButton'

interface SwitchMerchantResponse {
  token: string
  merchant_id: string
  role: string
  merchants: MerchantInfo[]
}

/** Above this many scopes, merchant groups start collapsed so the panel opens as a short list. */
const AUTO_EXPAND_LIMIT = 8

/** A scope's three levels, resolved with the fallbacks a partly-synced scope needs. */
interface ScopeLabels {
  org: string | null
  merchant: string | null
  profile: string
}

/**
 * What to call each level of one scope.
 *
 * Only `profile` is guaranteed: an unsynced scope has no ancestry to name, and the flat label the
 * backend built (or, failing that, the scope id) is the honest thing to show for it. The levels
 * above are `null` rather than a placeholder, so the layout can drop them instead of rendering
 * rows of "—".
 */
function labelsOf(scope: MerchantInfo): ScopeLabels {
  return {
    org: scope.hs_org_name ?? scope.hs_org_id ?? null,
    merchant: scope.hs_merchant_name ?? scope.hs_merchant_id ?? null,
    profile: scope.profile_name ?? scope.merchant_name ?? scope.merchant_id,
  }
}

/** A scope id shortened for a list row, where the full id is noise. */
function shortId(id: string): string {
  return id.length > 14 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id
}

/**
 * Group key for scopes with no ancestry to group by. Not a valid Hyperswitch id, so it can never
 * collide with a real merchant's own key.
 */
const UNGROUPED_KEY = 'ungrouped:no-ancestry'

interface MerchantGroup {
  key: string
  merchant: string | null
  org: string | null
  /** The org label is a raw Hyperswitch id, not a name — it is set in mono and never upper-cased. */
  orgIsId: boolean
  orgKey: string
  scopes: MerchantInfo[]
}

/**
 * The scopes arranged as the account tree they came from: orgs holding merchants holding profiles.
 *
 * Scopes with no ancestry collect under one unnamed group, which is what a DE-native session is
 * made of entirely — there the tree flattens back into the plain merchant list it always was.
 * Order follows the incoming list so the backend stays the one deciding it.
 */
function groupScopes(scopes: MerchantInfo[]): MerchantGroup[] {
  const groups = new Map<string, MerchantGroup>()

  for (const scope of scopes) {
    const labels = labelsOf(scope)
    const key = scope.hs_merchant_id ?? labels.merchant ?? UNGROUPED_KEY
    const existing = groups.get(key)
    if (existing) {
      existing.scopes.push(scope)
    } else {
      groups.set(key, {
        key,
        merchant: labels.merchant,
        org: labels.org,
        orgIsId: scope.hs_org_name == null,
        orgKey: scope.hs_org_id ?? labels.org ?? UNGROUPED_KEY,
        scopes: [scope],
      })
    }
  }

  return [...groups.values()]
}

/** Whether `scope` matches the search text, across every level and the raw ids. */
function matches(scope: MerchantInfo, query: string): boolean {
  const labels = labelsOf(scope)
  return [
    labels.profile,
    labels.merchant,
    labels.org,
    scope.merchant_id,
    scope.merchant_name,
  ].some((field) => field?.toLowerCase().includes(query))
}

/**
 * The account-scope picker: which org → merchant → profile the dashboard is configuring, and the
 * way to move to another one.
 *
 * The three levels are laid out as the tree they are — org and merchant as headers, profiles as the
 * rows — rather than flattened into one label per row, because an org grant repeats the same
 * merchant name on every row and truncates the profile name that is the only part differing between
 * them. A level shared by every scope in the list is stated once above the list instead of repeated
 * on every row, so the panel grows structure only as the account does. Search and collapsed groups are what make a large org
 * navigable; both are inert on a small account, where the whole list is already visible.
 */
export function ScopeSwitcher() {
  const navigate = useNavigate()
  const { user, merchants, updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [switching, setSwitching] = useState<string | null>(null)
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  // The keyboard cursor is held as a scope id, not a row number: folding a group or narrowing the
  // search renumbers every row, and an index would silently come to mean a different one.
  const [activeId, setActiveId] = useState<string | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  // A granted session's list is the scopes it was handed — a handed-over one from Hyperswitch, a
  // super-admin view from the organization it entered — and always includes the one it is already
  // on. A single entry therefore means there is nothing to switch between; a DE session's list
  // works the other way, where one membership is still a switcher.
  const isGranted = Boolean(user?.isRedirectSession || user?.isSuperAdminView)
  const canSwitch = merchants.length > (isGranted ? 1 : 0)

  // Hyperswitch creates the profiles a handed-over session can reach, so onboarding one here would
  // produce a scope its own account tree does not know about.
  const canAddMerchant = !user?.isRedirectSession && !user?.isSuperAdminView

  const currentScope = merchants.find((m) => m.merchant_id === user?.merchantId)

  // The current scope's own ancestry is authoritative for it — the session carries it even when the
  // switcher list does not (a view session before `/auth/me` returns, a scope outside the grant).
  const current: ScopeLabels = user?.hierarchy
    ? {
        org: user.hierarchy.hs_org_name ?? user.hierarchy.hs_org_id ?? null,
        merchant: user.hierarchy.hs_merchant_name ?? user.hierarchy.hs_merchant_id ?? null,
        profile: user.hierarchy.profile_name ?? currentScope?.merchant_name ?? user.merchantId,
      }
    : currentScope
      ? labelsOf(currentScope)
      : { org: null, merchant: null, profile: user?.merchantId ?? 'No scope' }

  const groups = useMemo(() => groupScopes(merchants), [merchants])
  const trimmedQuery = query.trim().toLowerCase()

  // A level that is the same for every scope in the list names nothing — it is the account the
  // session is already in, which the trigger chip and the footer id already say. Only the levels
  // that actually separate one row from another are drawn, so a single-merchant account gets a
  // plain list of profiles instead of two header rows repeating what is above the panel.
  // Computed over every group, not the filtered ones, so searching never reshapes the list.
  const showMerchantHeaders = groups.length > 1
  const showOrgHeaders = useMemo(() => new Set(groups.map((g) => g.orgKey)).size > 1, [groups])

  // ...and where no header row will carry those levels, they are stated once above the list. The
  // two are exclusive: ancestry common to every row is context for the whole panel, ancestry that
  // differs between rows is a header on each group. Either way it is written exactly once.
  const sharedAncestry = showMerchantHeaders ? null : (groups[0] ?? null)

  // Searching filters the rows and reveals every group still holding one, so a match is never
  // hidden behind a collapsed header.
  const visibleGroups = useMemo(() => {
    if (!trimmedQuery) return groups
    return groups
      .map((group) => ({ ...group, scopes: group.scopes.filter((s) => matches(s, trimmedQuery)) }))
      .filter((group) => group.scopes.length > 0)
  }, [groups, trimmedQuery])

  // The one rule for whether a group's rows are hidden, so what the keyboard walks is exactly what
  // is painted. A group whose merchant header is not drawn has no control to fold it and stays open.
  const isFolded = (group: MerchantGroup) =>
    showMerchantHeaders && group.merchant !== null && !trimmedQuery && collapsed.has(group.key)

  // The rows the keyboard walks, in the order they are painted.
  const navigableScopes = useMemo(
    () => visibleGroups.flatMap((g) => (isFolded(g) ? [] : g.scopes)),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- isFolded is derived from these
    [visibleGroups, collapsed, trimmedQuery, showMerchantHeaders],
  )

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  // Opening starts from a clean search on the current scope's group, with the rest of a large tree
  // folded away and the cursor already on the scope the session is on — in an org of a few dozen
  // profiles that row is otherwise well below the fold.
  useEffect(() => {
    if (!open) return
    setQuery('')
    setActiveId(user?.merchantId ?? null)
    const currentGroup = groups.find((g) =>
      g.scopes.some((s) => s.merchant_id === user?.merchantId),
    )
    setCollapsed(
      merchants.length <= AUTO_EXPAND_LIMIT
        ? new Set()
        : new Set(groups.filter((g) => g.key !== currentGroup?.key).map((g) => g.key)),
    )
  }, [open, groups, merchants.length, user?.merchantId])

  // While searching, the cursor sits on the top match, so Enter takes the row the typing pointed at.
  // Only the query moves the list here — collapsing is ignored during a search — so this does not
  // fight the arrow keys.
  useEffect(() => {
    if (!open || !trimmedQuery) return
    setActiveId(navigableScopes[0]?.merchant_id ?? null)
  }, [open, trimmedQuery, navigableScopes])

  useEffect(() => {
    if (!open) return
    listRef.current
      ?.querySelector('[data-active="true"]')
      ?.scrollIntoView({ block: 'nearest' })
  }, [activeId, open])

  async function handleSwitch(merchantId: string) {
    if (merchantId === user?.merchantId) {
      setOpen(false)
      return
    }
    if (switching) return
    setSwitching(merchantId)
    try {
      const res = await apiFetch<SwitchMerchantResponse>('/auth/switch-merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_id: merchantId }),
      })
      updateMerchant(res.token, res.merchant_id, res.merchants)
      setMerchantId(res.merchant_id)
      setOpen(false)
    } catch {
      // Leave the session where it is; the picker stays open to retry.
    } finally {
      setSwitching(null)
    }
  }

  function toggleGroup(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Escape') {
      setOpen(false)
      return
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (navigableScopes.length === 0) return
      const delta = e.key === 'ArrowDown' ? 1 : -1
      // A cursor on a row the current list no longer shows starts the walk from the near end.
      const from = navigableScopes.findIndex((s) => s.merchant_id === activeId)
      const next =
        from === -1
          ? delta === 1
            ? 0
            : navigableScopes.length - 1
          : (from + delta + navigableScopes.length) % navigableScopes.length
      setActiveId(navigableScopes[next].merchant_id)
      return
    }
    if (e.key === 'Enter') {
      e.preventDefault()
      const target = navigableScopes.find((s) => s.merchant_id === activeId)
      if (target) handleSwitch(target.merchant_id)
    }
  }

  // The chip names the merchant and the profile, so it takes the merchant's icon; a scope with no
  // ancestry is a merchant in its own right and keeps the org-level one.
  const TriggerIcon = current.merchant ? Store : Building2

  const triggerBody = (
    <>
      <TriggerIcon size={14} className="shrink-0 text-slate-500" />
      <span className="flex min-w-0 flex-col items-start leading-tight">
        {current.merchant && (
          <span className="max-w-[170px] truncate text-[11px] font-medium uppercase tracking-wide text-slate-500 dark:text-slate-400 leading-4">
            {current.merchant}
          </span>
        )}
        <span className="max-w-[170px] truncate text-[12.5px] font-semibold text-slate-700 dark:text-slate-200 leading-[17px]">
          {current.profile}
        </span>
      </span>
    </>
  )

  const triggerTitle = [
    current.org && `Organization  ${current.org}`,
    current.merchant && `Merchant  ${current.merchant}`,
    `Profile  ${current.profile}`,
    `Scope id  ${user?.merchantId ?? '—'}`,
  ]
    .filter(Boolean)
    .join('\n')

  if (!canSwitch) {
    // Nothing to move between, so the scope is context rather than a control. Hyperswitch owns this
    // tree for a handed-over session and it is changed there, not here.
    return (
      <div
        title={triggerTitle}
        className="flex h-10 min-w-0 items-center gap-2 rounded-lg border border-[#e6e6ee] bg-white px-3 text-slate-700 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-300"
      >
        {triggerBody}
      </div>
    )
  }

  return (
    <div className="relative" ref={rootRef}>
      <button
        onClick={() => setOpen((v) => !v)}
        title={triggerTitle}
        className="flex h-10 min-w-0 items-center gap-2 rounded-lg border border-[#e6e6ee] bg-white px-3 text-slate-700 transition-colors hover:bg-slate-50 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-300 dark:hover:bg-[#18181f]"
      >
        {triggerBody}
        <ChevronDown size={12} className="shrink-0 text-slate-500" />
      </button>

      {open && (
        <div
          onKeyDown={handleKeyDown}
          className="absolute right-0 top-12 z-50 flex max-h-[min(560px,70vh)] w-[380px] flex-col overflow-hidden rounded-xl border border-[#e6e6ee] bg-white shadow-xl dark:border-[#1a1a24] dark:bg-[#0c0c10]"
        >
          {/* The org and merchant every listed scope sits under. Not a repeat of the rows below:
              it is the level they all share, which is exactly why no row states it. */}
          {sharedAncestry && (sharedAncestry.org || sharedAncestry.merchant) && (
            <div className="flex items-center gap-1.5 border-b border-[#e6e6ee] px-3.5 py-2 text-slate-500 dark:border-[#1a1a24] dark:text-slate-400">
              {sharedAncestry.org && (
                <>
                  <Building2 size={11} className="shrink-0" />
                  <span
                    className={`min-w-0 truncate leading-4 ${
                      sharedAncestry.orgIsId ? 'font-mono text-[10.5px]' : 'text-[11px]'
                    }`}
                  >
                    {sharedAncestry.org}
                  </span>
                </>
              )}
              {sharedAncestry.org && sharedAncestry.merchant && (
                <ChevronRight size={10} className="shrink-0" />
              )}
              {sharedAncestry.merchant && (
                <>
                  <Store size={11} className="shrink-0" />
                  <span className="min-w-0 truncate text-[11px] font-medium text-slate-600 dark:text-slate-300 leading-4">
                    {sharedAncestry.merchant}
                  </span>
                </>
              )}
            </div>
          )}

          {/* The search is the panel's first row rather than a boxed field inside a band: it is
              focused the moment the panel opens, so a border and a focus ring would draw a second
              outline around the one control already holding the caret. */}
          <div className="relative border-b border-[#e6e6ee] dark:border-[#1a1a24]">
            <Search
              size={13}
              className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500"
            />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={isGranted ? 'Search profiles, merchants, ids' : 'Search merchants'}
              autoFocus
              className="h-11 w-full bg-transparent pl-9 pr-3.5 text-[13px] text-slate-700 placeholder:text-slate-500 focus:outline-none dark:text-slate-200 leading-[18px]"
            />
          </div>

          {/* Top padding only where no header is drawn: a sticky header pins below the scroll
              container's own padding, and rows would scroll through the band left above it. */}
          <div
            ref={listRef}
            className={`flex-1 overflow-y-auto pb-1 ${
              showOrgHeaders || showMerchantHeaders ? '' : 'pt-1'
            }`}
          >
            {visibleGroups.length === 0 && (
              <p className="px-3.5 py-6 text-center text-[12px] text-slate-500 leading-4">
                Nothing matches “{query.trim()}”.
              </p>
            )}

            {visibleGroups.map((group, i) => {
              const orgHeaderRow = showOrgHeaders && group.org !== null
              const showOrgHeader =
                orgHeaderRow && (i === 0 || visibleGroups[i - 1].orgKey !== group.orgKey)
              const merchantHeaderRow = showMerchantHeaders && group.merchant !== null
              const folded = isFolded(group)

              return (
                <div key={group.key}>
                  {/* Both headers stay pinned while their rows scroll: a long org is exactly where
                      the answer to "which merchant is this profile under?" scrolls out of reach.
                      The merchant sits below the org band, so it pins at that band's height. */}
                  {showOrgHeader && (
                    <p className="sticky top-0 z-20 flex items-center gap-1.5 bg-white px-3.5 pb-1 pt-2.5 text-slate-500 dark:bg-[#0c0c10] dark:text-slate-500 leading-4">
                      <Building2 size={11} className="shrink-0" />
                      {/* An org with no synced name shows its id, which upper-casing would mangle
                          past recognition — that spelling is the only copy of it the user has. */}
                      <span
                        className={`truncate ${
                          group.orgIsId
                            ? 'font-mono text-[10.5px]'
                            : 'text-[11px] font-semibold uppercase tracking-widest'
                        }`}
                      >
                        {group.org}
                      </span>
                    </p>
                  )}

                  {merchantHeaderRow && (
                    <button
                      onClick={() => toggleGroup(group.key)}
                      className={`sticky z-10 flex w-full items-center gap-1.5 bg-white px-3.5 py-1.5 text-left transition-colors hover:bg-slate-50 dark:bg-[#0c0c10] dark:hover:bg-[#13131a] ${
                        orgHeaderRow ? 'top-[26px]' : 'top-0'
                      }`}
                    >
                      <ChevronRight
                        size={12}
                        className={`shrink-0 text-slate-500 transition-transform ${folded ? '' : 'rotate-90'}`}
                      />
                      <Store size={12} className="shrink-0 text-slate-500" />
                      <span className="flex-1 truncate text-[12px] font-medium text-slate-600 dark:text-slate-300 leading-4">
                        {group.merchant}
                      </span>
                      <span className="shrink-0 text-[11px] tabular-nums text-slate-500 leading-4">
                        {group.scopes.length}
                      </span>
                    </button>
                  )}

                  {!folded &&
                    group.scopes.map((scope) => {
                      const LeafIcon = group.merchant !== null ? Layers : Building2
                      const labels = labelsOf(scope)
                      const isCurrent = scope.merchant_id === user?.merchantId
                      const isActive = scope.merchant_id === activeId
                      return (
                        <button
                          key={scope.merchant_id}
                          data-active={isActive}
                          onMouseEnter={() => setActiveId(scope.merchant_id)}
                          onClick={() => handleSwitch(scope.merchant_id)}
                          disabled={switching === scope.merchant_id}
                          className={`flex w-full items-center gap-2 py-1.5 pr-3 text-left transition-colors disabled:opacity-50 ${
                            merchantHeaderRow ? 'pl-9' : 'pl-3.5'
                          } ${isActive ? 'bg-slate-50 dark:bg-[#13131a]' : ''}`}
                        >
                          <LeafIcon size={12} className="shrink-0 text-slate-500 dark:text-slate-400" />
                          <span
                            className={`flex-1 truncate text-[13px] ${
                              isCurrent
                                ? 'font-semibold text-brand-600'
                                : 'font-medium text-slate-700 dark:text-slate-300'
                            } leading-[18px]`}
                          >
                            {labels.profile}
                          </span>
                          <span className="shrink-0 font-mono text-[11px] text-slate-500 dark:text-slate-400 leading-4">
                            {shortId(scope.merchant_id)}
                          </span>
                          {isCurrent && <Check size={13} className="shrink-0 text-brand-600" />}
                        </button>
                      )
                    })}
                </div>
              )
            })}
          </div>

          {/* The list already marks where the session is, so the footer carries only what a row
              cannot: the current scope's id in full, to copy into an API call or a support thread.
              Its tooltip holds the breadcrumb for the levels the list no longer repeats. */}
          <div className="flex items-center gap-1.5 border-t border-[#e6e6ee] px-3.5 py-2 dark:border-[#1a1a24]">
            <span
              title={triggerTitle}
              className="min-w-0 truncate font-mono text-[11px] text-slate-500 dark:text-slate-400 leading-4"
            >
              {user?.merchantId}
            </span>
            <CopyButton text={user?.merchantId ?? ''} label="Copy scope id" />
            {canAddMerchant && (
              <button
                onClick={() => {
                  setOpen(false)
                  navigate('/onboarding')
                }}
                className="ml-auto flex shrink-0 items-center gap-1 rounded-md px-1.5 py-1 text-brand-600 transition-colors hover:bg-slate-50 dark:hover:bg-[#13131a]"
              >
                <Plus size={12} />
                <span className="text-[12px] font-medium leading-4">Add merchant</span>
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  )
}

