import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Plus, ChevronDown, ChevronRight, Zap, PowerOff, Pencil, Copy, Trash2 } from 'lucide-react'
import { Card } from '../ui/Card'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { PageHeading } from '../ui/PageHeading'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { formatLastModified, lastModifiedMs } from '../../lib/routingRuleTimestamps'
import { FEATURE_FLAGS } from '../../lib/featureFlags'
import { RoutingAlgorithm } from '../../types/api'

type StatusFilter = 'all' | 'active' | 'inactive'

/** The type tag of a stored algorithm, wherever the response put it. */
export function algorithmType(algo: RoutingAlgorithm): string | undefined {
  return (algo.algorithm_data || algo.algorithm)?.type
}

/**
 * The merchant's algorithms and the active ones. Every routing page reads the same SWR keys, so
 * activating on one page invalidates the others — the backend deactivates the other mode's rule,
 * and a separate cache entry would go stale.
 */
export function useRoutingAlgorithms() {
  const { merchantId } = useMerchantStore()
  const all = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )
  const active = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )
  return { merchantId, all, active }
}

export interface RuleSummaryColumn {
  header: string
  /** Puts the gateway filter in this column's header. */
  gatewayFilter?: boolean
  cellClassName: string
  render: (algo: RoutingAlgorithm) => React.ReactNode
}

/** How a row stands out beyond Active/Inactive, e.g. as an arm of a running experiment. */
export interface RuleRowMark {
  rowClassName: string
  label: React.ReactNode
}

export interface RoutingRuleListConfig {
  title: string
  description: string
  /** `/routing/rules` → `/routing/rules/new`, `/routing/rules/:id/edit`. */
  basePath: string
  emptyText: string
  /** Which of the merchant's algorithms this page lists. */
  includes: (algo: RoutingAlgorithm) => boolean
  /** The gateways a rule sends traffic to, for the gateway filter. */
  gateways: (algo: RoutingAlgorithm) => string[]
  columns: RuleSummaryColumn[]
  breakdown: (algo: RoutingAlgorithm) => React.ReactNode
  /** The active algorithm that activating one of this page's rules replaces, if any. */
  replaces: (active: RoutingAlgorithm[]) => RoutingAlgorithm | undefined
  switchDialog: { title: string; description: (replaced: RoutingAlgorithm) => string }
  messages: { activated: string; deactivated: string }
  /** Marks rows that something else currently uses. */
  markRow?: (active: RoutingAlgorithm[]) => (algo: RoutingAlgorithm) => RuleRowMark | undefined
  /** Notices under the table, given the active algorithms. */
  notices: (active: RoutingAlgorithm[]) => React.ReactNode
  /** Vertical alignment of a row's cells: `top` suits summaries that wrap onto several lines. */
  cellAlign: 'top' | 'middle'
}

export function RoutingRuleListPage({ config }: { config: RoutingRuleListConfig }) {
  // Read-only sessions still see everything; the controls that would change it are inert.
  const canEditRouting = useCanEditRouting()
  const navigate = useNavigate()
  const { merchantId, all, active } = useRoutingAlgorithms()

  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())
  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  const [activating, setActivating] = useState(false)
  const [deactivatingId, setDeactivatingId] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [actionSuccess, setActionSuccess] = useState<string | null>(null)

  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [gatewayFilter, setGatewayFilter] = useState('all')
  const [nameFilter, setNameFilter] = useState('')

  const activeAlgorithms = active.data || []
  const activeIds = new Set(activeAlgorithms.map((a) => a.id))
  const replaced = config.replaces(activeAlgorithms)
  const markOf = config.markRow?.(activeAlgorithms)

  const rules = (all.data || [])
    .filter(config.includes)
    // Sorted by the same stamp the Last Modified column shows, so the order matches what is read.
    .sort((a, b) => lastModifiedMs(b) - lastModifiedMs(a))

  const gatewayOptions = Array.from(new Set(rules.flatMap(config.gateways))).sort()

  const visibleRules = rules.filter((algo) => {
    if (statusFilter === 'active' && !activeIds.has(algo.id)) return false
    if (statusFilter === 'inactive' && activeIds.has(algo.id)) return false
    if (gatewayFilter !== 'all' && !config.gateways(algo).includes(gatewayFilter)) return false
    if (nameFilter.trim()) {
      const needle = nameFilter.trim().toLowerCase()
      if (!algo.name.toLowerCase().includes(needle) && !algo.id.toLowerCase().includes(needle)) {
        return false
      }
    }
    return true
  })

  const align = config.cellAlign === 'top' ? 'align-top ' : ''
  // Name & ID, Status, the page's own columns, Last Modified, Actions.
  const columnCount = config.columns.length + 4

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  /** Runs one routing mutation with the shared busy, error and success handling. */
  async function mutate(path: string, id: string, success: string) {
    setActionError(null)
    setActionSuccess(null)
    try {
      await apiPost(path, { created_by: merchantId, routing_algorithm_id: id })
      setActionSuccess(success)
      await Promise.all([all.mutate(), active.mutate()])
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err))
    }
  }

  async function handleActivate(id: string) {
    if (!merchantId) return
    if (replaced) {
      setPendingActivateId(id)
      return
    }
    await doActivate(id)
  }

  async function doActivate(id: string) {
    setActivating(true)
    await mutate('/routing/activate', id, config.messages.activated)
    setActivating(false)
  }

  async function doDeactivate(id: string) {
    setDeactivatingId(id)
    await mutate('/routing/deactivate', id, config.messages.deactivated)
    setDeactivatingId(null)
  }

  async function doDelete(id: string) {
    if (!merchantId) return
    await mutate('/routing/delete', id, 'Rule deleted.')
    setPendingDeleteId(null)
  }

  const pendingDeleteRule = rules.find((a) => a.id === pendingDeleteId)

  return (
    <div className="space-y-6">
      <ConfirmDialog
        open={pendingActivateId !== null}
        title={config.switchDialog.title}
        description={replaced ? config.switchDialog.description(replaced) : ''}
        confirmLabel="Yes, activate"
        variant="primary"
        onConfirm={() => { const id = pendingActivateId!; setPendingActivateId(null); doActivate(id) }}
        onCancel={() => setPendingActivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeactivateId !== null}
        title="Deactivate this rule?"
        description="The rule will be deactivated for this merchant. It will remain saved and can be reactivated at any time."
        confirmLabel="Deactivate"
        variant="danger"
        onConfirm={() => { const id = pendingDeactivateId!; setPendingDeactivateId(null); doDeactivate(id) }}
        onCancel={() => setPendingDeactivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeleteId !== null}
        title="Delete this rule?"
        description={`"${pendingDeleteRule?.name}" will be permanently deleted. This cannot be undone.`}
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => doDelete(pendingDeleteId!)}
        onCancel={() => setPendingDeleteId(null)}
      />

      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <PageHeading title={config.title} description={config.description} />
        </div>
        <Button onClick={() => navigate(`${config.basePath}/new`)} disabled={!canEditRouting}>
          <Plus size={15} /> Create Rule
        </Button>
      </div>

      {actionError && <ErrorMessage error={actionError} />}
      {actionSuccess && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400">
          {actionSuccess}
        </div>
      )}

      <Card className="!rounded-[18px]">
        {!merchantId ? (
          <p className="px-4 py-6 text-sm text-slate-500">Set merchant ID to load rules.</p>
        ) : !all.data ? (
          <p className="px-4 py-6 text-sm text-slate-500">Loading...</p>
        ) : rules.length === 0 ? (
          <p className="px-4 py-6 text-sm text-slate-500">{config.emptyText}</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[880px] text-left">
              <thead>
                <tr className="border-b border-slate-100 text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:border-[#1e2330] dark:text-[#78849a] leading-4">
                  <th className="px-5 py-3.5">
                    <HeaderSearch
                      label="Rule Name & ID"
                      value={nameFilter}
                      onChange={setNameFilter}
                      ariaLabel="Filter rules by name"
                    />
                  </th>
                  <th className="px-5 py-3.5">
                    <HeaderFilter
                      label="Status"
                      value={statusFilter}
                      options={[
                        { value: 'all', label: 'All statuses' },
                        { value: 'active', label: 'Active' },
                        { value: 'inactive', label: 'Inactive' },
                      ]}
                      onChange={(v) => setStatusFilter(v as StatusFilter)}
                      ariaLabel="Filter by status"
                    />
                  </th>
                  {config.columns.map((column) => (
                    <th key={column.header} className="px-5 py-3.5">
                      {column.gatewayFilter ? (
                        <HeaderFilter
                          label={column.header}
                          value={gatewayFilter}
                          options={[
                            { value: 'all', label: 'All gateways' },
                            ...gatewayOptions.map((g) => ({ value: g, label: g })),
                          ]}
                          onChange={setGatewayFilter}
                          ariaLabel="Filter by gateway"
                        />
                      ) : (
                        column.header
                      )}
                    </th>
                  ))}
                  <th className="px-5 py-3.5">Last Modified</th>
                  <th className="px-5 py-3.5 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {visibleRules.length === 0 && (
                  <tr>
                    <td colSpan={columnCount} className="px-4 py-8 text-center">
                      <p className="text-sm text-slate-500">No rules match these filters.</p>
                      <button
                        type="button"
                        onClick={() => { setStatusFilter('all'); setGatewayFilter('all'); setNameFilter('') }}
                        className="mt-1.5 text-sm font-medium text-brand-600 hover:text-brand-700 dark:text-brand-400"
                      >
                        Clear filters
                      </button>
                    </td>
                  </tr>
                )}
                {visibleRules.map((algo) => {
                  const isActive = activeIds.has(algo.id)
                  const mark = markOf?.(algo)
                  const isExpanded = expandedRuleIds.has(algo.id)
                  // Both /routing/update and /routing/delete reject an active algorithm, so the
                  // controls mirror that rather than letting the request fail.
                  const lockedReason = isActive
                    ? 'Deactivate this rule first'
                    : !canEditRouting
                    ? 'You do not have permission to change routing'
                    : undefined

                  return [
                    <tr
                      key={algo.id}
                      data-testid="rule-row"
                      data-rule-name={algo.name}
                      onClick={() => toggleRuleExpand(algo.id)}
                      className={`cursor-pointer border-b border-slate-100 align-middle transition-colors hover:bg-slate-50 dark:border-[#1e2330] dark:hover:bg-[#11151d] ${
                        isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : mark?.rowClassName ?? ''
                      }`}
                    >
                      <td className={`${align}px-5 py-4`}>
                        <div className="flex items-start gap-2">
                          {isExpanded
                            ? <ChevronDown size={14} className="mt-1 shrink-0 text-slate-500" />
                            : <ChevronRight size={14} className="mt-1 shrink-0 text-slate-500" />
                          }
                          <div className="min-w-0">
                            <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                              {algo.name}
                            </p>
                            <p className="mt-0.5 truncate font-mono text-xs text-slate-500 dark:text-[#78849a]">
                              {algo.id}
                            </p>
                          </div>
                        </div>
                      </td>
                      <td className={`${align}px-5 py-4`}>
                        <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold ${
                          isActive
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                            : 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                        } leading-4`}>
                          {isActive ? 'Active' : 'Inactive'}
                        </span>
                        {mark && (
                          <span className="mt-1.5 block text-[11px] font-medium text-purple-600 dark:text-purple-400 leading-4">
                            {mark.label}
                          </span>
                        )}
                      </td>
                      {config.columns.map((column) => (
                        <td key={column.header} className={`${align}px-5 py-4 ${column.cellClassName}`}>
                          {column.render(algo)}
                        </td>
                      ))}
                      <td className={`${align}whitespace-nowrap px-5 py-4 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]`}>
                        {(() => {
                          const stamp = formatLastModified(algo)
                          if (!stamp) return '—'
                          return (
                            <span title={stamp.full}>
                              {stamp.date}
                              <span className="block text-[12px] text-slate-500 dark:text-[#78849a] leading-4">{stamp.time}</span>
                            </span>
                          )
                        })()}
                      </td>
                      <td className={`${align}px-5 py-4`} onClick={(e) => e.stopPropagation()}>
                        <RowMenu
                          items={[
                            isActive
                              ? {
                                  label: deactivatingId === algo.id ? 'Deactivating…' : 'Deactivate',
                                  icon: PowerOff,
                                  onSelect: () => setPendingDeactivateId(algo.id),
                                  disabled: deactivatingId === algo.id || !canEditRouting,
                                }
                              : {
                                  label: 'Activate',
                                  icon: Zap,
                                  tone: 'positive',
                                  onSelect: () => handleActivate(algo.id),
                                  disabled: activating || !canEditRouting,
                                },
                            {
                              label: 'Edit',
                              icon: Pencil,
                              onSelect: () => navigate(`${config.basePath}/${algo.id}/edit`),
                              disabled: Boolean(lockedReason),
                              hint: lockedReason,
                            },
                            {
                              label: 'Duplicate',
                              icon: Copy,
                              onSelect: () => navigate(`${config.basePath}/new?cloneFrom=${algo.id}`),
                              disabled: !canEditRouting,
                            },
                            ...(FEATURE_FLAGS.RULE_DELETION
                              ? [
                                  {
                                    label: 'Delete',
                                    icon: Trash2,
                                    tone: 'danger' as const,
                                    onSelect: () => setPendingDeleteId(algo.id),
                                    disabled: Boolean(lockedReason),
                                    hint: lockedReason,
                                  },
                                ]
                              : []),
                          ]}
                        />
                      </td>
                    </tr>,
                    isExpanded ? (
                      <tr key={`${algo.id}-detail`} className="border-b border-slate-100 dark:border-[#1e2330]">
                        <td colSpan={columnCount} className="bg-slate-50/60 px-6 py-5 dark:bg-[#0c0f17]">
                          {algo.description && algo.description !== 'N/A' && (
                            <p className="mb-3 text-sm text-slate-500 dark:text-[#78849a]">{algo.description}</p>
                          )}
                          {config.breakdown(algo)}
                        </td>
                      </tr>
                    ) : null,
                  ]
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {config.notices(activeAlgorithms)}
    </div>
  )
}
