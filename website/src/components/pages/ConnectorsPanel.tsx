import { useState } from 'react'
import { ChevronDown, ChevronRight, Network, Pencil, Plus, X } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { ClustersPanel } from './ClustersPanel'
import {
  deleteFeeOverride,
  setFeeOverride,
  useConnectorFees,
  type ConnectorFee,
} from '../../hooks/useCostRouting'
import { inputClass } from './CostRoutingShared'

// Connectors a merchant can set a manual blended fee for even before any settlement report is
// ingested (e.g. Stripe from contract terms). The list a fee can be *added* against; connectors
// that already have a fit/override show up from the API regardless.
const KNOWN_CONNECTORS = ['adyen', 'stripe', 'checkout', 'worldpay', 'braintree', 'chase', 'cybersource']

function titleCase(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

/** "45.0 bps + 0.10 / txn" (fixed omitted when zero). */
function formatFee(pctBps: number | null, fixed: number | null): string {
  if (pctBps == null && fixed == null) return '—'
  const pct = `${(pctBps ?? 0).toFixed(1)} bps`
  return fixed && fixed > 0 ? `${pct} + ${fixed.toFixed(2)} / txn` : pct
}

/**
 * Connectors overview: every connector the merchant has a fit, an override, or credentials for —
 * with the blended fee we'd actually charge to and an inline editor to override it. Setting an
 * override makes all new EV calculations use that flat rate for the connector (see the backend
 * `serving::lookup`). This is the primary thing a merchant configures here; ingestion history and
 * coverage are supporting detail below.
 */
export function ConnectorsPanel({ merchantId }: { merchantId?: string }) {
  const { fees, isLoading, error, mutate } = useConnectorFees(merchantId)
  // Which connector row's editor is open, which is expanded to its segments, and a draft for adding
  // a brand-new connector.
  const [editing, setEditing] = useState<string | null>(null)
  const [expanded, setExpanded] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)

  const configured = new Set(fees.map((f) => f.connector))
  const addableConnectors = KNOWN_CONNECTORS.filter((c) => !configured.has(c))

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Network size={16} className="text-brand-500" />
            <div>
              <SurfaceLabel>Connectors</SurfaceLabel>
              <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
                Fees &amp; overrides per connector
              </h2>
              <p className="mt-0.5 text-xs text-slate-500 dark:text-[#9ca7ba]">
                Set a connector-wide fee, or expand one to override its individual segments.
              </p>
            </div>
          </div>
          {merchantId && addableConnectors.length > 0 && !adding && (
            <Button variant="secondary" size="sm" onClick={() => setAdding(true)}>
              <Plus size={14} /> Add connector
            </Button>
          )}
        </div>
      </CardHeader>
      <CardBody className="space-y-3">
        {!merchantId ? (
          <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
            Set a merchant ID to view configured connectors.
          </p>
        ) : isLoading ? (
          <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
            <Spinner size={16} /> Loading connectors...
          </div>
        ) : (
          <>
            {adding && (
              <AddConnectorRow
                merchantId={merchantId}
                connectors={addableConnectors}
                onDone={() => {
                  setAdding(false)
                  mutate()
                }}
                onCancel={() => setAdding(false)}
              />
            )}

            {fees.length === 0 && !adding ? (
              <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
                No connectors yet. Add one to set its blended fee, or configure ingestion below to
                learn fees from settlement reports.
              </p>
            ) : (
              <ul className="divide-y divide-slate-100 dark:divide-[#1c1c23]">
                {fees.map((fee) => (
                  <ConnectorRow
                    key={fee.connector}
                    merchantId={merchantId}
                    fee={fee}
                    isEditing={editing === fee.connector}
                    isExpanded={expanded === fee.connector}
                    onToggleExpand={() =>
                      setExpanded((prev) => (prev === fee.connector ? null : fee.connector))
                    }
                    onEdit={() => setEditing(fee.connector)}
                    onClose={() => setEditing(null)}
                    onSaved={() => {
                      setEditing(null)
                      mutate()
                    }}
                  />
                ))}
              </ul>
            )}
          </>
        )}
        <ErrorMessage
          error={error instanceof Error ? error.message : error ? 'Failed to load connectors' : null}
        />
      </CardBody>
    </Card>
  )
}

function ConnectorRow({
  merchantId,
  fee,
  isEditing,
  isExpanded,
  onToggleExpand,
  onEdit,
  onClose,
  onSaved,
}: {
  merchantId: string
  fee: ConnectorFee
  isEditing: boolean
  isExpanded: boolean
  onToggleExpand: () => void
  onEdit: () => void
  onClose: () => void
  onSaved: () => void
}) {
  const hasOverride = fee.source === 'override'
  const hasModel = fee.model_pct_bps != null
  // Only connectors with a fitted account have segments to drill into.
  const canExpand = Boolean(fee.account)

  return (
    <li className="py-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <button
          type="button"
          onClick={canExpand ? onToggleExpand : undefined}
          className={`flex min-w-0 items-center gap-2 text-left ${canExpand ? '' : 'cursor-default'}`}
          aria-expanded={canExpand ? isExpanded : undefined}
        >
          {canExpand ? (
            <span className="text-slate-400">
              {isExpanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
            </span>
          ) : (
            <span className="w-[15px]" />
          )}
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium text-slate-800 dark:text-white">
                {titleCase(fee.connector)}
              </span>
              {hasOverride ? (
                <Badge variant="purple">Manual override</Badge>
              ) : hasModel ? (
                <Badge variant="green">From reports</Badge>
              ) : (
                <Badge variant="gray">No fee set</Badge>
              )}
            </div>
            <p className="mt-0.5 text-xs text-slate-400">
              {fee.account ? `Account ${fee.account}` : 'No ingestion account'}
              {fee.override_updated_at && hasOverride && (
                <> · edited {new Date(fee.override_updated_at).toLocaleDateString()}</>
              )}
            </p>
          </div>
        </button>

        <div className="flex items-center gap-3">
          <div className="text-right">
            <p className="text-sm font-semibold tabular-nums text-slate-800 dark:text-[#c7cfdd]">
              {formatFee(fee.effective_pct_bps, fee.effective_fixed)}
            </p>
            {/* When an override masks a learned model, show what it replaced. */}
            {hasOverride && hasModel && (
              <p className="text-[11px] text-slate-400 line-through tabular-nums">
                {formatFee(fee.model_pct_bps, fee.model_fixed)}
              </p>
            )}
          </div>
          {!isEditing && (
            <Button variant="ghost" size="sm" onClick={onEdit}>
              <Pencil size={13} /> Connector fee
            </Button>
          )}
        </div>
      </div>

      {isEditing && (
        <FeeEditor
          merchantId={merchantId}
          connector={fee.connector}
          initialPctBps={fee.override_pct_bps ?? fee.model_pct_bps ?? 0}
          initialFixed={fee.override_fixed ?? fee.model_fixed ?? 0}
          canClear={hasOverride}
          onCancel={onClose}
          onSaved={onSaved}
        />
      )}

      {isExpanded && canExpand && (
        <div className="mt-3 rounded-lg border border-slate-200 bg-slate-50/50 p-3 dark:border-[#232833] dark:bg-[#0b1017]">
          <p className="mb-2 text-xs text-slate-500 dark:text-[#9ca7ba]">
            Override individual segments for {titleCase(fee.connector)} · {fee.account}. A segment
            fee beats this connector's blanket fee.
          </p>
          <ClustersPanel
            merchantId={merchantId}
            editable
            limit={10}
            scope={{ connector: fee.connector, account: fee.account ?? undefined }}
          />
        </div>
      )}
    </li>
  )
}

function AddConnectorRow({
  merchantId,
  connectors,
  onDone,
  onCancel,
}: {
  merchantId: string
  connectors: string[]
  onDone: () => void
  onCancel: () => void
}) {
  const [connector, setConnector] = useState(connectors[0] ?? '')

  return (
    <div className="rounded-lg border border-slate-200 bg-slate-50/60 p-3 dark:border-[#232833] dark:bg-[#0b1017]">
      <div className="mb-3 flex items-center justify-between">
        <span className="text-sm font-medium text-slate-700 dark:text-[#c7cfdd]">
          Add a connector fee
        </span>
        <button
          onClick={onCancel}
          className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200"
          aria-label="Cancel"
        >
          <X size={16} />
        </button>
      </div>
      <div className="mb-3 max-w-xs">
        <select className={inputClass} value={connector} onChange={(e) => setConnector(e.target.value)}>
          {connectors.map((c) => (
            <option key={c} value={c}>
              {titleCase(c)}
            </option>
          ))}
        </select>
      </div>
      <FeeEditor
        merchantId={merchantId}
        connector={connector}
        initialPctBps={0}
        initialFixed={0}
        canClear={false}
        onCancel={onCancel}
        onSaved={onDone}
      />
    </div>
  )
}

function FeeEditor({
  merchantId,
  connector,
  initialPctBps,
  initialFixed,
  canClear,
  onCancel,
  onSaved,
}: {
  merchantId: string
  connector: string
  initialPctBps: number
  initialFixed: number
  canClear: boolean
  onCancel: () => void
  onSaved: () => void
}) {
  const [pctBps, setPctBps] = useState(String(initialPctBps))
  const [fixed, setFixed] = useState(String(initialFixed))
  const [busy, setBusy] = useState<'save' | 'clear' | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function save() {
    const p = parseFloat(pctBps)
    const f = parseFloat(fixed)
    if (!isFinite(p) || p < 0 || !isFinite(f) || f < 0) {
      setError('Enter non-negative numbers for bps and fixed fee.')
      return
    }
    setBusy('save')
    setError(null)
    try {
      await setFeeOverride(merchantId, connector, { pct_bps: p, fixed: f })
      onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save fee')
    } finally {
      setBusy(null)
    }
  }

  async function clear() {
    setBusy('clear')
    setError(null)
    try {
      await deleteFeeOverride(merchantId, connector)
      onSaved()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to clear override')
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="mt-3 space-y-3 rounded-lg border border-slate-200 bg-white p-3 dark:border-[#232833] dark:bg-[#0c1219]">
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
          <Button size="sm" onClick={save} disabled={busy !== null}>
            {busy === 'save' ? <Spinner size={13} /> : null}
            Save fee
          </Button>
          <Button variant="ghost" size="sm" onClick={onCancel} disabled={busy !== null}>
            Cancel
          </Button>
          {canClear && (
            <Button variant="danger" size="sm" onClick={clear} disabled={busy !== null}>
              {busy === 'clear' ? <Spinner size={13} /> : null}
              Remove override
            </Button>
          )}
        </div>
      </div>
      <p className="text-xs text-slate-400">
        Applied to every economic-value calculation for {titleCase(connector)} from now on, replacing
        the learned model for this connector.
      </p>
      <ErrorMessage error={error} />
    </div>
  )
}
