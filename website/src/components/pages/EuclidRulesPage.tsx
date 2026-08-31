import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { Plus, ChevronDown, ChevronRight, Zap, PowerOff, Pencil, Copy, Trash2 } from 'lucide-react'
import { Card } from '../ui/Card'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { formatLastModified, lastModifiedMs } from '../../lib/routingRuleTimestamps'
import { RoutingAlgorithm } from '../../types/api'
import { RuleBreakdown } from '../routing/euclid/RuleBreakdown'
import {
  summarizeConditions, summarizeDestination, destinationGateways,
} from '../../features/routing/euclid/summarize'

import { PageHeading } from '../ui/PageHeading'
import { Notice } from '../ui/Notice'
type StatusFilter = 'all' | 'active' | 'inactive'

export function EuclidRulesPage() {
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

  const { data: allAlgorithms, mutate: mutateAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )

  const { data: activeAlgorithms, mutate: mutateActiveAlgorithms } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const activeIds = new Set((activeAlgorithms || []).map((a) => a.id))
  const activeVolumeAlgorithm = (activeAlgorithms || []).find(
    (a) => (a.algorithm_data || a.algorithm)?.type === 'volume_split'
  )
  const activeAbTestAlgorithm = (activeAlgorithms || []).find(
    (a) => (a.algorithm_data || a.algorithm)?.type === 'ab_test'
  )
  const abTestArmIds = activeAbTestAlgorithm
    ? (() => {
        const d = (activeAbTestAlgorithm.algorithm_data || activeAbTestAlgorithm.algorithm)?.data as
          | { control_algorithm_id?: string; variant_algorithm_id?: string }
          | undefined
        return new Set([d?.control_algorithm_id, d?.variant_algorithm_id].filter(Boolean) as string[])
      })()
    : new Set<string>()

  const ruleAlgorithms = (allAlgorithms || [])
    .filter((algo) => {
      const algorithm = algo.algorithm_data || algo.algorithm
      return algorithm?.type !== 'volume_split' && algorithm?.type !== 'ab_test'
    })
    // Sorted by the same stamp the Last Modified column shows, so the order matches what is read.
    .sort((a, b) => lastModifiedMs(b) - lastModifiedMs(a))

  const gatewayOptions = Array.from(new Set(ruleAlgorithms.flatMap(destinationGateways))).sort()

  const visibleRules = ruleAlgorithms.filter((algo) => {
    if (statusFilter === 'active' && !activeIds.has(algo.id)) return false
    if (statusFilter === 'inactive' && activeIds.has(algo.id)) return false
    if (gatewayFilter !== 'all' && !destinationGateways(algo).includes(gatewayFilter)) return false
    if (nameFilter.trim()) {
      const needle = nameFilter.trim().toLowerCase()
      if (!algo.name.toLowerCase().includes(needle) && !algo.id.toLowerCase().includes(needle)) {
        return false
      }
    }
    return true
  })

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds(prev => {
      const newSet = new Set(prev)
      if (newSet.has(id)) { newSet.delete(id) } else { newSet.add(id) }
      return newSet
    })
  }

  async function handleActivate(id: string) {
    if (!merchantId) return
    if (activeVolumeAlgorithm) {
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
      setActionSuccess('Rule activated successfully.')
      await Promise.all([mutateAlgorithms(), mutateActiveAlgorithms()])
    } catch (err) {
      setActionError(String(err))
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
      setActionSuccess('Rule deactivated successfully.')
      await Promise.all([mutateAlgorithms(), mutateActiveAlgorithms()])
    } catch (err) {
      setActionError(String(err))
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
      await Promise.all([mutateAlgorithms(), mutateActiveAlgorithms()])
    } catch (err) {
      setActionError(String(err))
    } finally {
      setPendingDeleteId(null)
    }
  }

  const pendingDeleteRule = ruleAlgorithms.find((a) => a.id === pendingDeleteId)

  return (
    <div className="space-y-6">
      <ConfirmDialog
        open={pendingActivateId !== null}
        title="Switch to Rule-Based Routing?"
        description={`"${activeVolumeAlgorithm?.name}" (Volume Split) is currently active. Activating this rule will replace it.`}
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
          <PageHeading title="Rule-Based Routing" description="Create conditions to route card and alternative payment transactions dynamically across multiple active gateways." />
        </div>
        <Button onClick={() => navigate('/routing/rules/new')} disabled={!canEditRouting}>
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
        ) : !allAlgorithms ? (
          <p className="px-4 py-6 text-sm text-slate-500">Loading...</p>
        ) : ruleAlgorithms.length === 0 ? (
          <p className="px-4 py-6 text-sm text-slate-500">No rule-based rules yet.</p>
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
                  <th className="px-5 py-3.5">Conditions Overview</th>
                  <th className="px-5 py-3.5">
                    <HeaderFilter
                      label="Destination Gateway"
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
                    <td colSpan={6} className="px-4 py-8 text-center">
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
                  const isInAbTest = abTestArmIds.has(algo.id)
                  const isExpanded = expandedRuleIds.has(algo.id)
                  const conditions = summarizeConditions(algo)
                  const destination = summarizeDestination(algo)
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
                        isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : isInAbTest ? 'bg-purple-50/40 dark:bg-purple-900/10' : ''
                      }`}
                    >
                      <td className="align-top px-5 py-4">
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
                      <td className="align-top px-5 py-4">
                        <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold ${
                          isActive
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                            : 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                        } leading-4`}>
                          {isActive ? 'Active' : 'Inactive'}
                        </span>
                        {isInAbTest && (
                          <span className="mt-1.5 block text-[11px] font-medium text-purple-600 dark:text-purple-400 leading-4">
                            In A/B test
                          </span>
                        )}
                      </td>
                      <td className="max-w-[420px] px-5 py-4 align-top">
                        <p className="break-words font-mono text-xs leading-[18px] text-slate-600 dark:text-[#8d96a8]" title={conditions}>
                          {conditions}
                        </p>
                      </td>
                      <td className="max-w-[300px] px-5 py-4 align-top">
                        <p className="break-words text-sm font-medium leading-5 text-slate-800 dark:text-slate-200" title={destination}>
                          {destination}
                        </p>
                      </td>
                      <td className="align-top whitespace-nowrap px-5 py-4 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
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
                      <td className="align-top px-5 py-4" onClick={(e) => e.stopPropagation()}>
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
                              onSelect: () => navigate(`/routing/rules/${algo.id}/edit`),
                              disabled: Boolean(lockedReason),
                              hint: lockedReason,
                            },
                            {
                              label: 'Duplicate',
                              icon: Copy,
                              onSelect: () => navigate(`/routing/rules/new?cloneFrom=${algo.id}`),
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
                        <td colSpan={6} className="bg-slate-50/60 px-6 py-5 dark:bg-[#0c0f17]">
                          {algo.description && algo.description !== 'N/A' && (
                            <p className="mb-3 text-sm text-slate-500 dark:text-[#78849a]">{algo.description}</p>
                          )}
                          <RuleBreakdown algo={algo} />
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

      {activeVolumeAlgorithm && (
        <Notice tone="warning">
          <strong>Volume Split is active</strong> — activating a rule-based rule will automatically deactivate it.
        </Notice>
      )}
      {activeAbTestAlgorithm && (
        <Notice tone="info">
          <strong>A/B experiment "{activeAbTestAlgorithm.name}" is active</strong> — rules marked "In A/B test" are used as experiment arms. Activating a rule directly will stop the experiment.
        </Notice>
      )}
    </div>
  )
}
