import { useState } from 'react'
import useSWR, { useSWRConfig } from 'swr'
import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from 'recharts'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { CHART_TOOLTIP_ITEM_STYLE, CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import { RoutingAlgorithm } from '../../types/api'
import { Plus, Trash2, PowerOff, ChevronDown, ChevronUp } from 'lucide-react'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { validateVolumeSplitRule } from '../../features/routing/volumeSplit/schema'
import { toVolumeSplitCreatePayload } from '../../features/routing/volumeSplit/payload'
import { toVolumeSplitRuleDetailsState } from '../../features/routing/volumeSplit/state'
import { VolumeSplitGatewayFormEntry } from '../../features/routing/volumeSplit/types'

const COLORS = ['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899']

function makeId() { return Math.random().toString(36).slice(2) }

function createInitialGateways(): VolumeSplitGatewayFormEntry[] {
  return [
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
  ]
}

function clampSplit(value: number) {
  if (!Number.isFinite(value)) return 0
  return Math.min(100, Math.max(0, Math.round(value)))
}

function withInferredSplit(entries: VolumeSplitGatewayFormEntry[]) {
  if (!entries.length) return entries

  const normalized = entries.map(entry => ({
    ...entry,
    split: clampSplit(entry.split),
  }))

  if (normalized.length === 1) {
    return [{ ...normalized[0], split: 100 }]
  }

  const inferredIndex = normalized.length - 1
  const fixedTotal = normalized
    .slice(0, inferredIndex)
    .reduce((sum, gateway) => sum + gateway.split, 0)

  return normalized.map((entry, index) =>
    index === inferredIndex
      ? { ...entry, split: Math.max(0, 100 - fixedTotal) }
      : entry,
  )
}

export function VolumeSplitPage() {
  const { merchantId } = useMerchantStore()
  const { mutate: mutateCache } = useSWRConfig()

  const { data: active, mutate: mutateActive } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['active-routing', merchantId] : null,
    () => apiPost(`/routing/list/active/${merchantId}`)
  )

  const activeVol = active?.find(r => (r.algorithm_data || r.algorithm)?.type === 'volume_split')
  const activeRuleBased = active?.find(r => {
    const t = (r.algorithm_data || r.algorithm)?.type
    return t && t !== 'volume_split'
  })

  const [gateways, setGateways] = useState<VolumeSplitGatewayFormEntry[]>(() => createInitialGateways())
  const [ruleName, setRuleName] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)
  const [expandedRuleIds, setExpandedRuleIds] = useState<Set<string>>(new Set())
  const [deactivatingRuleId, setDeactivatingRuleId] = useState<string | null>(null)
  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)

  const inferredGatewayId = gateways[gateways.length - 1]?.id ?? null
  const fixedTotal = inferredGatewayId
    ? gateways
        .filter(gateway => gateway.id !== inferredGatewayId)
        .reduce((sum, gateway) => sum + gateway.split, 0)
    : 0
  const overAllocated = Math.max(0, fixedTotal - 100)
  const total = gateways.reduce((s, g) => s + g.split, 0)

  function updateGateway(id: string, field: 'gatewayName' | 'gatewayId' | 'split', val: string | number) {
    setGateways(gs =>
      withInferredSplit(
        gs.map(g => {
          if (g.id !== id) return g
          if (field === 'split') {
            return { ...g, split: clampSplit(Number(val)) }
          }
          return { ...g, [field]: val }
        }),
      ),
    )
  }

  function addGateway() {
    setGateways(gs => withInferredSplit([...gs, { id: makeId(), gatewayName: '', gatewayId: '', split: 0 }]))
  }

  function removeGateway(id: string) {
    setGateways(gs => {
      const remaining = gs.filter(g => g.id !== id)
      return withInferredSplit(
        remaining.length
          ? remaining
          : [{ id: makeId(), gatewayName: '', gatewayId: '', split: 100 }],
      )
    })
  }

  async function handleCreate() {
    if (!merchantId) return setError('Set a merchant ID first')
    const validationError = validateVolumeSplitRule({ ruleName, gateways })
    if (validationError) return setError(validationError)

    setSaving(true); setError(null); setSuccess(null); setCreatedId(null)
    try {
      const nextRuleName = ruleName.trim()
      const payload = toVolumeSplitCreatePayload({ ruleName, gateways }, merchantId)
      const result = await apiPost<RoutingAlgorithm>('/routing/create', payload)
      await Promise.all([
        mutateActive(),
        mutateCache(['routing-list', merchantId]),
      ])
      setCreatedId(result.rule_id ?? result.id)
      setSuccess(`Rule "${nextRuleName}" created successfully. Configurator reset.`)
      setRuleName('')
      setGateways(createInitialGateways())
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to create rule')
    } finally {
      setSaving(false)
    }
  }

  async function handleActivate(ruleId: string) {
    if (!merchantId) return
    setSuccess(null)
    setCreatedId(null)
    if (activeRuleBased) {
      setPendingActivateId(ruleId)
      return
    }
    await doActivate(ruleId)
  }

  async function doActivate(ruleId: string) {
    try {
      setError(null)
      setSuccess(null)
      setCreatedId(null)
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: ruleId })
      await Promise.all([
        mutateActive(),
        mutateCache(['routing-list', merchantId]),
      ])
      setSuccess('Rule activated.')
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to activate')
    }
  }

  async function handleDeactivate(ruleId: string) {
    if (!merchantId) return
    setPendingDeactivateId(ruleId)
  }

  async function doDeactivate(ruleId: string) {
    setDeactivatingRuleId(ruleId)
    setError(null)
    setSuccess(null)
    setCreatedId(null)
    try {
      await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: ruleId })
      await Promise.all([
        mutateActive(),
        mutateCache(['routing-list', merchantId]),
      ])
      setSuccess('Rule deactivated.')
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to deactivate')
    } finally {
      setDeactivatingRuleId(null)
    }
  }

  function toggleRuleExpand(id: string) {
    setExpandedRuleIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <div className="space-y-6">
      <ConfirmDialog
        open={pendingActivateId !== null}
        title="Switch to Volume Split Routing?"
        description={`"${activeRuleBased?.name}" (Rule-Based) is currently active. Activating this rule will replace it.`}
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

      <div>
        <h1 className="text-lg font-semibold text-slate-900 dark:text-white">Volume Split Routing</h1>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">

        {/* ── Left: saved rules list ───────────────────────────── */}
        <div className="space-y-3 lg:col-span-1">
          <SavedRulesList
            merchantId={merchantId}
            activeRuleId={activeVol?.id}
            onActivate={handleActivate}
            onDeactivate={handleDeactivate}
            deactivatingRuleId={deactivatingRuleId}
            expandedRuleIds={expandedRuleIds}
            onToggleExpand={toggleRuleExpand}
          />
          {activeRuleBased && (
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
              <strong>Rule-Based routing is active</strong> — activating a volume split rule will automatically deactivate it.
            </div>
          )}
          {error && <ErrorMessage error={error} />}
          {success && (
            <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-200">
              <span className="min-w-0">
                {createdId ? <>Rule created: <span className="font-mono">{createdId}</span></> : success}
              </span>
              {createdId ? (
                <Button type="button" size="sm" onClick={() => handleActivate(createdId)}>
                  Activate Now
                </Button>
              ) : null}
            </div>
          )}
        </div>

        {/* ── Right: create form ───────────────────────────────── */}
        <div className="space-y-4 lg:col-span-2">
          <Card>
            <CardHeader>
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Create Volume Split Rule</h2>
            </CardHeader>
            <CardBody className="space-y-4">
              <div>
                <label className="mb-1 block text-xs text-slate-500 dark:text-[#8a8a93]">Rule Name *</label>
                <input
                  value={ruleName}
                  onChange={e => setRuleName(e.target.value)}
                  placeholder="e.g. ab-test-split"
                  className="w-64 rounded-lg border border-slate-200 bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                />
              </div>

              <div className="space-y-2">
                <div className="hidden grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(260px,320px)_32px] gap-2 px-1 text-xs font-medium text-slate-500 md:grid">
                  <span>Gateway Name</span>
                  <span>Gateway ID</span>
                  <span>Split %</span>
                  <span />
                </div>
                {gateways.map((g, index) => {
                  const isInferred = g.id === inferredGatewayId
                  const label = g.gatewayName.trim() || `Gateway ${index + 1}`
                  return (
                    <div key={g.id} className="grid grid-cols-1 gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(260px,320px)_32px] md:items-center">
                      <input
                        value={g.gatewayName}
                        onChange={e => updateGateway(g.id, 'gatewayName', e.target.value)}
                        placeholder="e.g. stripe"
                        className="rounded-lg border border-slate-200 bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                      />
                      <input
                        value={g.gatewayId}
                        onChange={e => updateGateway(g.id, 'gatewayId', e.target.value)}
                        placeholder="optional gateway_id"
                        className="rounded-lg border border-slate-200 bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                      />
                      <div className="flex min-w-0 items-center gap-2 rounded-lg border border-slate-200 bg-transparent px-2 py-1.5 focus-within:ring-1 focus-within:ring-brand-500 dark:border-[#222226]">
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: COLORS[index % COLORS.length] }}
                        />
                        <input
                          type="range"
                          min={0}
                          max={100}
                          value={g.split}
                          disabled={isInferred}
                          onChange={e => updateGateway(g.id, 'split', Number(e.target.value))}
                          aria-label={`${label} allocation slider`}
                          className="h-2 min-w-0 flex-1 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                          style={{ accentColor: COLORS[index % COLORS.length] }}
                        />
                        <input
                          type="number"
                          min={0}
                          max={100}
                          value={g.split}
                          onChange={e => updateGateway(g.id, 'split', Number(e.target.value))}
                          disabled={isInferred}
                          aria-label={`${label} split percentage`}
                          className="w-12 border-0 bg-transparent p-0 text-right text-sm tabular-nums focus:outline-none disabled:cursor-not-allowed disabled:opacity-70"
                        />
                        <span className="text-xs text-slate-500">%</span>
                        {isInferred && gateways.length > 1 && (
                          <span className="rounded-full bg-slate-200 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-slate-600 dark:bg-[#1a1a22] dark:text-slate-300">
                            Auto
                          </span>
                        )}
                      </div>
                      <button
                        type="button"
                        onClick={() => removeGateway(g.id)}
                        disabled={gateways.length === 1}
                        className="text-slate-400 hover:text-red-500 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  )
                })}
                <div className="flex items-center gap-3">
                  <button type="button" onClick={addGateway} className="flex items-center gap-1 text-sm text-brand-500 hover:text-brand-600">
                    <Plus size={14} /> Add Gateway
                  </button>
                  <span className={`text-xs font-medium ${total === 100 ? 'text-emerald-400' : 'text-red-400'}`}>
                    Total: {total}%{overAllocated ? ` (reduce fixed splits by ${overAllocated}%)` : total !== 100 ? ' (must be 100)' : ''}
                  </span>
                </div>
              </div>

              <Button onClick={handleCreate} disabled={saving || !merchantId}>
                {saving ? <><Spinner size={14} /> Creating…</> : 'Create Rule'}
              </Button>
            </CardBody>
          </Card>
        </div>

      </div>
    </div>
  )
}

function SavedRulesList({
  merchantId,
  activeRuleId,
  onActivate,
  onDeactivate,
  deactivatingRuleId,
  expandedRuleIds,
  onToggleExpand,
}: {
  merchantId: string
  activeRuleId?: string
  onActivate: (id: string) => void
  onDeactivate: (id: string) => void
  deactivatingRuleId: string | null
  expandedRuleIds: Set<string>
  onToggleExpand: (id: string) => void
}) {
  const { data: rules, isLoading } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['routing-list', merchantId] : null,
    () => apiPost(`/routing/list/${merchantId}`)
  )

  const volRules = rules?.filter(r => (r.algorithm_data || r.algorithm)?.type === 'volume_split') ?? []

  return (
    <Card>
      <CardHeader>
        <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Saved Rules</h2>
      </CardHeader>
      <div>
        {!merchantId ? (
          <p className="px-6 py-3 text-sm text-slate-400">Set merchant ID to load rules.</p>
        ) : isLoading ? (
          <div className="flex justify-center py-4"><Spinner /></div>
        ) : volRules.length === 0 ? (
          <p className="px-6 py-3 text-sm text-slate-400">No volume split rules yet.</p>
        ) : (
          <div>
            {volRules.map((r) => {
              const isActive = activeRuleId === r.id
              const isExpanded = expandedRuleIds.has(r.id)
              const details = toVolumeSplitRuleDetailsState(r)
              const splitText = details?.gateways
                .map(g => `${g.gatewayName}: ${g.split}%`)
                .join(' · ')

              return (
                <div
                  key={r.id}
                  className={`border-b border-slate-100 dark:border-[#1e2330] last:border-b-0 transition-colors ${
                    isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                  }`}
                >
                  <div className="px-6 pt-3 pb-2">
                    <div className="flex items-center justify-between gap-2">
                      <button
                        type="button"
                        onClick={() => onToggleExpand(r.id)}
                        className="group min-w-0 flex-1 text-left"
                      >
                        <div className="flex items-center gap-1.5">
                          <p className={`truncate font-medium transition-colors group-hover:text-brand-600 dark:group-hover:text-brand-400 ${
                            isActive ? 'text-emerald-900 dark:text-emerald-100' : 'text-slate-900 dark:text-white'
                          }`}>
                            {r.name}
                          </p>
                          {isActive && (
                            <span className="inline-flex shrink-0 items-center gap-0.5 rounded-full bg-emerald-100 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400">
                              ● Active
                            </span>
                          )}
                          {isExpanded
                            ? <ChevronUp size={12} className="ml-auto shrink-0 text-slate-400" />
                            : <ChevronDown size={12} className="ml-auto shrink-0 text-slate-400" />
                          }
                        </div>
                        <div className="mt-0.5 flex items-center gap-2">
                          {splitText && (
                            <p className="min-w-0 truncate text-[11px] text-slate-500 dark:text-[#6d7a8d]">
                              {splitText}
                            </p>
                          )}
                          {r.created_at && (
                            <span className="ml-auto shrink-0 text-[10px] text-slate-400 dark:text-[#4e5870]">
                              {new Date(r.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}
                            </span>
                          )}
                        </div>
                      </button>

                      <div className="flex shrink-0 items-center gap-1.5">
                        {isActive ? (
                          <Button
                            size="sm"
                            variant="danger"
                            onClick={() => onDeactivate(r.id)}
                            disabled={deactivatingRuleId === r.id}
                          >
                            <PowerOff size={13} />
                            {deactivatingRuleId === r.id ? 'Deactivating' : 'Deactivate'}
                          </Button>
                        ) : (
                          <button
                            type="button"
                            onClick={() => onActivate(r.id)}
                            className="inline-flex min-w-[68px] items-center justify-center rounded-full border border-slate-200 bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-500 transition-colors duration-150 hover:border-brand-200 hover:bg-brand-50 hover:text-brand-600 dark:border-[#2a3040] dark:bg-[#1a1f2a] dark:text-[#8090a8] dark:hover:border-brand-800 dark:hover:bg-brand-900/20 dark:hover:text-brand-400"
                          >
                            Activate
                          </button>
                        )}
                      </div>
                    </div>
                  </div>

                  {isExpanded && (() => {
                    const pieData = details?.gateways.map(g => ({
                      name: g.gatewayName + (g.gatewayId ? ` (${g.gatewayId})` : ''),
                      value: g.split,
                    })) ?? []
                    return (
                      <div className="border-t border-slate-100 bg-slate-50/60 px-6 py-4 dark:border-[#1e2330] dark:bg-[#0c0f17]">
                        {pieData.length > 0 && (
                          <ResponsiveContainer width="100%" height={200}>
                            <PieChart>
                              <Pie
                                data={pieData}
                                dataKey="value"
                                nameKey="name"
                                cx="50%"
                                cy="50%"
                                outerRadius={72}
                                isAnimationActive={false}
                                label={({ name, value }) => `${name}: ${value}%`}
                                labelLine={{ stroke: '#45454f' }}
                              >
                                {pieData.map((_, i) => (
                                  <Cell key={i} fill={COLORS[i % COLORS.length]} />
                                ))}
                              </Pie>
                              <Tooltip
                                formatter={(v) => `${v}%`}
                                contentStyle={CHART_TOOLTIP_STYLE}
                                labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                                itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                              />
                            </PieChart>
                          </ResponsiveContainer>
                        )}
                      </div>
                    )
                  })()}
                </div>
              )
            })}
          </div>
        )}
      </div>
    </Card>
  )
}
