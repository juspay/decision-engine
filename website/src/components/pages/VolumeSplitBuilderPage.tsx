import { useEffect, useRef, useState } from 'react'
import { useNavigate, useParams, useSearchParams } from 'react-router-dom'
import useSWR from 'swr'
import { Plus, Trash2, ArrowLeft } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { useCanEditRouting } from '../../store/authStore'
import { apiPost } from '../../lib/api'
import { RoutingAlgorithm } from '../../types/api'
import { VolumeSplitGatewayFormEntry } from '../../features/routing/volumeSplit/types'
import { toVolumeSplitCreatePayload, toVolumeSplitAlgorithm } from '../../features/routing/volumeSplit/payload'
import { toVolumeSplitRuleDetailsState } from '../../features/routing/volumeSplit/state'
import { validateVolumeSplitRule } from '../../features/routing/volumeSplit/schema'
import { SplitBreakdown, SPLIT_COLORS } from '../routing/volumeSplit/SplitBreakdown'
import { GatewaySelect } from '../ui/GatewaySelect'
import { gatewayOptions } from '../../lib/connectors'

function makeId() { return Math.random().toString(36).slice(2) }

function createInitialGateways(): VolumeSplitGatewayFormEntry[] {
  return [
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
    { id: makeId(), gatewayName: '', gatewayId: '', split: 50 },
  ]
}

function clampSplit(value: number) {
  if (Number.isNaN(value)) return 0
  return Math.min(100, Math.max(0, Math.round(value)))
}

/** The last row absorbs whatever the fixed rows leave, so a split always totals 100. */
function withInferredSplit(entries: VolumeSplitGatewayFormEntry[]) {
  if (entries.length === 0) return entries
  const fixed = entries.slice(0, -1)
  const fixedTotal = fixed.reduce((sum, entry) => sum + entry.split, 0)
  const remainder = clampSplit(100 - fixedTotal)
  const last = entries[entries.length - 1]
  return [...fixed, { ...last, split: remainder }]
}

export function VolumeSplitBuilderPage() {
  const canEditRouting = useCanEditRouting()
  const navigate = useNavigate()
  const { id: editId } = useParams<{ id: string }>()
  const [searchParams] = useSearchParams()
  const cloneFromId = searchParams.get('cloneFrom')
  const isEdit = Boolean(editId)

  const { merchantId } = useMerchantStore()

  const [ruleName, setRuleName] = useState('')
  const [ruleDesc, setRuleDesc] = useState('')
  const [gateways, setGateways] = useState<VolumeSplitGatewayFormEntry[]>(() => createInitialGateways())
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const { data: allRules, mutate: mutateRules } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`)
  )
  const { data: activeRules } = useSWR<RoutingAlgorithm[]>(
    merchantId ? `/routing/list/active/${merchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`)
  )

  const sourceId = editId || cloneFromId
  const sourceRule = sourceId ? (allRules || []).find((r) => r.id === sourceId) : undefined
  // /routing/update rejects an active algorithm (ensure_routing_algorithm_inactive), so say so up
  // front rather than letting Save fail.
  const isEditingActiveRule = isEdit && (activeRules || []).some((r) => r.id === editId)

  // Seed from the source rule exactly once — otherwise SWR revalidation would overwrite edits.
  const seededFrom = useRef<string | null>(null)
  useEffect(() => {
    if (!sourceRule || seededFrom.current === sourceRule.id) return
    const details = toVolumeSplitRuleDetailsState(sourceRule)
    if (!details) return
    seededFrom.current = sourceRule.id
    setRuleName(isEdit ? details.name : `copy-of-${details.name}`)
    setRuleDesc(details.description && details.description !== 'N/A' ? details.description : '')
    if (details.gateways.length > 0) setGateways(details.gateways)
  }, [sourceRule, isEdit])

  const inferredGatewayId = gateways[gateways.length - 1]?.id ?? null
  const fixedTotal = inferredGatewayId
    ? gateways.filter((g) => g.id !== inferredGatewayId).reduce((sum, g) => sum + g.split, 0)
    : 0
  const overAllocated = Math.max(0, fixedTotal - 100)
  const total = gateways.reduce((sum, g) => sum + g.split, 0)

  function updateGateway(id: string, field: 'gatewayName' | 'gatewayId' | 'split', val: string | number) {
    setGateways((gs) =>
      withInferredSplit(
        gs.map((g) => {
          if (g.id !== id) return g
          if (field === 'split') return { ...g, split: clampSplit(Number(val)) }
          return { ...g, [field]: val }
        })
      )
    )
  }

  function addGateway() {
    setGateways((gs) => withInferredSplit([...gs, { id: makeId(), gatewayName: '', gatewayId: '', split: 0 }]))
  }

  function removeGateway(id: string) {
    setGateways((gs) => {
      const remaining = gs.filter((g) => g.id !== id)
      return withInferredSplit(
        remaining.length ? remaining : [{ id: makeId(), gatewayName: '', gatewayId: '', split: 100 }]
      )
    })
  }

  function handleClear() {
    setRuleName('')
    setRuleDesc('')
    setGateways(createInitialGateways())
    setError(null)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!merchantId) { setError('Set a merchant ID first'); return }
    const validationError = validateVolumeSplitRule({ ruleName, description: ruleDesc, gateways })
    if (validationError) { setError(validationError); return }

    setSaving(true)
    setError(null)
    try {
      if (isEdit) {
        await apiPost('/routing/update', {
          created_by: merchantId,
          routing_algorithm_id: editId,
          name: ruleName.trim(),
          description: ruleDesc,
          algorithm: toVolumeSplitAlgorithm(gateways),
        })
      } else {
        await apiPost<RoutingAlgorithm>(
          '/routing/create',
          toVolumeSplitCreatePayload({ ruleName, description: ruleDesc, gateways }, merchantId)
        )
      }
      await mutateRules()
      navigate('/routing/volume')
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to save rule')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-6">
      <div className="min-w-0">
        <button
          type="button"
          onClick={() => navigate('/routing/volume')}
          className="mb-2 inline-flex items-center gap-2 text-sm font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
        >
          <ArrowLeft size={16} /> Volume Split Routing
        </button>
        <h1 className="truncate text-2xl font-semibold text-slate-900 dark:text-white">
          {isEdit ? 'Edit Volume Split Rule' : 'Create Volume Split Rule'}
        </h1>
        <p className="mt-1 text-sm text-slate-500 dark:text-[#8d96a8]">
          Divide traffic across gateways by percentage. The last row absorbs whatever the others leave.
        </p>
      </div>

      {isEdit && !sourceRule && allRules && (
        <ErrorMessage error="That rule no longer exists for this merchant." />
      )}
      {isEditingActiveRule && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
          <strong>This rule is active</strong> — active rules cannot be edited. Deactivate it from the
          rules list first, then come back.
        </div>
      )}

      <div className="flex flex-col gap-6 xl:flex-row xl:items-start">
        <form onSubmit={handleSubmit} className="min-w-0 flex-1 space-y-4">
          <Card>
            <CardBody className="space-y-5">
              <div className="grid grid-cols-1 gap-x-12 gap-y-4 md:grid-cols-2">
                <div className="flex items-center gap-2">
                  <label
                    htmlFor="volume-rule-name"
                    className="shrink-0 whitespace-nowrap text-[13px] font-semibold text-slate-700 dark:text-slate-300"
                  >
                    Rule Name *
                  </label>
                  <input
                    id="volume-rule-name"
                    value={ruleName}
                    onChange={(e) => setRuleName(e.target.value)}
                    placeholder="e.g. ab-test-split"
                    className="min-w-0 flex-1 rounded-lg border border-slate-200 bg-transparent px-3.5 py-2.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                  />
                </div>
                <div className="flex items-center gap-2">
                  <label
                    htmlFor="volume-rule-description"
                    className="shrink-0 whitespace-nowrap text-[13px] font-semibold text-slate-700 dark:text-slate-300"
                  >
                    Description
                  </label>
                  <input
                    id="volume-rule-description"
                    value={ruleDesc}
                    onChange={(e) => setRuleDesc(e.target.value)}
                    placeholder="Optional description"
                    className="min-w-0 flex-1 rounded-lg border border-slate-200 bg-transparent px-3.5 py-2.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                  />
                </div>
              </div>

              <div className="space-y-3">
                <p className="border-b border-slate-100 pb-2.5 text-xs font-semibold uppercase tracking-widest text-slate-500 dark:border-[#1c1c24] dark:text-[#8d96a8]">
                  Traffic Distribution
                </p>

                <div className="hidden grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(260px,320px)_32px] gap-3 px-1 text-xs font-medium text-slate-500 md:grid">
                  <span>Gateway Name</span>
                  <span>Gateway ID</span>
                  <span>Split %</span>
                  <span />
                </div>

                {gateways.map((g, index) => {
                  const isInferred = g.id === inferredGatewayId
                  const label = g.gatewayName.trim() || `Gateway ${index + 1}`
                  return (
                    <div
                      key={g.id}
                      className="grid grid-cols-1 gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(260px,320px)_32px] md:items-center"
                    >
                      <GatewaySelect
                        value={g.gatewayName}
                        onChange={(val) => updateGateway(g.id, 'gatewayName', val)}
                        placeholder="e.g. stripe"
                        options={gatewayOptions(
                          [],
                          gateways.filter((other) => other.id !== g.id).map((other) => other.gatewayName)
                        )}
                        className="min-w-0"
                      />
                      <input
                        value={g.gatewayId}
                        onChange={(e) => updateGateway(g.id, 'gatewayId', e.target.value)}
                        placeholder="optional gateway_id"
                        className="rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
                      />
                      <div className="flex min-w-0 items-center gap-2 rounded-lg border border-slate-200 bg-transparent px-2.5 py-2 focus-within:ring-1 focus-within:ring-brand-500 dark:border-[#222226]">
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: SPLIT_COLORS[index % SPLIT_COLORS.length] }}
                        />
                        <input
                          type="range"
                          min={0}
                          max={100}
                          value={g.split}
                          disabled={isInferred}
                          onChange={(e) => updateGateway(g.id, 'split', Number(e.target.value))}
                          aria-label={`${label} allocation slider`}
                          className="h-2 min-w-0 flex-1 cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                          style={{ accentColor: SPLIT_COLORS[index % SPLIT_COLORS.length] }}
                        />
                        <input
                          type="number"
                          min={0}
                          max={100}
                          value={g.split}
                          onChange={(e) => updateGateway(g.id, 'split', Number(e.target.value))}
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
                        aria-label={`Remove ${label}`}
                        className="text-slate-400 hover:text-red-500 disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        <Trash2 size={15} />
                      </button>
                    </div>
                  )
                })}

                <div className="flex items-center gap-4">
                  <button
                    type="button"
                    onClick={addGateway}
                    className="flex items-center gap-1 text-[13px] font-medium text-brand-600 transition-colors hover:text-brand-700 dark:text-brand-400"
                  >
                    <Plus size={14} /> Add Gateway
                  </button>
                  <span className={`text-xs font-medium ${total === 100 ? 'text-emerald-500' : 'text-red-500'}`}>
                    Total: {total}%
                    {overAllocated
                      ? ` (reduce fixed splits by ${overAllocated}%)`
                      : total !== 100 ? ' (must be 100)' : ''}
                  </span>
                </div>
              </div>

              <ErrorMessage error={error} />
            </CardBody>
          </Card>

          <div className="flex flex-wrap items-center gap-4">
            <Button type="submit" disabled={saving || !merchantId || !canEditRouting || isEditingActiveRule}>
              {saving ? <><Spinner size={14} /> Saving…</> : isEdit ? 'Save Changes' : 'Create Rule'}
            </Button>
            <Button type="button" variant="secondary" size="sm" onClick={handleClear}>
              Clear
            </Button>
            <button
              type="button"
              onClick={() => navigate('/routing/volume')}
              className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-800 dark:text-[#8d96a8] dark:hover:text-white"
            >
              Cancel
            </button>
            <p className="text-sm text-slate-400 dark:text-[#6d7a8d]">
              {isEdit
                ? 'Saved changes apply the next time this rule is activated.'
                : 'A new rule is created inactive — activate it from the rules list.'}
            </p>
          </div>
        </form>

        <aside className="w-full shrink-0 xl:w-[24rem]">
          <div className="xl:sticky xl:top-6">
            <Card>
              <CardBody className="space-y-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-widest text-slate-500 dark:text-[#8d96a8]">
                    Rule details
                  </p>
                  <p className="mt-1 truncate text-sm font-semibold text-slate-800 dark:text-slate-100">
                    {ruleName.trim() || 'Untitled rule'}
                  </p>
                </div>
                <SplitBreakdown gateways={gateways} />
              </CardBody>
            </Card>
          </div>
        </aside>
      </div>
    </div>
  )
}
