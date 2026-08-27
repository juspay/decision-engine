import { useEffect, useMemo, useState } from 'react'
import { Check, ChevronDown, ChevronUp, Pencil, X } from 'lucide-react'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  countActiveFilters,
  setClusterOverride,
  useClusterFacets,
  useCostClusters,
  type ClusterFee,
  type ClusterFilters,
  type ClusterSegment,
  type ClustersScope,
} from '../../hooks/useCostRouting'
import { ClusterFilterBar } from './ClusterFilterBar'
import { bpsToPct, ccySymbol, formatFee, pctText, pctToBps } from './SeedCostShared'

// Compact numeric input for the inline fee editor. Deliberately not `inputClass` (which is `w-full`
// and would collapse in the narrow Fee cell, scrolling the value out of view).
const feeInputClass =
  'rounded-lg border border-slate-200 bg-white px-2 py-1 text-sm text-right text-slate-900 ' +
  'focus:border-brand-500 focus:outline-none dark:border-[#232833] dark:bg-[#0b1017] dark:text-white'

const NETWORK_LABELS: Record<string, string> = {
  mc: 'Mastercard',
  visa: 'Visa',
  amex: 'Amex',
  diners: 'Diners',
  discover: 'Discover',
  jcb: 'JCB',
}

function titleCase(s: string): string {
  return s ? s.charAt(0).toUpperCase() + s.slice(1) : s
}

function networkLabel(network: string): string {
  return NETWORK_LABELS[network.toLowerCase()] ?? titleCase(network)
}

/**
 * Card **tier** names, most specific first, as they appear inside an `ic_category` label
 * ("Visa UAE Consumer Credit **Platinum**"). Longest-first so "world elite" wins over "world".
 *
 * Deliberately excludes `consumer` / `commercial`: those are the cardholder *class*, an orthogonal
 * axis. A card is consumer AND Classic. Listing them here made the column mix two vocabularies —
 * "Platinum" (a tier) in one row and "Consumer" (a class) in the next, which reads as if the second
 * card had no tier when in fact the label simply never stated one.
 */
const TIER_WORDS = [
  'world elite',
  'ultra premium',
  'superpremium',
  'infinite',
  'signature',
  'platinum',
  'purchasing',
  'corporate',
  'business',
  'premium',
  'classic',
  'standard',
  'world',
  'gold',
]

/**
 * The card **tier** for a cluster, from whichever column this connector's report states it in:
 *
 *  1. `variant`, when the report encodes a scheme tier (Adyen: `visastandarddebit` → "Standard").
 *     Braintree and Checkout synthesize `{network}{funding}`, which strips to nothing.
 *  2. a tier word inside `ic_category` ("Visa UAE Consumer Credit Platinum" → "Platinum").
 *
 * Otherwise `—`: the report did not state a tier, which is NOT the same as the card having none.
 * `card_product` (the BIN's dominant interchange rate) is deliberately not a fallback here — it is a
 * rate, not a tier name, and putting it in this column made one row read "Platinum" and the next
 * "0.50% tier". It stays in the cell's tooltip, where a rate is unambiguous.
 */
function programOf(c: ClusterFee): string {
  let v = (c.variant ?? '').toLowerCase()
  // Wallet variants are their own product (e.g. "visa_applepay").
  if (v.includes('_')) {
    const w = v.split('_').slice(1).join(' ')
    if (w.includes('apple')) return 'Apple Pay'
    if (w.includes('google')) return 'Google Pay'
    return titleCase(w)
  }
  const net = c.card_network?.toLowerCase() ?? ''
  const fund = c.funding?.toLowerCase() ?? ''
  if (net && v.startsWith(net)) v = v.slice(net.length)
  if (fund && v.endsWith(fund)) v = v.slice(0, v.length - fund.length)
  if (v) return titleCase(v)

  const category = (c.ic_category ?? '').toLowerCase()
  const word = TIER_WORDS.find((w) => category.includes(w))
  // Per word, so "world elite" reads "World Elite" rather than "World elite".
  if (word) return word.split(' ').map(titleCase).join(' ')

  return '—'
}

/** Why the Program cell says what it says — the ladder is invisible otherwise. */
function programTitle(c: ClusterFee): string | undefined {
  const bps = Number(c.card_product)
  if (!c.card_product || !Number.isFinite(bps) || bps <= 0) return undefined
  return `Issuer BIN's dominant interchange rate: ${(bps / 100).toFixed(2)}%`
}

function formatCompact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`
  return n.toFixed(0)
}

// How confident the fit is, and why it's worth a merchant's attention. THIN = too few transactions
// to pin the rate; NON_LINEAR = the cost isn't one straight line (usually a cap or tier). Both are
// rates the merchant can correct with a contract rate — that's the point of showing them.
//
// These grade the WHOLE-CLUSTER single-line fit only. A NON_LINEAR cluster the segmenter then split
// into clean tiers is not poorly priced — see `FitBadge`, which reports the tiers instead.
const VERDICT_LABELS: Record<string, { label: string; variant: 'green' | 'orange' | 'red'; hint: string }> = {
  GOOD: { label: 'Good', variant: 'green', hint: 'Enough transactions and a tight fit — this rate is trusted at decide time.' },
  THIN: { label: 'Thin', variant: 'orange', hint: 'Too few transactions to pin the rate confidently. Override it with your contract rate.' },
  NON_LINEAR: { label: 'Poor fit', variant: 'red', hint: "A single rate can't price this segment — its cost isn't one straight line, and no clean set of amount tiers could be recovered either. Override it with your contract rate." },
}

/**
 * The merchant's top segments by settled volume, with the fee we charge each one to. Read-only in the
 * ingested-data view; editable in the overrides view, where the highest-traffic clusters can be given
 * a surgical fee that replaces the learned model for just that segment.
 */
export function ClustersPanel({
  merchantId,
  editable,
  limit = 500,
  scope,
  defaultSort = 'gross_sum',
}: {
  merchantId?: string
  editable: boolean
  limit?: number
  /** When set, shows one ingested snapshot's segments instead of the merchant-wide top set. */
  scope?: ClustersScope
  /** Initial ranking — also selects which top-N the backend returns, not just display order.
   * 'gross_sum' (default) = settled GMV / cost impact; 'n' = transaction count. */
  defaultSort?: 'gross_sum' | 'n'
}) {
  // Per-column filters, applied server-side. `filters` is part of the SWR key, so editing a box
  // refetches a narrowed list rather than hiding rows from the page already loaded — that's what
  // makes the long tail (clusters far below the top-N by volume) reachable at all.
  const [filters, setFilters] = useState<ClusterFilters>({})
  const activeFilters = countActiveFilters(filters)
  const { clusters, isLoading, error, mutate } = useCostClusters(merchantId, {
    limit,
    order: defaultSort,
    filters,
    ...scope,
  })
  // Suggestions come from the unfiltered scope, so clearing one box still offers its full value set.
  const { facets } = useClusterFacets(merchantId, scope)
  const [editingKey, setEditingKey] = useState<string | null>(null)
  // Seeded from `defaultSort` (GMV = money moved / cost impact; txns = count); click Volume/Txns to
  // re-sort the fetched set.
  const [sortKey, setSortKey] = useState<'gross_sum' | 'n'>(defaultSort)
  const [sortDir, setSortDir] = useState<'desc' | 'asc'>('desc')

  const sorted = useMemo(() => {
    const rows = [...clusters]
    rows.sort((a, b) => {
      const diff = a[sortKey] - b[sortKey]
      return sortDir === 'desc' ? -diff : diff
    })
    return rows
  }, [clusters, sortKey, sortDir])

  // Most reports never state a card tier (Braintree and Checkout carry none at all; Adyen only when
  // its `Payment Method Variant` encodes one), and a column of nothing but em-dashes costs width on
  // an already-wide table. So the column appears only when some row actually has a tier to show.
  const showProgram = useMemo(() => clusters.some((c) => programOf(c) !== '—'), [clusters])

  function toggleSort(key: 'gross_sum' | 'n') {
    if (key === sortKey) {
      setSortDir((d) => (d === 'desc' ? 'asc' : 'desc'))
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  // Rendered above every state (loading / empty / table). It must never unmount on a narrowing
  // filter — otherwise a filter that matches nothing would take its own clear button away with it.
  const filterBar = (
    <ClusterFilterBar
      filters={filters}
      facets={facets}
      onChange={setFilters}
      rightSlot={
        <span className="text-[12px] text-slate-500 dark:text-[#8d96aa] leading-4">
          {clusters.length.toLocaleString()} segment{clusters.length === 1 ? '' : 's'}
          {clusters.length >= limit && ' (capped — narrow the filters to see the rest)'}
        </span>
      }
    />
  )

  if (merchantId && isLoading) {
    return (
      <div className="space-y-3">
        {filterBar}
        <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
          <Spinner size={16} /> Loading segments…
        </div>
      </div>
    )
  }
  if (!clusters.length) {
    return (
      <div className="space-y-3">
        {filterBar}
        <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
          {activeFilters > 0
            ? 'No segments match these filters — clear one to widen the search.'
            : 'No fitted segments yet — they appear once a settlement report has been ingested.'}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {filterBar}
      <div className="overflow-x-auto">
        <table className="w-full min-w-[720px] text-left text-sm">
          <thead>
            <tr className="border-b border-slate-200 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] dark:border-[#232833] leading-4">
              <th className="py-2 pr-3 font-semibold">Connector</th>
              <th className="py-2 pr-3 font-semibold">Network</th>
              {showProgram && <th className="py-2 pr-3 font-semibold">Program</th>}
              <th className="py-2 pr-3 font-semibold">Type</th>
              <th className="py-2 pr-3 font-semibold">Country</th>
              <th className="py-2 pr-3 font-semibold">Currency</th>
              <th className="py-2 pr-3 font-semibold">Category</th>
              <th className="py-2 pr-3 font-semibold">Fit</th>
              <SortableHeader
                label="Volume"
                active={sortKey === 'gross_sum'}
                dir={sortDir}
                onClick={() => toggleSort('gross_sum')}
              />
              <SortableHeader
                label="Txns"
                active={sortKey === 'n'}
                dir={sortDir}
                onClick={() => toggleSort('n')}
              />
              <th className="py-2 pr-3 text-right font-semibold">Fee</th>
              {editable && <th className="py-2 pr-3" />}
            </tr>
          </thead>
          <tbody>
            {sorted.map((c) => (
              <ClusterRow
                key={c.key}
                c={c}
                editable={editable}
                showProgram={showProgram}
                merchantId={merchantId}
                isEditing={editingKey === c.key}
                onEdit={() => setEditingKey(c.key)}
                onCancel={() => setEditingKey(null)}
                onSaved={() => {
                  setEditingKey(null)
                  mutate()
                }}
              />
            ))}
          </tbody>
        </table>
      </div>

      <ErrorMessage
        error={error instanceof Error ? error.message : error ? 'Failed to load segments' : null}
      />
    </div>
  )
}

/**
 * How well we price this segment — the state AFTER segmentation, not the raw fit verdict.
 *
 * A tiered cluster is graded on its recovered pieces, not on the single line that failed to fit it:
 * a capped interchange is `NON_LINEAR` by construction, and reporting that as "Poor fit" would
 * describe a problem the segmenter has already solved. Every piece GOOD = we price it well, in
 * tiers. Only a cluster with no usable pieces falls back to the whole-cluster verdict.
 */
function FitBadge({ verdict, segments }: { verdict: string; segments?: ClusterSegment[] }) {
  const tiers = segments?.length ?? 0
  if (tiers > 0) {
    const weak = segments!.filter((s) => s.verdict?.toUpperCase() !== 'GOOD').length
    return weak === 0 ? (
      <span
        title={`One rate couldn't price this segment, so it was split into ${tiers} amount tiers — each one fits tightly. The tiered rates are what the Fee column expands to.`}
      >
        <Badge variant="green">Tiered</Badge>
      </span>
    ) : (
      <span
        title={`Split into ${tiers} amount tiers, but ${weak} of them still don't fit well. Worth overriding with your contract rate.`}
      >
        <Badge variant="orange">Tiered</Badge>
      </span>
    )
  }
  const v = VERDICT_LABELS[verdict?.toUpperCase() ?? '']
  if (v) {
    return (
      <span title={v.hint}>
        <Badge variant={v.variant}>{v.label}</Badge>
      </span>
    )
  }
  // Override-only cluster: no fitted row behind it, so there's no grade to state.
  return <span className="text-slate-500 dark:text-[#78849a]">—</span>
}

/** A right-aligned, clickable column header that shows the sort direction when active. */
function SortableHeader({
  label,
  active,
  dir,
  onClick,
}: {
  label: string
  active: boolean
  dir: 'asc' | 'desc'
  onClick: () => void
}) {
  return (
    <th className="py-2 pr-3 text-right font-medium">
      <button
        type="button"
        onClick={onClick}
        aria-sort={active ? (dir === 'desc' ? 'descending' : 'ascending') : 'none'}
        className={`inline-flex items-center gap-1 transition-colors hover:text-slate-600 dark:hover:text-slate-200 ${
          active ? 'text-slate-600 dark:text-slate-200' : ''
        }`}
      >
        {label}
        {active ? (
          dir === 'desc' ? (
            <ChevronDown size={12} />
          ) : (
            <ChevronUp size={12} />
          )
        ) : (
          <ChevronDown size={12} className="opacity-30" />
        )}
      </button>
    </th>
  )
}

/**
 * One segment row. In the overrides view it edits inline: clicking Edit swaps this row's Fee cell
 * for the rate/fixed inputs and its action cell for Save/Cancel/Remove — no detached editor panel.
 * The identity columns (network/program/country/…) stay put and serve as the row's label.
 */
function ClusterRow({
  c,
  editable,
  showProgram,
  merchantId,
  isEditing,
  onEdit,
  onCancel,
  onSaved,
}: {
  c: ClusterFee
  editable: boolean
  /** Whether the table is rendering the Program column at all (hidden when no row has a tier). */
  showProgram: boolean
  merchantId?: string
  isEditing: boolean
  onEdit: () => void
  onCancel: () => void
  onSaved: () => void
}) {
  const isOverride = c.source === 'override'
  // Pre-fill with the fee the row actually shows (effective = override, else model, else an inherited
  // connector fee). Seeding from override/model alone left inherited-fee segments at 0 even though a
  // real fee was displayed.
  const seedPct = () => String(bpsToPct(c.effective_pct_bps ?? c.override_pct_bps ?? c.model_pct_bps ?? 0))
  const seedFixed = () => String(c.effective_fixed ?? c.override_fixed ?? c.model_fixed ?? 0)
  const [pct, setPct] = useState(seedPct)
  const [fixed, setFixed] = useState(seedFixed)
  const [busy, setBusy] = useState<'save' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [expanded, setExpanded] = useState(false)
  // Capped/tiered clusters carry recovered per-segment rates; ordinary clusters don't.
  const hasSegments = (c.segments?.length ?? 0) > 0

  // The row stays mounted, so re-seed the inputs from the cluster each time it enters edit mode —
  // otherwise a reopened editor would show whatever was typed (and not saved) last time. Only keyed
  // on the open/close transition so a background data refresh can't clobber in-progress typing.
  useEffect(() => {
    if (!isEditing) return
    setPct(seedPct())
    setFixed(seedFixed())
    setError(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isEditing])

  async function save() {
    if (!merchantId) return
    const p = parseFloat(pct)
    const f = parseFloat(fixed)
    if (!isFinite(p) || p < 0 || !isFinite(f) || f < 0) {
      setError('Enter non-negative numbers for the rate and fixed fee.')
      return
    }
    setBusy('save')
    setError(null)
    try {
      await setClusterOverride(merchantId, c.key, { pct_bps: pctToBps(p), fixed: f })
      onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save fee')
    } finally {
      setBusy(null)
    }
  }

  return (
    <>
      <tr className="border-b border-slate-100 last:border-0 dark:border-[#1c1c23]">
        <td className="py-2 pr-3 capitalize text-slate-600 dark:text-[#c7cfdd]">
          {titleCase(c.connector)}
        </td>
        <td className="py-2 pr-3 font-medium text-slate-700 dark:text-[#c7cfdd]">
          {networkLabel(c.card_network)}
        </td>
        {showProgram && (
          <td className="py-2 pr-3 text-slate-600 dark:text-[#c7cfdd]" title={programTitle(c)}>
            {programOf(c)}
          </td>
        )}
        <td className="py-2 pr-3 capitalize text-slate-600 dark:text-[#c7cfdd]">
          {c.funding || '—'}
        </td>
        <td className="py-2 pr-3 tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {c.issuer_country?.toUpperCase() || '—'}
        </td>
        <td className="py-2 pr-3 tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {c.currency?.toUpperCase() || '—'}
        </td>
        <td className="py-2 pr-3 text-slate-500 dark:text-[#9ca7ba]">{c.ic_category || '—'}</td>
        <td className="py-2 pr-3">
          <FitBadge verdict={c.verdict} segments={c.segments} />
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {c.gross_sum > 0 ? formatCompact(c.gross_sum) : '—'}
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-500 dark:text-[#9ca7ba]">
          {c.n > 0 ? c.n.toLocaleString() : '—'}
        </td>
        <td className="py-2 pr-3 text-right">
          {isEditing ? (
            <div className="flex items-center justify-end gap-1">
              <input
                className={`${feeInputClass} w-20`}
                type="number"
                step="0.01"
                min="0"
                value={pct}
                onChange={(e) => setPct(e.target.value)}
                title="Rate (%)"
                aria-label="Rate (%)"
              />
              <input
                className={`${feeInputClass} w-16`}
                type="number"
                step="0.01"
                min="0"
                value={fixed}
                onChange={(e) => setFixed(e.target.value)}
                title="Fixed per txn"
                aria-label="Fixed per txn"
              />
            </div>
          ) : hasSegments ? (
            <button
              type="button"
              onClick={() => setExpanded((v) => !v)}
              className="ml-auto flex items-center justify-end gap-1.5"
              title="Show per-segment rates"
              aria-expanded={expanded}
            >
              <span className="rounded bg-amber-100 px-1.5 py-0.5 text-[11px] font-medium text-amber-700 dark:bg-amber-500/15 dark:text-amber-400 leading-4">
                {c.segments!.length} tiers
              </span>
              <span className="tabular-nums font-medium text-slate-800 dark:text-[#c7cfdd]">
                {formatFee(c.effective_pct_bps, c.effective_fixed, { currency: c.currency })}
              </span>
              {expanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            </button>
          ) : (
            <>
              <div className="flex items-center justify-end gap-1.5">
                {isOverride && <Badge variant="purple">Override</Badge>}
                <span className="tabular-nums font-medium text-slate-800 dark:text-[#c7cfdd]">
                  {formatFee(c.effective_pct_bps, c.effective_fixed, { currency: c.currency })}
                </span>
              </div>
              {isOverride && c.model_pct_bps != null && (
                <span className="block text-[11px] tabular-nums text-slate-500 line-through leading-4">
                  {formatFee(c.model_pct_bps, c.model_fixed, { currency: c.currency })}
                </span>
              )}
            </>
          )}
        </td>
        {editable && (
          <td className="py-2 pr-3 text-right">
            {isEditing ? (
              <div className="flex items-center justify-end gap-1 whitespace-nowrap">
                <button
                  type="button"
                  onClick={save}
                  disabled={busy !== null || !merchantId}
                  title="Save fee"
                  aria-label="Save fee"
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md bg-brand-600 text-white transition-colors hover:bg-brand-700 disabled:opacity-40 dark:bg-white dark:text-black dark:hover:bg-slate-200"
                >
                  {busy === 'save' ? <Spinner size={13} /> : <Check size={14} />}
                </button>
                <button
                  type="button"
                  onClick={onCancel}
                  disabled={busy !== null}
                  title="Cancel"
                  aria-label="Cancel"
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-800 disabled:opacity-40 dark:text-[#a1a1aa] dark:hover:bg-[#121214] dark:hover:text-white"
                >
                  <X size={14} />
                </button>
              </div>
            ) : (
              <Button variant="ghost" size="sm" onClick={onEdit}>
                <Pencil size={13} /> Edit
              </Button>
            )}
          </td>
        )}
      </tr>
      {expanded && hasSegments && (
        <tr className="bg-slate-50/60 dark:bg-[#15151a]">
          <td colSpan={10 + (showProgram ? 1 : 0) + (editable ? 1 : 0)} className="px-3 pb-3 pt-1">
            <div className="mb-1 text-[11px] text-slate-500 dark:text-[#8d96aa] leading-4">
              Per-segment rates — this cluster’s interchange is capped or tiered, so one rate can’t
              price it. Each tier below is priced separately by transaction amount.
            </div>
            <table className="w-full text-[12px] leading-4">
              <thead>
                <tr className="text-slate-500 dark:text-[#78849a]">
                  <th className="py-1 pr-3 text-left font-medium">Amount range</th>
                  <th className="py-1 pr-3 text-right font-medium">Rate</th>
                  <th className="py-1 pr-3 text-right font-medium">Fixed</th>
                  <th className="py-1 pr-3 text-right font-medium">Txns</th>
                  <th className="py-1 pr-3 text-right font-medium">Volume</th>
                  <th className="py-1 pr-3 text-right font-medium">Fit err (bps)</th>
                  <th className="py-1 text-right font-medium">Quality</th>
                </tr>
              </thead>
              <tbody>
                {c.segments!.map((s) => (
                  <tr
                    key={s.seg_idx}
                    className="tabular-nums text-slate-600 dark:text-[#c7cfdd]"
                  >
                    <td className="py-1 pr-3 text-left">
                      {formatCompact(s.lo)}–{formatCompact(s.hi)}
                    </td>
                    <td className="py-1 pr-3 text-right font-medium">
                      {s.pct_bps != null ? pctText(s.pct_bps) : '—'}
                    </td>
                    <td className="py-1 pr-3 text-right">
                      {s.fixed != null ? `${ccySymbol(c.currency)}${s.fixed.toFixed(2)}` : '—'}
                    </td>
                    <td className="py-1 pr-3 text-right">{s.n.toLocaleString()}</td>
                    <td className="py-1 pr-3 text-right">{formatCompact(s.gross_sum)}</td>
                    <td className="py-1 pr-3 text-right">
                      {s.bps_rmse != null ? s.bps_rmse.toFixed(1) : '—'}
                    </td>
                    <td className="py-1 text-right text-slate-500 dark:text-[#9ca7ba]">
                      {/* Never the raw enum: `NON_LINEAR` is an internal grade name and reads as a
                          system error to a merchant. Same wording as the cluster-level badge. */}
                      {VERDICT_LABELS[s.verdict?.toUpperCase() ?? '']?.label ?? s.verdict}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </td>
        </tr>
      )}
      {isEditing && error && (
        <tr>
          <td colSpan={10 + (showProgram ? 1 : 0) + (editable ? 1 : 0)} className="pb-2 pr-3">
            <ErrorMessage error={error} />
          </td>
        </tr>
      )}
    </>
  )
}
