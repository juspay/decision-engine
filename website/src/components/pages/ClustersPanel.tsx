import { useEffect, useMemo, useState } from 'react'
import { Check, ChevronDown, ChevronUp, Pencil, X } from 'lucide-react'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  setClusterOverride,
  useCostClusters,
  type ClusterFee,
  type ClustersScope,
} from '../../hooks/useCostRouting'

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

/** The card program/tier extracted from `variant` — e.g. `visastandarddebit` → "Standard",
 * `visasuperpremiumdebit` → "Superpremium", `visa_applepay` → "Apple Pay". Falls back to the raw
 * variant when the network/funding affixes can't be stripped. */
function programOf(c: ClusterFee): string {
  let v = (c.variant ?? '').toLowerCase()
  if (!v) return '—'
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
  return v ? titleCase(v) : '—'
}

/** Human-readable segment name: "Visa Standard debit · HU · HUF" (+ category). Used in the editor
 * heading, where a single-line label reads better than the split columns. */
function clusterLabel(c: ClusterFee): string {
  const program = programOf(c)
  const card = [networkLabel(c.card_network), program !== '—' ? program : '', c.funding]
    .filter(Boolean)
    .join(' ')
  const parts = [card, c.issuer_country?.toUpperCase(), c.currency?.toUpperCase()].filter(Boolean)
  const base = parts.join(' · ') || 'Unknown segment'
  const amount = amountSegmentLabel(c)
  const category = c.ic_category ? `${base} · ${c.ic_category}` : base
  return amount ? `${category} · ${amount}` : category
}

function amountSegmentLabel(c: ClusterFee): string {
  if (!c.segment_idx || c.amount_lo == null || c.amount_hi == null) return ''
  return `${formatCompact(c.amount_lo)}-${formatCompact(c.amount_hi)}`
}

function formatFee(pctBps: number | null, fixed: number | null): string {
  if (pctBps == null && fixed == null) return '—'
  const pct = `${(pctBps ?? 0).toFixed(1)} bps`
  return fixed && fixed > 0 ? `${pct} + ${fixed.toFixed(2)}` : pct
}

function formatCompact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`
  return n.toFixed(0)
}

/**
 * The merchant's top segments by settled volume, with the fee we charge each one to. Read-only in the
 * ingested-data view; editable in the overrides view, where the highest-traffic clusters can be given
 * a surgical fee that replaces the learned model for just that segment.
 */
export function ClustersPanel({
  merchantId,
  editable,
  limit = 10,
  scope,
}: {
  merchantId?: string
  editable: boolean
  limit?: number
  /** When set, shows one ingested snapshot's segments instead of the merchant-wide top set. */
  scope?: ClustersScope
}) {
  const { clusters, isLoading, error, mutate } = useCostClusters(merchantId, { limit, ...scope })
  const [editingKey, setEditingKey] = useState<string | null>(null)
  // Ranked by GMV by default (money moved = cost impact); click Volume/Txns to re-sort.
  const [sortKey, setSortKey] = useState<'gross_sum' | 'n'>('gross_sum')
  const [sortDir, setSortDir] = useState<'desc' | 'asc'>('desc')

  const sorted = useMemo(() => {
    const rows = [...clusters]
    rows.sort((a, b) => {
      const diff = a[sortKey] - b[sortKey]
      return sortDir === 'desc' ? -diff : diff
    })
    return rows
  }, [clusters, sortKey, sortDir])

  function toggleSort(key: 'gross_sum' | 'n') {
    if (key === sortKey) {
      setSortDir((d) => (d === 'desc' ? 'asc' : 'desc'))
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  if (merchantId && isLoading) {
    return (
      <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
        <Spinner size={16} /> Loading segments…
      </div>
    )
  }
  if (!clusters.length) {
    return (
      <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
        No fitted segments yet — they appear once a settlement report has been ingested.
      </p>
    )
  }

  return (
    <div className="space-y-2">
      <div className="overflow-x-auto">
        <table className="w-full min-w-[720px] text-left text-sm">
          <thead>
            <tr className="border-b border-slate-200 text-xs uppercase tracking-[0.14em] text-slate-400 dark:border-[#232833]">
              <th className="py-2 pr-3 font-semibold">Connector</th>
              <th className="py-2 pr-3 font-semibold">Network</th>
              <th className="py-2 pr-3 font-semibold">Program</th>
              <th className="py-2 pr-3 font-semibold">Type</th>
              <th className="py-2 pr-3 font-semibold">Country</th>
              <th className="py-2 pr-3 font-semibold">Currency</th>
              <th className="py-2 pr-3 font-semibold">Category</th>
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
    <th className="py-2 pr-3 text-right font-semibold">
      <button
        type="button"
        onClick={onClick}
        aria-sort={active ? (dir === 'desc' ? 'descending' : 'ascending') : 'none'}
        className={`inline-flex items-center gap-1 uppercase tracking-[0.14em] transition-colors hover:text-slate-600 dark:hover:text-slate-200 ${
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
 * for the bps/fixed inputs and its action cell for Save/Cancel/Remove — no detached editor panel.
 * The identity columns (network/program/country/…) stay put and serve as the row's label.
 */
function ClusterRow({
  c,
  editable,
  merchantId,
  isEditing,
  onEdit,
  onCancel,
  onSaved,
}: {
  c: ClusterFee
  editable: boolean
  merchantId?: string
  isEditing: boolean
  onEdit: () => void
  onCancel: () => void
  onSaved: () => void
}) {
  const isOverride = c.source === 'override'
  const label = clusterLabel(c)
  // Pre-fill with the fee the row actually shows (effective = override, else model, else an inherited
  // connector fee). Seeding from override/model alone left inherited-fee segments at 0 even though a
  // real fee was displayed.
  const seedPctBps = () => String(c.effective_pct_bps ?? c.override_pct_bps ?? c.model_pct_bps ?? 0)
  const seedFixed = () => String(c.effective_fixed ?? c.override_fixed ?? c.model_fixed ?? 0)
  const [pctBps, setPctBps] = useState(seedPctBps)
  const [fixed, setFixed] = useState(seedFixed)
  const [busy, setBusy] = useState<'save' | null>(null)
  const [error, setError] = useState<string | null>(null)

  // The row stays mounted, so re-seed the inputs from the cluster each time it enters edit mode —
  // otherwise a reopened editor would show whatever was typed (and not saved) last time. Only keyed
  // on the open/close transition so a background data refresh can't clobber in-progress typing.
  useEffect(() => {
    if (!isEditing) return
    setPctBps(seedPctBps())
    setFixed(seedFixed())
    setError(null)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isEditing])

  async function save() {
    if (!merchantId) return
    const p = parseFloat(pctBps)
    const f = parseFloat(fixed)
    if (!isFinite(p) || p < 0 || !isFinite(f) || f < 0) {
      setError('Enter non-negative numbers for bps and fixed fee.')
      return
    }
    setBusy('save')
    setError(null)
    try {
      await setClusterOverride(merchantId, c.key, { pct_bps: p, fixed: f })
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
        <td className="py-2 pr-3 text-slate-600 dark:text-[#c7cfdd]">{programOf(c)}</td>
        <td className="py-2 pr-3 capitalize text-slate-600 dark:text-[#c7cfdd]">
          {c.funding || '—'}
        </td>
        <td className="py-2 pr-3 tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {c.issuer_country?.toUpperCase() || '—'}
        </td>
        <td className="py-2 pr-3 tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {c.currency?.toUpperCase() || '—'}
        </td>
        <td className="py-2 pr-3 text-slate-500 dark:text-[#9ca7ba]">
          {c.ic_category || '—'}
          {(c.interchange_bps || amountSegmentLabel(c)) && (
            <span className="block text-[11px] text-slate-400">
              {[c.interchange_bps ? `${c.interchange_bps} bps IC` : '', amountSegmentLabel(c)]
                .filter(Boolean)
                .join(' · ')}
            </span>
          )}
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
                step="0.1"
                min="0"
                value={pctBps}
                onChange={(e) => setPctBps(e.target.value)}
                title="Percentage (bps)"
                aria-label="Percentage (bps)"
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
          ) : (
            <>
              <div className="flex items-center justify-end gap-1.5">
                {isOverride && <Badge variant="purple">Override</Badge>}
                <span className="tabular-nums font-medium text-slate-800 dark:text-[#c7cfdd]">
                  {formatFee(c.effective_pct_bps, c.effective_fixed)}
                </span>
              </div>
              {isOverride && c.model_pct_bps != null && (
                <span className="block text-[11px] tabular-nums text-slate-400 line-through">
                  {formatFee(c.model_pct_bps, c.model_fixed)}
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
                  title={`Save fee for ${label}`}
                  aria-label={`Save fee for ${label}`}
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
      {isEditing && error && (
        <tr>
          <td colSpan={editable ? 11 : 10} className="pb-2 pr-3">
            <ErrorMessage error={error} />
          </td>
        </tr>
      )}
    </>
  )
}
