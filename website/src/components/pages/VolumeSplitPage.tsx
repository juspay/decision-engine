import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Plus, ChevronDown, ChevronRight, Zap, PowerOff, Pencil, Copy, Trash2 } from 'lucide-react'
import { Card } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { formatLastModified, lastModifiedMs } from '../../lib/routingRuleTimestamps'
import { RoutingAlgorithm } from '../../types/api'
import { toVolumeSplitRuleDetailsState } from '../../features/routing/volumeSplit/state'
import { SplitBreakdown } from '../routing/volumeSplit/SplitBreakdown'

type StatusFilter = 'all' | 'active' | 'inactive'

/** "stripe 50% / adyen 50%" — the row's one-line view of where traffic goes. */
function describeSplit(algo: RoutingAlgorithm): string {
  const gateways = toVolumeSplitRuleDetailsState(algo)?.gateways ?? []
  const named = gateways.filter((g) => g.gatewayName.trim())
  if (named.length === 0) return 'No gateways configured'
  return named.map((g) => `${g.gatewayName} ${g.split}%`).join(' / ')
}

function splitGateways(algo: RoutingAlgorithm): string[] {
  return (toVolumeSplitRuleDetailsState(algo)?.gateways ?? [])
    .map((g) => g.gatewayName.trim())
    .filter(Boolean)
}

export function VolumeSplitPage() {
  // Read-only sessions still see everything; the controls that would change it are inert.
  const canEditRouting = useCanEditRouting()
  const navigate = useNavigate()
  const { merchantId } = useMerchantStore()

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

  // Same SWR keys as the rule-based pages, so activating here invalidates their view too — the
  // backend deactivates the other mode's rule, and a separate cache entry would go stale.
  const { data: allRules, mutate: mutateRules } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )
  const { data: activeRules, mutate: mutateActive } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const activeIds = new Set((activeRules || []).map((r) => r.id))
  const activeRuleBased = (activeRules || []).find((r) => {
    const t = (r.algorithm_data || r.algorithm)?.type
    return t && t !== 'volume_split'
  })

  const volumeRules = (allRules || [])
    .filter((r) => (r.algorithm_data || r.algorithm)?.type === 'volume_split')
    // Sorted by the same stamp the Last Modified column shows, so the order matches what is read.
    .sort((a, b) => lastModifiedMs(b) - lastModifiedMs(a))

  const gatewayOptions = Array.from(new Set(volumeRules.flatMap(splitGateways))).sort()

  const visibleRules = volumeRules.filter((algo) => {
    if (statusFilter === 'active' && !activeIds.has(algo.id)) return false
    if (statusFilter === 'inactive' && activeIds.has(algo.id)) return false
    if (gatewayFilter !== 'all' && !splitGateways(algo).includes(gatewayFilter)) return false
    if (nameFilter.trim()) {
      const needle = nameFilter.trim().toLowerCase()
      if (!algo.name.toLowerCase().includes(needle) && !algo.id.toLowerCase().includes(needle)) {
        return false
      }
    }
    return true
  })

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  async function handleActivate(id: string) {
    if (!merchantId) return
    if (activeRuleBased) {
      setPendingActivateId(id)
      return
    }
    await doActivate(id)
  }

  async function doActivate(id: string) {
    setActivating(true)
    setActionError(null)
    setActionSuccess(null)
    try {
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: id })
      setActionSuccess('Rule activated.')
      await Promise.all([mutateRules(), mutateActive()])
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'Failed to activate')
    } finally {
      setActivating(false)
    }
  }

  async function doDeactivate(id: string) {
    setDeactivatingId(id)
    setActionError(null)
    setActionSuccess(null)
    try {
      await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: id })
      setActionSuccess('Rule deactivated.')
      await Promise.all([mutateRules(), mutateActive()])
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'Failed to deactivate')
    } finally {
      setDeactivatingId(null)
    }
  }

  async function doDelete(id: string) {
    if (!merchantId) return
    setActionError(null)
    setActionSuccess(null)
    try {
      await apiPost('/routing/delete', { created_by: merchantId, routing_algorithm_id: id })
      setActionSuccess('Rule deleted.')
      await Promise.all([mutateRules(), mutateActive()])
    } catch (err: unknown) {
      setActionError(err instanceof Error ? err.message : 'Failed to delete')
    } finally {
      setPendingDeleteId(null)
    }
  }

  const pendingDeleteRule = volumeRules.find((r) => r.id === pendingDeleteId)

  return (
    <div className="space-y-6">
      <ConfirmDialog
        open={pendingActivateId !== null}
        title="Switch to Volume Split Routing?"
        description={`"${activeRuleBased?.name}" is currently active. Activating this rule will replace it.`}
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
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Volume Split Routing</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8d96a8]">
            Divide payment traffic across gateways by fixed percentages, independent of transaction
            attributes.
          </p>
        </div>
        <Button onClick={() => navigate('/routing/volume/new')} disabled={!canEditRouting}>
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
          <p className="px-4 py-6 text-sm text-slate-400">Set merchant ID to load rules.</p>
        ) : !allRules ? (
          <p className="px-4 py-6 text-sm text-slate-400">Loading...</p>
        ) : volumeRules.length === 0 ? (
          <p className="px-4 py-6 text-sm text-slate-400">No volume split rules yet.</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[880px] text-left">
              <thead>
                <tr className="border-b border-slate-100 text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:border-[#1e2330] dark:text-[#6d7a8d]">
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
                  <th className="px-5 py-3.5">
                    <HeaderFilter
                      label="Distribution"
                      value={gatewayFilter}
                      options={[
                        { value: 'all', label: 'All gateways' },
                        ...gatewayOptions.map((g) => ({ value: g, label: g })),
                      ]}
                      onChange={setGatewayFilter}
                      ariaLabel="Filter by gateway"
                    />
                  </th>
                  <th className="px-5 py-3.5">Last Modified</th>
                  <th className="px-5 py-3.5 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {visibleRules.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-4 py-8 text-center">
                      <p className="text-sm text-slate-400">No rules match these filters.</p>
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
                  const isExpanded = expandedRuleIds.has(algo.id)
                  const distribution = describeSplit(algo)
                  // Both /routing/update and /routing/delete reject an active algorithm.
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
                        isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                      }`}
                    >
                      <td className="px-5 py-4">
                        <div className="flex items-start gap-2">
                          {isExpanded
                            ? <ChevronDown size={14} className="mt-1 shrink-0 text-slate-400" />
                            : <ChevronRight size={14} className="mt-1 shrink-0 text-slate-400" />
                          }
                          <div className="min-w-0">
                            <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                              {algo.name}
                            </p>
                            <p className="mt-0.5 truncate font-mono text-xs text-slate-400 dark:text-[#6d7a8d]">
                              {algo.id}
                            </p>
                          </div>
                        </div>
                      </td>
                      <td className="px-5 py-4">
                        <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold ${
                          isActive
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                            : 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                        }`}>
                          {isActive ? 'Active' : 'Inactive'}
                        </span>
                      </td>
                      <td className="max-w-[320px] px-5 py-4">
                        <p className="truncate text-sm font-medium text-slate-800 dark:text-slate-200" title={distribution}>
                          {distribution}
                        </p>
                      </td>
                      <td className="whitespace-nowrap px-5 py-4 text-[13px] text-slate-500 dark:text-[#6d7a8d]">
                        {(() => {
                          const stamp = formatLastModified(algo)
                          if (!stamp) return '—'
                          return (
                            <span title={stamp.full}>
                              {stamp.date}
                              <span className="block text-[12px] text-slate-400 dark:text-[#55627a]">{stamp.time}</span>
                            </span>
                          )
                        })()}
                      </td>
                      <td className="px-5 py-4" onClick={(e) => e.stopPropagation()}>
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
                              onSelect: () => navigate(`/routing/volume/${algo.id}/edit`),
                              disabled: Boolean(lockedReason),
                              hint: lockedReason,
                            },
                            {
                              label: 'Duplicate',
                              icon: Copy,
                              onSelect: () => navigate(`/routing/volume/new?cloneFrom=${algo.id}`),
                              disabled: !canEditRouting,
                            },
                            {
                              label: 'Delete',
                              icon: Trash2,
                              tone: 'danger',
                              onSelect: () => setPendingDeleteId(algo.id),
                              disabled: Boolean(lockedReason),
                              hint: lockedReason,
                            },
                          ]}
                        />
                      </td>
                    </tr>,
                    isExpanded ? (
                      <tr key={`${algo.id}-detail`} className="border-b border-slate-100 dark:border-[#1e2330]">
                        <td colSpan={5} className="bg-slate-50/60 px-6 py-5 dark:bg-[#0c0f17]">
                          {algo.description && algo.description !== 'N/A' && (
                            <p className="mb-3 text-sm text-slate-500 dark:text-[#6d7a8d]">{algo.description}</p>
                          )}
                          <SplitBreakdown gateways={toVolumeSplitRuleDetailsState(algo)?.gateways ?? []} />
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

      {activeRuleBased && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
          <strong>"{activeRuleBased.name}" is active</strong> — activating a volume split rule will
          automatically deactivate it.
        </div>
      )}
    </div>
  )
}
