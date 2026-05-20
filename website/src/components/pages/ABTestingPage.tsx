import { useState } from 'react'
import useSWR, { useSWRConfig } from 'swr'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost, fetcher } from '../../lib/api'
import {
  RoutingAlgorithm,
  ABTestAlgorithmData,
  ExperimentResultsResponse,
} from '../../types/api'
import { ShieldAlert, ChevronDown, ChevronRight, PowerOff } from 'lucide-react'
import { validateABTestForm } from '../../features/routing/abTesting/schema'
import { toABTestCreatePayload } from '../../features/routing/abTesting/payload'
import { ABTestFormValues } from '../../features/routing/abTesting/types'

const SAMPLE_SIZE_PRESETS = [1000, 5000, 10000, 50000]

function deltaLabel(deltaPp: number) {
  const sign = deltaPp > 0 ? '+' : ''
  return `${sign}${deltaPp.toFixed(2)}pp`
}

function authRatePct(rate: number) {
  return `${(rate * 100).toFixed(2)}%`
}

interface LiveResultsProps {
  algorithm: RoutingAlgorithm
  merchantId: string
}

function LiveResults({ algorithm, merchantId }: LiveResultsProps) {
  const abData = (algorithm.algorithm_data || algorithm.algorithm)?.data as ABTestAlgorithmData | undefined

  const resultsUrl = `/analytics/experiment/${algorithm.id}/results?min_sample_size=${abData?.min_sample_size ?? 1000}&guardrail_threshold_pp=${abData?.guardrail_threshold_pp ?? 3}`
  const { data: results, isLoading, error } = useSWR<ExperimentResultsResponse>(
    merchantId ? resultsUrl : null,
    fetcher,
    { refreshInterval: 60_000 },
  )

  if (isLoading) {
    return (
      <p className="mt-2 text-[11px] text-slate-400 italic">Loading stats…</p>
    )
  }

  if (error || !results) {
    return (
      <p className="mt-2 text-[11px] text-slate-400 italic">
        Stats unavailable — analytics pipeline may not be configured in this environment.
      </p>
    )
  }

  const totalTxns = results.control.transaction_count + results.variant.transaction_count
  const progress = Math.min(100, Math.round((totalTxns / results.min_sample_size) * 100))

  return (
    <div className="mt-3 space-y-3">
      {/* Progress */}
      <div>
        <div className="flex justify-between text-xs text-slate-500 mb-1">
          <span>Transactions collected</span>
          <span>{totalTxns.toLocaleString()} / {results.min_sample_size.toLocaleString()}</span>
        </div>
        <div className="h-1.5 bg-slate-100 dark:bg-slate-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-brand-500 rounded-full transition-all duration-500"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* Arm metrics */}
      <div className="grid grid-cols-3 gap-2">
        <div className="rounded-lg border border-slate-200 dark:border-[#222226] px-3 py-2">
          <div className="text-[10px] text-slate-400 mb-0.5">Control ({100 - (abData?.variant_split_pct ?? 10)}%)</div>
          <div className="text-sm font-semibold text-slate-800 dark:text-slate-100">{authRatePct(results.control.auth_rate)}</div>
          <div className="text-[10px] text-slate-400">{results.control.transaction_count.toLocaleString()} txns</div>
        </div>
        <div className="rounded-lg border border-slate-200 dark:border-[#222226] px-3 py-2">
          <div className="text-[10px] text-slate-400 mb-0.5">Variant ({abData?.variant_split_pct ?? 10}%)</div>
          <div className="text-sm font-semibold text-slate-800 dark:text-slate-100">{authRatePct(results.variant.auth_rate)}</div>
          <div className="text-[10px] text-slate-400">{results.variant.transaction_count.toLocaleString()} txns</div>
        </div>
        <div className="rounded-lg border border-slate-200 dark:border-[#222226] px-3 py-2">
          <div className="text-[10px] text-slate-400 mb-0.5">Delta</div>
          <div className={`text-sm font-semibold ${results.delta_pp > 0 ? 'text-emerald-600 dark:text-emerald-400' : results.delta_pp < 0 ? 'text-red-500' : 'text-slate-800 dark:text-slate-100'}`}>
            {deltaLabel(results.delta_pp)}
          </div>
          {results.p_value !== null && (
            <div className="text-[10px] text-slate-400">p = {results.p_value.toFixed(4)}</div>
          )}
        </div>
      </div>

      {/* Guardrail warning */}
      {results.verdict === 'guardrail_breached' && (
        <div className="flex items-center gap-2 rounded-lg border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 px-3 py-2 text-xs text-red-600 dark:text-red-400">
          <ShieldAlert size={12} />
          Variant dropped beyond guardrail ({abData?.guardrail_threshold_pp}pp). Consider stopping.
        </div>
      )}
    </div>
  )
}

export function ABTestingPage() {
  const { merchantId } = useMerchantStore()
  const { mutate: mutateCache } = useSWRConfig()

  const { data: allAlgorithms, mutate: mutateAll } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['routing-list', merchantId] : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`),
  )
  const { data: activeAlgorithms, mutate: mutateActive } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['active-routing', merchantId] : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${merchantId}`),
  )

  const activeAbTest = activeAlgorithms?.find(
    (r) => (r.algorithm_data || r.algorithm)?.type === 'ab_test',
  )
  const savedAbTests = allAlgorithms?.filter(
    (r) => (r.algorithm_data || r.algorithm)?.type === 'ab_test',
  ) ?? []
  const eligibleAlgorithms = allAlgorithms?.filter(
    (r) => (r.algorithm_data || r.algorithm)?.type !== 'ab_test',
  ) ?? []

  // Form state
  const [form, setForm] = useState<ABTestFormValues>({
    name: '',
    controlAlgorithmId: '',
    variantAlgorithmId: '',
    variantSplitPct: 10,
    minSampleSize: 5000,
    guardrailThresholdPp: 3,
  })
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)

  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set())

  function toggleExpand(id: string) {
    setExpandedIds(prev => {
      const next = new Set(prev)
      next.has(id) ? next.delete(id) : next.add(id)
      return next
    })
  }

  async function handleCreate() {
    if (!merchantId) return
    const validationError = validateABTestForm(form)
    if (validationError) { setError(validationError); return }
    setSaving(true)
    setError(null)
    setSuccess(null)
    try {
      const payload = toABTestCreatePayload(form, merchantId)
      const result = await apiPost<RoutingAlgorithm>('/routing/create', payload)
      const id = result.rule_id || result.id
      setCreatedId(id)
      setSuccess(`"${form.name}" created.`)
      setForm({ name: '', controlAlgorithmId: '', variantAlgorithmId: '', variantSplitPct: 10, minSampleSize: 5000, guardrailThresholdPp: 3 })
      await mutateAll()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to create experiment')
    } finally {
      setSaving(false)
    }
  }

  async function handleActivate(id: string) {
    if (activeAbTest && activeAbTest.id !== id) { setPendingActivateId(id); return }
    await doActivate(id)
  }

  async function doActivate(id: string) {
    if (!merchantId) return
    try {
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: id })
      await Promise.all([mutateActive(), mutateAll()])
      setCreatedId(null)
      setSuccess(null)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to activate experiment')
    }
  }

  async function doDeactivate(id: string) {
    if (!merchantId) return
    try {
      await apiPost('/routing/deactivate', { created_by: merchantId, routing_algorithm_id: id })
      await Promise.all([mutateActive(), mutateAll(), mutateCache(['active-routing', merchantId])])
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to stop experiment')
    } finally {
      setPendingDeactivateId(null)
    }
  }

  function algorithmName(id: string) {
    return allAlgorithms?.find(a => a.id === id)?.name ?? id
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">A/B Testing</h1>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">

        {/* Left: existing experiments */}
        <div className="lg:col-span-1 space-y-3">
          <Card>
            <CardHeader>
              <h2 className="text-sm font-semibold text-slate-800">Experiments</h2>
            </CardHeader>
            <div>
              {!merchantId ? (
                <p className="px-4 py-3 text-sm text-slate-400">Set merchant ID to load experiments.</p>
              ) : !allAlgorithms ? (
                <p className="px-4 py-3 text-sm text-slate-400">Loading…</p>
              ) : savedAbTests.length === 0 ? (
                <p className="px-4 py-3 text-sm text-slate-400">No experiments yet.</p>
              ) : (
                <div>
                  {savedAbTests.map((algo) => {
                    const abData = (algo.algorithm_data || algo.algorithm)?.data as ABTestAlgorithmData | undefined
                    const isActive = activeAbTest?.id === algo.id
                    const isExpanded = expandedIds.has(algo.id)

                    return (
                      <div
                        key={algo.id}
                        className={`border-b border-slate-100 dark:border-[#1e2330] last:border-b-0 transition-colors ${
                          isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                        }`}
                      >
                        <div className="px-4 pt-3 pb-2">
                          <div className="flex items-center justify-between gap-2">
                            <button
                              type="button"
                              onClick={() => toggleExpand(algo.id)}
                              className="min-w-0 flex-1 text-left group"
                            >
                              <div className="flex items-center gap-1.5">
                                <p className={`truncate font-medium group-hover:text-brand-600 dark:group-hover:text-brand-400 transition-colors ${
                                  isActive ? 'text-emerald-900 dark:text-emerald-100' : 'text-slate-900 dark:text-white'
                                }`}>
                                  {algo.name}
                                </p>
                                {isActive && (
                                  <span className="shrink-0 inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400">
                                    ● Active
                                  </span>
                                )}
                                {isExpanded
                                  ? <ChevronDown size={12} className="text-slate-400 shrink-0 ml-auto" />
                                  : <ChevronRight size={12} className="text-slate-400 shrink-0 ml-auto" />
                                }
                              </div>
                              {abData && (
                                <p className="text-[11px] text-slate-400 mt-0.5 truncate">
                                  {100 - abData.variant_split_pct}/{abData.variant_split_pct} split
                                </p>
                              )}
                            </button>

                            <div className="flex items-center gap-1.5 shrink-0">
                              {!isActive ? (
                                <button
                                  type="button"
                                  onClick={() => handleActivate(algo.id)}
                                  className="group/badge relative inline-flex items-center justify-center min-w-[68px] px-2.5 py-0.5 rounded-full text-xs font-medium border transition-colors duration-150
                                    border-slate-200 dark:border-slate-700 text-slate-500 dark:text-slate-400
                                    hover:border-emerald-400 hover:text-emerald-600 hover:bg-emerald-50 dark:hover:border-emerald-600 dark:hover:text-emerald-400 dark:hover:bg-emerald-900/20"
                                >
                                  <span className="transition-opacity duration-150 group-hover/badge:opacity-0">Inactive</span>
                                  <span className="absolute inset-0 flex items-center justify-center gap-1 opacity-0 transition-opacity duration-150 group-hover/badge:opacity-100">
                                    Activate
                                  </span>
                                </button>
                              ) : (
                                <button
                                  type="button"
                                  onClick={() => setPendingDeactivateId(algo.id)}
                                  className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full text-xs font-medium border border-slate-200 dark:border-slate-700 text-slate-500 hover:border-red-400 hover:text-red-500 dark:hover:border-red-600 dark:hover:text-red-400 transition-colors"
                                >
                                  <PowerOff size={10} />
                                  Stop
                                </button>
                              )}
                            </div>
                          </div>

                          {/* Expanded details + live results */}
                          {isExpanded && abData && (
                            <div className="mt-2 space-y-1 text-xs text-slate-500">
                              <div>Control: <span className="text-slate-700 dark:text-slate-300">{algorithmName(abData.control_algorithm_id)}</span></div>
                              <div>Variant: <span className="text-slate-700 dark:text-slate-300">{algorithmName(abData.variant_algorithm_id)}</span></div>
                              <div>Min sample: <span className="text-slate-700 dark:text-slate-300">{abData.min_sample_size.toLocaleString()}</span> · Guardrail: <span className="text-slate-700 dark:text-slate-300">{abData.guardrail_threshold_pp}pp</span></div>
                              {isActive && merchantId && (
                                <LiveResults algorithm={algo} merchantId={merchantId} />
                              )}
                            </div>
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
              )}
            </div>
          </Card>

          {activeAbTest && (
            <div className="rounded-lg border border-purple-200 bg-purple-50 px-3 py-2 text-xs text-purple-700 dark:border-purple-500/30 dark:bg-purple-500/10 dark:text-purple-300">
              <strong>Experiment "{activeAbTest.name}" is running</strong> — creating a new experiment will not affect it until you activate a different one.
            </div>
          )}
        </div>

        {/* Right: create form */}
        <div className="lg:col-span-2 space-y-4">
          <Card>
            <CardHeader>
              <h2 className="text-sm font-semibold text-slate-800">New Experiment</h2>
            </CardHeader>
            <CardBody className="space-y-5">

              {/* Name */}
              <div>
                <label className="block text-xs text-slate-500 mb-1">Experiment name *</label>
                <input
                  className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  placeholder="e.g. Stripe vs Checkout.com Auth Rate"
                  value={form.name}
                  onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                />
              </div>

              {/* Control / Variant */}
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs text-slate-500 mb-1">Control arm *</label>
                  <p className="text-[11px] text-slate-400 mb-1.5">Your current strategy — the baseline.</p>
                  <select
                    className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    value={form.controlAlgorithmId}
                    onChange={e => setForm(f => ({ ...f, controlAlgorithmId: e.target.value }))}
                  >
                    <option value="">Select algorithm</option>
                    {eligibleAlgorithms.map(a => (
                      <option key={a.id} value={a.id} disabled={a.id === form.variantAlgorithmId}>
                        {a.name} ({(a.algorithm_data || a.algorithm)?.type})
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="block text-xs text-slate-500 mb-1">Variant arm *</label>
                  <p className="text-[11px] text-slate-400 mb-1.5">The new strategy you want to test.</p>
                  <select
                    className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    value={form.variantAlgorithmId}
                    onChange={e => setForm(f => ({ ...f, variantAlgorithmId: e.target.value }))}
                  >
                    <option value="">Select algorithm</option>
                    {eligibleAlgorithms.map(a => (
                      <option key={a.id} value={a.id} disabled={a.id === form.controlAlgorithmId}>
                        {a.name} ({(a.algorithm_data || a.algorithm)?.type})
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Traffic split */}
              <div>
                <label className="block text-xs text-slate-500 mb-1">
                  Variant traffic — <span className="font-semibold text-slate-700 dark:text-slate-300">{form.variantSplitPct}% variant / {100 - form.variantSplitPct}% control</span>
                </label>
                <p className="text-[11px] text-slate-400 mb-2">Keep this small (5–15%) to limit exposure while collecting data.</p>
                <input
                  type="range"
                  min={5}
                  max={30}
                  step={1}
                  value={form.variantSplitPct}
                  onChange={e => setForm(f => ({ ...f, variantSplitPct: Number(e.target.value) }))}
                  className="w-full accent-brand-500"
                />
                <div className="flex justify-between text-[10px] text-slate-400 mt-0.5">
                  <span>5%</span>
                  <span>30%</span>
                </div>
              </div>

              {/* Min sample size */}
              <div>
                <label className="block text-xs text-slate-500 mb-1">Minimum sample size</label>
                <p className="text-[11px] text-slate-400 mb-2">Transactions to collect before reporting a significance verdict.</p>
                <div className="flex items-center gap-2 flex-wrap">
                  {SAMPLE_SIZE_PRESETS.map(n => (
                    <button
                      key={n}
                      type="button"
                      onClick={() => setForm(f => ({ ...f, minSampleSize: n }))}
                      className={`px-3 py-1 rounded-md text-xs font-medium border transition-colors ${
                        form.minSampleSize === n
                          ? 'bg-brand-500 text-white border-brand-500'
                          : 'border-slate-200 dark:border-[#222226] text-slate-600 dark:text-slate-400 hover:border-brand-400'
                      }`}
                    >
                      {n.toLocaleString()}
                    </button>
                  ))}
                  <input
                    type="number"
                    min={100}
                    className="w-28 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500"
                    value={form.minSampleSize}
                    onChange={e => setForm(f => ({ ...f, minSampleSize: Number(e.target.value) }))}
                  />
                </div>
              </div>

              {/* Guardrail */}
              <div>
                <label className="block text-xs text-slate-500 mb-1">Safety guardrail (pp)</label>
                <p className="text-[11px] text-slate-400 mb-2">Flag experiment if variant auth rate drops more than this many percentage points below control.</p>
                <input
                  type="number"
                  min={0.5}
                  max={20}
                  step={0.5}
                  className="w-28 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  value={form.guardrailThresholdPp}
                  onChange={e => setForm(f => ({ ...f, guardrailThresholdPp: Number(e.target.value) }))}
                />
              </div>

              <ErrorMessage error={error} />

              {success && (
                <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-200">
                  <span>{success}</span>
                  {createdId && (
                    <Button size="sm" variant="primary" onClick={() => handleActivate(createdId)}>
                      Activate Now
                    </Button>
                  )}
                </div>
              )}

              <Button variant="primary" onClick={handleCreate} disabled={saving || !merchantId}>
                {saving ? <><Spinner size={14} /> Creating…</> : 'Create Experiment'}
              </Button>

            </CardBody>
          </Card>
        </div>
      </div>

      <ConfirmDialog
        open={pendingActivateId !== null}
        title="Switch active experiment?"
        description="An experiment is already running. Activating this one will replace it."
        confirmLabel="Yes, activate"
        variant="primary"
        onConfirm={() => { const id = pendingActivateId!; setPendingActivateId(null); void doActivate(id) }}
        onCancel={() => setPendingActivateId(null)}
      />
      <ConfirmDialog
        open={pendingDeactivateId !== null}
        title="Stop experiment?"
        description="This will deactivate the experiment and restore default routing. Results will remain available."
        confirmLabel="Stop experiment"
        variant="danger"
        onConfirm={() => { const id = pendingDeactivateId!; void doDeactivate(id) }}
        onCancel={() => setPendingDeactivateId(null)}
      />
    </div>
  )
}
