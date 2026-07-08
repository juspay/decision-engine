import { useMemo, useState } from 'react'
import { ChevronDown, ChevronUp, Pencil } from 'lucide-react'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  deleteClusterOverride,
  setClusterOverride,
  useCostClusters,
  type ClusterFee,
  type ClustersScope,
} from '../../hooks/useCostRouting'
import { inputClass } from './CostRoutingShared'

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
  return c.ic_category ? `${base} · ${c.ic_category}` : base
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
            {sorted.map((c) => {
              const isOverride = c.source === 'override'
              return (
                <tr
                  key={c.key}
                  className="border-b border-slate-100 last:border-0 dark:border-[#1c1c23]"
                >
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
                  </td>
                  <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
                    {c.gross_sum > 0 ? formatCompact(c.gross_sum) : '—'}
                  </td>
                  <td className="py-2 pr-3 text-right tabular-nums text-slate-500 dark:text-[#9ca7ba]">
                    {c.n > 0 ? c.n.toLocaleString() : '—'}
                  </td>
                  <td className="py-2 pr-3 text-right">
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
                  </td>
                  {editable && (
                    <td className="py-2 pr-3 text-right">
                      {editingKey !== c.key && (
                        <Button variant="ghost" size="sm" onClick={() => setEditingKey(c.key)}>
                          <Pencil size={13} /> Edit
                        </Button>
                      )}
                    </td>
                  )}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      {editable &&
        editingKey &&
        (() => {
          const c = clusters.find((x) => x.key === editingKey)
          if (!c) return null
          return (
            <ClusterFeeEditor
              key={c.key}
              merchantId={merchantId}
              cluster={c}
              onCancel={() => setEditingKey(null)}
              onSaved={() => {
                setEditingKey(null)
                mutate()
              }}
            />
          )
        })()}

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

function ClusterFeeEditor({
  merchantId,
  cluster,
  onCancel,
  onSaved,
}: {
  merchantId?: string
  cluster: ClusterFee
  onCancel: () => void
  onSaved: () => void
}) {
  const [pctBps, setPctBps] = useState(
    String(cluster.override_pct_bps ?? cluster.model_pct_bps ?? 0),
  )
  const [fixed, setFixed] = useState(String(cluster.override_fixed ?? cluster.model_fixed ?? 0))
  const [busy, setBusy] = useState<'save' | 'clear' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const canClear = cluster.source === 'override'

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
      await setClusterOverride(merchantId, cluster.key, { pct_bps: p, fixed: f })
      onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save fee')
    } finally {
      setBusy(null)
    }
  }

  async function clear() {
    if (!merchantId) return
    setBusy('clear')
    setError(null)
    try {
      await deleteClusterOverride(merchantId, cluster.key)
      onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to clear override')
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="space-y-3 rounded-lg border border-slate-200 bg-white p-3 dark:border-[#232833] dark:bg-[#0c1219]">
      <p className="text-sm font-medium text-slate-700 dark:text-[#c7cfdd]">{clusterLabel(cluster)}</p>
      <div className="flex flex-wrap items-end gap-3">
        <label className="space-y-1">
          <span className="block text-xs font-medium text-slate-600 dark:text-[#9ca7ba]">
            Percentage (bps)
          </span>
          <input
            className={`${inputClass} w-32`}
            type="number"
            step="0.1"
            min="0"
            value={pctBps}
            onChange={(e) => setPctBps(e.target.value)}
          />
        </label>
        <label className="space-y-1">
          <span className="block text-xs font-medium text-slate-600 dark:text-[#9ca7ba]">
            Fixed per txn
          </span>
          <input
            className={`${inputClass} w-32`}
            type="number"
            step="0.01"
            min="0"
            value={fixed}
            onChange={(e) => setFixed(e.target.value)}
          />
        </label>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={save} disabled={busy !== null || !merchantId}>
            {busy === 'save' ? <Spinner size={13} /> : null}
            Save fee
          </Button>
          <Button variant="ghost" size="sm" onClick={onCancel} disabled={busy !== null}>
            Cancel
          </Button>
          {canClear && (
            <Button variant="danger" size="sm" onClick={clear} disabled={busy !== null}>
              {busy === 'clear' ? <Spinner size={13} /> : null}
              Remove
            </Button>
          )}
        </div>
      </div>
      <ErrorMessage error={error} />
    </div>
  )
}
