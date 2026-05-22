import { useState, useEffect } from 'react'
import { useSearchParams } from 'react-router-dom'
import useSWR, { useSWRConfig } from 'swr'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost, fetcher } from '../../lib/api'
import {
  RoutingAlgorithm,
  ABTestAlgorithmData,
  ExperimentResultsResponse,
  ExperimentTransactionsResponse,
} from '../../types/api'
import { ShieldAlert, PowerOff, Plus, FlaskConical, CheckCircle2, XCircle, Clock, AlertTriangle, Sliders } from 'lucide-react'
import { validateABTestForm } from '../../features/routing/abTesting/schema'
import { toABTestCreatePayload } from '../../features/routing/abTesting/payload'
import { ABTestFormValues, ABTestExperimentType, SrConfigOverrideForm, DEFAULT_VARIANT_SR_CONFIG } from '../../features/routing/abTesting/types'

const SAMPLE_SIZE_PRESETS = [1000, 5000, 10000, 50000]

function deltaLabel(deltaPp: number) {
  const sign = deltaPp > 0 ? '+' : ''
  return `${sign}${deltaPp.toFixed(2)}pp`
}

function authRatePct(rate: number) {
  return `${(rate * 100).toFixed(2)}%`
}

function VerdictChip({ verdict }: { verdict: string }) {
  if (verdict === 'collecting_data') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
      <Clock size={11} /> Collecting data
    </span>
  )
  if (verdict === 'variant_wins') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">
      <CheckCircle2 size={11} /> Variant wins
    </span>
  )
  if (verdict === 'variant_loses') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400">
      <XCircle size={11} /> Variant loses
    </span>
  )
  if (verdict === 'guardrail_breached') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400">
      <AlertTriangle size={11} /> Guardrail breached
    </span>
  )
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
      Not significant
    </span>
  )
}

// ─── SR Config param display helpers ──────────────────────────────────────────

function srParamLabel(key: string): string {
  const map: Record<string, string> = {
    hedging_percent: 'Hedging %',
    elimination_threshold: 'Elimination threshold',
  }
  return map[key] ?? key
}

function srParamFormat(key: string, value: number): string {
  if (key === 'hedging_percent') return `${value}%`
  if (key === 'elimination_threshold') return `SR < ${(value * 100).toFixed(0)}%`
  return String(value)
}

interface SrParamDiffProps {
  abData: ABTestAlgorithmData
}

function SrParamDiff({ abData }: SrParamDiffProps) {
  const vari = abData.variant_sr_config ?? {}
  const keys = Object.keys(vari) as (keyof typeof vari)[]

  if (keys.length === 0) return null

  return (
    <div className="rounded-xl border border-slate-200 dark:border-[#222226] overflow-hidden">
      <div className="px-4 py-2.5 bg-slate-50 dark:bg-[#0a0a0f] border-b border-slate-200 dark:border-[#222226]">
        <p className="text-[10px] font-medium uppercase tracking-wide text-slate-400">Parameter overrides</p>
      </div>
      <table className="w-full text-xs">
        <thead>
          <tr className="text-left text-[10px] text-slate-400 border-b border-slate-100 dark:border-[#1e2330]">
            <th className="px-4 py-2">Parameter</th>
            <th className="px-4 py-2 text-slate-500">Control (current config)</th>
            <th className="px-4 py-2 text-brand-500">Variant (override)</th>
          </tr>
        </thead>
        <tbody>
          {keys.map(k => {
            const vv = vari[k]
            return (
              <tr key={String(k)} className="border-b border-slate-50 dark:border-[#131318]">
                <td className="px-4 py-2 text-slate-500">{srParamLabel(String(k))}</td>
                <td className="px-4 py-2 text-slate-400 italic">Live SR config</td>
                <td className="px-4 py-2 font-mono font-semibold text-brand-600 dark:text-brand-400">
                  {vv !== undefined ? srParamFormat(String(k), vv) : '—'}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

// ─── Experiment detail panel ──────────────────────────────────────────────────

interface DetailPanelProps {
  algorithm: RoutingAlgorithm
  isActive: boolean
  merchantId: string
  algorithmName: (id: string) => string
  onActivate: () => void
  onStop: () => void
}

function formatTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'short' }).format(new Date(ms))
}

function ExperimentDetailPanel({
  algorithm,
  isActive,
  merchantId,
  algorithmName,
  onActivate,
  onStop,
}: DetailPanelProps) {
  const abData = (algorithm.algorithm_data || algorithm.algorithm)?.data as ABTestAlgorithmData | undefined
  const isTuning = Boolean(abData?.variant_sr_config)

  const resultsUrl = abData
    ? `/analytics/experiment/${algorithm.id}/results?min_sample_size=${abData.min_sample_size}&guardrail_threshold_pp=${abData.guardrail_threshold_pp}`
    : null

  const { data: results, isLoading } = useSWR<ExperimentResultsResponse>(
    merchantId && resultsUrl ? resultsUrl : null,
    fetcher,
    { refreshInterval: 60_000 },
  )

  const TXN_PAGE_SIZE = 20
  const [txnPage, setTxnPage] = useState(1)

  const txnsUrl = `/analytics/experiment/${algorithm.id}/transactions?page_size=${TXN_PAGE_SIZE}&page=${txnPage}`
  const { data: txnData, isLoading: txnsLoading } = useSWR<ExperimentTransactionsResponse>(
    merchantId ? txnsUrl : null,
    fetcher,
    { refreshInterval: 60_000 },
  )

  function routingType(variantArm: string): string {
    if (!abData) return '—'
    const algorithmId = variantArm === 'control' ? abData.control_algorithm_id : abData.variant_algorithm_id
    if (algorithmId === 'sr_routing') {
      if (isTuning) return variantArm === 'variant' ? 'SR Routing (custom params)' : 'SR Routing (live config)'
      return 'SR Routing'
    }
    return algorithmName(algorithmId)
  }

  function openAuditForTxn(paymentId: string, variantArm: string) {
    const isSr = variantArm === 'control'
      ? abData?.control_algorithm_id === 'sr_routing'
      : abData?.variant_algorithm_id === 'sr_routing'
    if (!isSr) return
    const url = `/audit?range=1d&exclude_routing_approach=NTW_BASED_ROUTING&payment_id=${encodeURIComponent(paymentId)}`
    window.open(url, '_blank')
  }

  const totalTxns = results ? results.control.transaction_count + results.variant.transaction_count : 0
  const minSample = abData?.min_sample_size ?? 1000
  const progress = Math.min(100, Math.round((totalTxns / minSample) * 100))
  const controlPct = 100 - (abData?.variant_split_pct ?? 10)
  const variantPct = abData?.variant_split_pct ?? 10

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-2">
            <h2 className="text-base font-semibold text-slate-900 dark:text-white">{algorithm.name}</h2>
            {isTuning && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300">
                <Sliders size={9} /> SR Config Tuning
              </span>
            )}
            {isActive
              ? <Badge variant="green">Active</Badge>
              : <Badge variant="gray">Inactive</Badge>
            }
          </div>
          {abData && (
            <p className="mt-0.5 text-xs text-slate-500">
              {controlPct}/{variantPct} split · Min sample {minSample.toLocaleString()} · Guardrail {abData.guardrail_threshold_pp}pp
            </p>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isActive
            ? <Button size="sm" variant="danger" onClick={onStop}><PowerOff size={13} /> Stop</Button>
            : <Button size="sm" variant="primary" onClick={onActivate}>Activate</Button>
          }
        </div>
      </div>

      {/* Arm config */}
      {abData && (
        isTuning ? (
          <SrParamDiff abData={abData} />
        ) : (
          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-xl border border-slate-200 dark:border-[#222226] bg-slate-50 dark:bg-[#0c0c10] px-4 py-3">
              <p className="text-[10px] font-medium uppercase tracking-wide text-slate-400 mb-1">Control ({controlPct}%)</p>
              <p className="text-sm font-medium text-slate-800 dark:text-white truncate">{algorithmName(abData.control_algorithm_id)}</p>
              <p className="text-[10px] text-slate-400 mt-0.5">Baseline</p>
            </div>
            <div className="rounded-xl border border-brand-200 dark:border-brand-800/50 bg-brand-50/50 dark:bg-brand-900/10 px-4 py-3">
              <p className="text-[10px] font-medium uppercase tracking-wide text-brand-400 mb-1">Variant ({variantPct}%)</p>
              <p className="text-sm font-medium text-slate-800 dark:text-white truncate">{algorithmName(abData.variant_algorithm_id)}</p>
              <p className="text-[10px] text-slate-400 mt-0.5">Being tested</p>
            </div>
          </div>
        )
      )}

      {/* Stats */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Results</h3>
            <p className="text-xs text-slate-500 mt-0.5">Updates every 60 seconds</p>
          </div>
          {results && <VerdictChip verdict={results.verdict} />}
        </CardHeader>
        <CardBody className="space-y-5">
          {isLoading && !results ? (
            <div className="flex items-center gap-2 text-sm text-slate-400"><Spinner size={14} /> Loading stats…</div>
          ) : !results ? (
            <p className="text-sm text-slate-400 italic">
              Stats unavailable — analytics pipeline may not be configured in this environment.
            </p>
          ) : (
            <>
              {/* Progress */}
              <div>
                <div className="flex justify-between text-xs text-slate-500 mb-1.5">
                  <span>Transactions collected</span>
                  <span className="font-medium">{totalTxns.toLocaleString()} / {minSample.toLocaleString()}</span>
                </div>
                <div className="h-2 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-brand-500 rounded-full transition-all duration-500"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <p className="mt-1 text-[10px] text-slate-400">{progress}% of minimum sample collected</p>
              </div>

              {/* Arm comparison */}
              <div className="grid grid-cols-3 gap-3">
                {[
                  { label: `Control (${controlPct}%)`, metrics: results.control, accent: false },
                  { label: `Variant (${variantPct}%)`, metrics: results.variant, accent: true },
                ].map(({ label, metrics, accent }) => {
                  const noOutcome = metrics.transaction_count - metrics.success_count - metrics.failure_count
                  return (
                    <div key={label} className={`rounded-xl border px-4 py-3 space-y-1 ${accent ? 'border-brand-200 dark:border-brand-800/50' : 'border-slate-200 dark:border-[#222226]'}`}>
                      <p className={`text-[10px] ${accent ? 'text-brand-500' : 'text-slate-400'}`}>{label}</p>
                      <p className="text-xl font-bold text-slate-800 dark:text-white">{authRatePct(metrics.auth_rate)}</p>
                      <p className="text-xs text-slate-400">{metrics.transaction_count.toLocaleString()} txns</p>
                      <p className="text-xs text-emerald-600 dark:text-emerald-400">{metrics.success_count.toLocaleString()} success</p>
                      {metrics.failure_count > 0 && (
                        <p className="text-xs text-red-500 dark:text-red-400">{metrics.failure_count.toLocaleString()} failure</p>
                      )}
                      {noOutcome > 0 && (
                        <p className="text-xs text-amber-600 dark:text-amber-400" title="Routed payments with no outcome recorded yet">
                          {noOutcome.toLocaleString()} no outcome
                        </p>
                      )}
                    </div>
                  )
                })}

                <div key="delta" className="rounded-xl border border-slate-200 dark:border-[#222226] px-4 py-3 space-y-1">
                  <p className="text-[10px] text-slate-400">Delta</p>
                  <p className={`text-xl font-bold ${results.delta_pp > 0 ? 'text-emerald-600 dark:text-emerald-400' : results.delta_pp < 0 ? 'text-red-500' : 'text-slate-800 dark:text-white'}`}>
                    {deltaLabel(results.delta_pp)}
                  </p>
                  {results.p_value !== null ? (
                    <p className="text-xs text-slate-400">p = {results.p_value.toFixed(4)}</p>
                  ) : (
                    <p className="text-xs text-slate-400">p = —</p>
                  )}
                  {results.confidence_interval !== null && results.confidence_interval !== undefined && (
                    <p className="text-[10px] text-slate-400">
                      95% CI [{deltaLabel(results.confidence_interval[0])}, {deltaLabel(results.confidence_interval[1])}]
                    </p>
                  )}
                </div>
              </div>

              {/* Guardrail warning */}
              {results.verdict === 'guardrail_breached' && (
                <div className="flex items-center gap-2 rounded-lg border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 px-3 py-2 text-xs text-red-600 dark:text-red-400">
                  <ShieldAlert size={12} />
                  Variant auth rate dropped {Math.abs(results.delta_pp).toFixed(2)}pp below control — beyond the {abData?.guardrail_threshold_pp}pp guardrail. Consider stopping the experiment.
                </div>
              )}
            </>
          )}
        </CardBody>
      </Card>

      {/* Transactions */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <div>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Transactions</h3>
            <p className="text-xs text-slate-500 mt-0.5">
              {txnData ? `${txnData.total.toLocaleString()} decisions · click any row to open in Decision Audit` : 'Loading…'}
            </p>
          </div>
          {txnsLoading && <Spinner size={14} />}
        </CardHeader>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs text-slate-500 bg-slate-50 dark:bg-[#0a0a0f] border-b border-t border-slate-100 dark:border-[#1e2330]">
                <th className="px-4 py-2.5 font-medium">Arm</th>
                <th className="px-4 py-2.5 font-medium">Routing</th>
                <th className="px-4 py-2.5 font-medium">Payment ID</th>
                <th className="px-4 py-2.5 font-medium">Gateway</th>
                <th className="px-4 py-2.5 font-medium">Status</th>
                <th className="px-4 py-2.5 font-medium">Time</th>
              </tr>
            </thead>
          </table>
          <div className="max-h-[400px] overflow-y-auto">
            <table className="w-full text-sm">
              <tbody>
                {!txnData?.transactions.length ? (
                  <tr>
                    <td colSpan={6} className="px-4 py-8 text-sm text-slate-400 text-center">
                      {txnsLoading ? 'Loading…' : 'No transactions logged yet for this experiment.'}
                    </td>
                  </tr>
                ) : txnData.transactions.map((txn, idx) => {
                  const txnIsSr = txn.variant_arm === 'control'
                    ? abData?.control_algorithm_id === 'sr_routing'
                    : abData?.variant_algorithm_id === 'sr_routing'
                  return (
                  <tr
                    key={`${txn.payment_id}-${idx}`}
                    onClick={() => openAuditForTxn(txn.payment_id, txn.variant_arm)}
                    title={txnIsSr ? 'Open in Decision Audit' : 'Audit trail not available for static arm payments'}
                    className={`border-b border-slate-50 dark:border-[#131318] transition-colors ${txnIsSr ? 'cursor-pointer hover:bg-slate-50 dark:hover:bg-[#0f0f16]' : 'cursor-default opacity-60'}`}
                  >
                    <td className="px-4 py-2.5">
                      <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold ${
                        txn.variant_arm === 'control'
                          ? 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
                          : 'bg-brand-100 text-brand-700 dark:bg-brand-900/30 dark:text-brand-300'
                      }`}>
                        {txn.variant_arm === 'control' ? 'Control' : 'Variant'}
                      </span>
                    </td>
                    <td className="px-4 py-2.5 text-xs text-slate-500 dark:text-slate-400 whitespace-nowrap">
                      {routingType(txn.variant_arm)}
                    </td>
                    <td className="px-4 py-2.5 font-mono text-xs text-slate-600 dark:text-slate-400 max-w-[180px] truncate">
                      {txn.payment_id}
                    </td>
                    <td className="px-4 py-2.5 text-xs text-slate-700 dark:text-slate-300">
                      {txn.gateway ?? '—'}
                    </td>
                    <td className="px-4 py-2.5">
                      {txn.status === 'success' ? (
                        <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">success</span>
                      ) : txn.status === 'failure' ? (
                        <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400">failure</span>
                      ) : (
                        <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400" title="Payment was routed but no outcome was recorded — counted against auth rate">
                          <Clock size={9} /> no outcome
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2.5 text-xs text-slate-400 whitespace-nowrap">
                      {formatTime(txn.created_at_ms)}
                    </td>
                  </tr>
                  )
                })}
              </tbody>
            </table>
            {/* Pagination */}
            {txnData && txnData.total > TXN_PAGE_SIZE && (() => {
              const totalPages = Math.ceil(txnData.total / TXN_PAGE_SIZE)
              return (
                <div className="flex items-center justify-between px-4 py-3 border-t border-slate-100 dark:border-[#1e2330]">
                  <p className="text-xs text-slate-500">
                    Page {txnPage} of {totalPages} · {txnData.total.toLocaleString()} total
                  </p>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      onClick={() => setTxnPage(p => Math.max(1, p - 1))}
                      disabled={txnPage === 1 || txnsLoading}
                      className="px-2.5 py-1 rounded-md border border-slate-200 dark:border-[#222226] text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                    >
                      ← Prev
                    </button>
                    {Array.from({ length: Math.min(totalPages, 7) }, (_, i) => {
                      const page = totalPages <= 7
                        ? i + 1
                        : [1, totalPages, txnPage - 1, txnPage, txnPage + 1].includes(i + 1)
                          ? i + 1
                          : null
                      if (page === null) return null
                      const prev = totalPages <= 7 ? i : [1, totalPages, txnPage - 1, txnPage, txnPage + 1].includes(i) ? i : null
                      const showEllipsis = prev !== null && page - prev > 1
                      return (
                        <span key={i} className="flex items-center gap-1">
                          {showEllipsis && <span className="px-1 text-xs text-slate-400">…</span>}
                          <button
                            type="button"
                            onClick={() => setTxnPage(page)}
                            disabled={txnsLoading}
                            className={`min-w-[28px] px-2 py-1 rounded-md border text-xs transition-colors ${
                              page === txnPage
                                ? 'border-brand-500 bg-brand-500 text-white'
                                : 'border-slate-200 dark:border-[#222226] text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22]'
                            }`}
                          >
                            {page}
                          </button>
                        </span>
                      )
                    })}
                    <button
                      type="button"
                      onClick={() => setTxnPage(p => Math.min(totalPages, p + 1))}
                      disabled={txnPage === totalPages || txnsLoading}
                      className="px-2.5 py-1 rounded-md border border-slate-200 dark:border-[#222226] text-xs text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                    >
                      Next →
                    </button>
                  </div>
                </div>
              )
            })()}
          </div>
        </div>
      </Card>
    </div>
  )
}

// ─── SR Config Tuning arm editor ──────────────────────────────────────────────

interface SrArmEditorProps {
  label: string
  splitPct: number
  config: SrConfigOverrideForm
  onChange: (fn: (c: SrConfigOverrideForm) => SrConfigOverrideForm) => void
}

function SrArmEditor({ label, splitPct, config, onChange }: SrArmEditorProps) {
  return (
    <div className="rounded-xl border border-brand-200 dark:border-brand-800/50 bg-brand-50/30 dark:bg-brand-900/10 px-4 py-4 space-y-3">
      <p className="text-[10px] font-semibold uppercase tracking-wide text-brand-500">
        {label} ({splitPct}%)
      </p>

      <div className="space-y-2.5">
        <div>
          <label className="block text-[11px] text-slate-500 mb-1">Hedging % (explore-exploit)</label>
          <input
            type="number" min={0} max={100} step={1}
            value={config.hedgingPercent ?? ''}
            placeholder="e.g. 5"
            onChange={e => onChange(c => ({ ...c, hedgingPercent: e.target.value === '' ? null : Number(e.target.value) }))}
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
          <p className="text-[10px] text-slate-400 mt-0.5">Share of traffic sent to non-top gateways to keep scores fresh</p>
        </div>

        <div>
          <label className="block text-[11px] text-slate-500 mb-1">Elimination threshold (0–1)</label>
          <input
            type="number" min={0} max={1} step={0.01}
            value={config.eliminationThreshold ?? ''}
            placeholder="e.g. 0.70"
            onChange={e => onChange(c => ({ ...c, eliminationThreshold: e.target.value === '' ? null : Number(e.target.value) }))}
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
          <p className="text-[10px] text-slate-400 mt-0.5">SR score (0–1) below which a gateway is dropped from routing</p>
        </div>
      </div>
    </div>
  )
}

// ─── Create form ──────────────────────────────────────────────────────────────

interface CreateFormProps {
  form: ABTestFormValues
  setForm: (fn: (f: ABTestFormValues) => ABTestFormValues) => void
  eligibleAlgorithms: RoutingAlgorithm[]
  saving: boolean
  error: string | null
  success: string | null
  createdId: string | null
  merchantId: string | null
  onCreate: () => void
  onActivateCreated: (id: string) => void
}

function CreateForm({
  form, setForm, eligibleAlgorithms, saving, error, success, createdId,
  merchantId, onCreate, onActivateCreated,
}: CreateFormProps) {
  const isTuningMode = form.experimentType === 'sr_config_tuning'

  const { data: srConfig } = useSWR(
    isTuningMode && merchantId ? ['rule-sr-ab', merchantId] : null,
    () => apiPost<{ config: { data: { defaultHedgingPercent: number | null } } }>(
      '/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }
    ),
    { shouldRetryOnError: false, revalidateOnFocus: false }
  )
  const { data: elimConfig } = useSWR(
    isTuningMode && merchantId ? ['rule-elim-ab', merchantId] : null,
    () => apiPost<{ config: { data: { threshold: number } } }>(
      '/rule/get', { merchant_id: merchantId, algorithm: 'elimination' }
    ),
    { shouldRetryOnError: false, revalidateOnFocus: false }
  )

  const liveHedging = srConfig?.config?.data?.defaultHedgingPercent ?? null
  const liveElimination = elimConfig?.config?.data?.threshold ?? null

  const tabClass = (type: ABTestExperimentType) =>
    `px-3 py-1.5 text-xs font-medium rounded-md border transition-colors ${
      form.experimentType === type
        ? 'bg-slate-900 text-white border-slate-900 dark:bg-white dark:text-slate-900 dark:border-white'
        : 'border-slate-200 dark:border-[#222226] text-slate-600 dark:text-slate-400 hover:border-slate-400 dark:hover:border-slate-500'
    }`

  return (
    <Card>
      <CardHeader>
        <h2 className="text-sm font-semibold text-slate-800 dark:text-white">New Experiment</h2>
        <p className="text-xs text-slate-500 mt-0.5">Define the arms and safety parameters for this experiment.</p>
      </CardHeader>
      <CardBody className="space-y-5">

        {/* Experiment type toggle */}
        <div>
          <label className="block text-xs text-slate-500 mb-2">Experiment type</label>
          <div className="flex items-center gap-2">
            <button type="button" className={tabClass('algorithm_comparison')} onClick={() => setForm(f => ({ ...f, experimentType: 'algorithm_comparison' }))}>
              Algorithm comparison
            </button>
            <button type="button" className={tabClass('sr_config_tuning')} onClick={() => setForm(f => ({ ...f, experimentType: 'sr_config_tuning' }))}>
              <Sliders size={12} className="inline mr-1" />SR config tuning
            </button>
          </div>
          <p className="mt-1.5 text-[11px] text-slate-400">
            {form.experimentType === 'algorithm_comparison'
              ? 'Compare two different routing strategies (e.g. SR vs priority list).'
              : 'Same SR algorithm, different hyperparameters — tune bucket size, hedging %, elimination threshold, or scoring weights.'
            }
          </p>
        </div>

        {/* Name */}
        <div>
          <label className="block text-xs text-slate-500 mb-1">Experiment name *</label>
          <input
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
            placeholder={form.experimentType === 'sr_config_tuning' ? 'e.g. Hedging 10% vs 5%' : 'e.g. Stripe vs Checkout.com'}
            value={form.name}
            onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
          />
        </div>

        {/* ── Algorithm comparison arms ── */}
        {form.experimentType === 'algorithm_comparison' && (
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
                <option value="sr_routing" disabled={form.variantAlgorithmId === 'sr_routing'}>SR Routing (Dynamic)</option>
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
                <option value="sr_routing" disabled={form.controlAlgorithmId === 'sr_routing'}>SR Routing (Dynamic)</option>
                {eligibleAlgorithms.map(a => (
                  <option key={a.id} value={a.id} disabled={a.id === form.controlAlgorithmId}>
                    {a.name} ({(a.algorithm_data || a.algorithm)?.type})
                  </option>
                ))}
              </select>
            </div>
          </div>
        )}

        {/* ── SR Config Tuning arms ── */}
        {form.experimentType === 'sr_config_tuning' && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {/* Control — live config, non-editable */}
            <div className="rounded-xl border border-slate-200 dark:border-[#222226] bg-slate-50/50 dark:bg-[#0c0c10] px-4 py-4 space-y-3">
              <p className="text-[10px] font-semibold uppercase tracking-wide text-slate-400">
                Control ({100 - form.variantSplitPct}%) — current config
              </p>
              <div className="space-y-2.5">
                <div>
                  <p className="text-[11px] text-slate-500 mb-0.5">Hedging % (explore-exploit)</p>
                  <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                    {liveHedging !== null ? `${liveHedging}%` : <span className="text-slate-400 italic text-xs">Not configured</span>}
                  </p>
                </div>
                <div>
                  <p className="text-[11px] text-slate-500 mb-0.5">Elimination threshold</p>
                  <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                    {liveElimination !== null ? `SR < ${(liveElimination * 100).toFixed(0)}%` : <span className="text-slate-400 italic text-xs">Not configured</span>}
                  </p>
                </div>
              </div>
              <p className="text-[10px] text-slate-400 pt-1 border-t border-slate-100 dark:border-[#1e2330]">
                Edit in <span className="font-medium">SR Routing → Scoring / Elimination</span>
              </p>
            </div>

            {/* Variant — editable overrides */}
            <SrArmEditor
              label="Variant"
              splitPct={form.variantSplitPct}
              config={form.variantSrConfig}
              onChange={fn => setForm(f => ({ ...f, variantSrConfig: fn(f.variantSrConfig) }))}
            />
          </div>
        )}

        {/* Traffic split */}
        <div>
          <label className="block text-xs text-slate-500 mb-1">
            Variant traffic — <span className="font-semibold text-slate-700 dark:text-slate-300">{form.variantSplitPct}% variant / {100 - form.variantSplitPct}% control</span>
          </label>
          <p className="text-[11px] text-slate-400 mb-2">Keep this small (5–15%) to limit exposure.</p>
          <input
            type="range" min={5} max={30} step={1}
            value={form.variantSplitPct}
            onChange={e => setForm(f => ({ ...f, variantSplitPct: Number(e.target.value) }))}
            className="w-full accent-brand-500"
          />
          <div className="flex justify-between text-[10px] text-slate-400 mt-0.5"><span>5%</span><span>30%</span></div>
        </div>

        {/* Min sample */}
        <div>
          <label className="block text-xs text-slate-500 mb-1">Minimum sample size</label>
          <p className="text-[11px] text-slate-400 mb-2">Transactions needed before reporting a significance verdict.</p>
          <div className="flex items-center gap-2 flex-wrap">
            {SAMPLE_SIZE_PRESETS.map(n => (
              <button
                key={n} type="button"
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
              type="number" min={100}
              className="w-28 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500"
              value={form.minSampleSize}
              onChange={e => setForm(f => ({ ...f, minSampleSize: Number(e.target.value) }))}
            />
          </div>
        </div>

        {/* Guardrail */}
        <div>
          <label className="block text-xs text-slate-500 mb-1">Safety guardrail (pp)</label>
          <p className="text-[11px] text-slate-400 mb-2">Flag if variant auth rate drops more than this many percentage points below control.</p>
          <input
            type="number" min={0.5} max={20} step={0.5}
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
              <Button size="sm" variant="primary" onClick={() => onActivateCreated(createdId)}>
                Activate Now
              </Button>
            )}
          </div>
        )}

        <Button variant="primary" onClick={onCreate} disabled={saving || !merchantId}>
          {saving ? <><Spinner size={14} /> Creating…</> : 'Create Experiment'}
        </Button>
      </CardBody>
    </Card>
  )
}

// ─── Page ──────────────────────────────────────────────────────────────────────

const DEFAULT_FORM: ABTestFormValues = {
  name: '',
  experimentType: 'algorithm_comparison',
  controlAlgorithmId: '',
  variantAlgorithmId: '',
  variantSplitPct: 10,
  minSampleSize: 5000,
  guardrailThresholdPp: 3,
  variantSrConfig: { ...DEFAULT_VARIANT_SR_CONFIG },
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

  const activeAbTest = activeAlgorithms?.find(r => (r.algorithm_data || r.algorithm)?.type === 'ab_test')
  const savedAbTests = allAlgorithms?.filter(r => (r.algorithm_data || r.algorithm)?.type === 'ab_test') ?? []
  const eligibleAlgorithms = allAlgorithms?.filter(r => (r.algorithm_data || r.algorithm)?.type !== 'ab_test') ?? []

  const [searchParams, setSearchParams] = useSearchParams()
  const selectedId = searchParams.get('experiment')
  const [showCreate, setShowCreate] = useState(false)

  const selectedAlgo = savedAbTests.find(a => a.id === selectedId) ?? null

  const [form, setForm] = useState<ABTestFormValues>({ ...DEFAULT_FORM })
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)
  const [createdId, setCreatedId] = useState<string | null>(null)

  const [pendingActivateId, setPendingActivateId] = useState<string | null>(null)
  const [pendingDeactivateId, setPendingDeactivateId] = useState<string | null>(null)

  // Auto-select the active experiment when the page loads with no selection
  useEffect(() => {
    if (!selectedId && !showCreate && activeAbTest) {
      setSearchParams({ experiment: activeAbTest.id }, { replace: true })
    }
  }, [activeAbTest?.id])

  function selectExperiment(id: string) {
    setSearchParams({ experiment: id }, { replace: true })
    setShowCreate(false)
  }

  function openCreate() {
    setSearchParams({}, { replace: true })
    setShowCreate(true)
    setSuccess(null)
    setError(null)
  }

  async function handleCreate() {
    if (!merchantId) return
    const validationError = validateABTestForm(form)
    if (validationError) { setError(validationError); return }
    setSaving(true); setError(null); setSuccess(null)
    try {
      const payload = toABTestCreatePayload(form, merchantId)
      const result = await apiPost<RoutingAlgorithm>('/routing/create', payload)
      const id = result.rule_id || result.id
      setCreatedId(id)
      setSuccess(`"${form.name}" created.`)
      setForm({ ...DEFAULT_FORM })
      await mutateAll()
      setSearchParams({ experiment: id }, { replace: true })
      setShowCreate(false)
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
      setCreatedId(null); setSuccess(null)
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
    if (id === 'sr_routing') return 'SR Routing (Dynamic)'
    return allAlgorithms?.find(a => a.id === id)?.name ?? id
  }

  const rightPanelContent = (() => {
    if (showCreate) return 'create'
    if (selectedAlgo) return 'detail'
    if (savedAbTests.length === 0) return 'create'
    return 'empty'
  })()

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">A/B Testing</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
            Compare routing strategies on live traffic with statistical significance.
          </p>
        </div>
        <Button variant="secondary" size="sm" onClick={openCreate}>
          <Plus size={14} /> New Experiment
        </Button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-5">

        {/* Left: experiment list */}
        <div className="space-y-1">
          <p className="px-1 pb-1 text-[11px] font-medium uppercase tracking-wide text-slate-400">
            Experiments {savedAbTests.length > 0 && `(${savedAbTests.length})`}
          </p>

          {!merchantId ? (
            <p className="px-2 py-2 text-sm text-slate-400">Set merchant ID to load experiments.</p>
          ) : !allAlgorithms ? (
            <p className="px-2 py-2 text-sm text-slate-400">Loading…</p>
          ) : savedAbTests.length === 0 ? (
            <div className="flex flex-col items-center gap-2 rounded-xl border border-dashed border-slate-200 dark:border-[#222226] py-8 text-center">
              <FlaskConical size={20} className="text-slate-300 dark:text-slate-600" />
              <p className="text-sm text-slate-400">No experiments yet.</p>
              <Button size="sm" variant="secondary" onClick={openCreate}><Plus size={13} /> Create one</Button>
            </div>
          ) : (
            <div className="rounded-xl border border-slate-200 dark:border-[#222226] overflow-hidden">
              {savedAbTests.map((algo, idx) => {
                const abData = (algo.algorithm_data || algo.algorithm)?.data as ABTestAlgorithmData | undefined
                const isActive = activeAbTest?.id === algo.id
                const isSelected = selectedId === algo.id
                const isTuning = Boolean(abData?.variant_sr_config)

                return (
                  <button
                    key={algo.id}
                    type="button"
                    onClick={() => selectExperiment(algo.id)}
                    className={`w-full text-left px-3 py-2.5 transition-colors ${
                      idx > 0 ? 'border-t border-slate-100 dark:border-[#1e2330]' : ''
                    } ${
                      isSelected
                        ? 'bg-brand-50 dark:bg-brand-900/20'
                        : 'hover:bg-slate-50 dark:hover:bg-[#0f0f16]'
                    }`}
                  >
                    <div className="flex items-center gap-1.5 min-w-0">
                      <p className={`truncate text-sm font-medium ${
                        isSelected ? 'text-brand-700 dark:text-brand-300' : 'text-slate-800 dark:text-white'
                      }`}>
                        {algo.name}
                      </p>
                      {isActive && (
                        <span className="shrink-0 inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400">
                          ● Active
                        </span>
                      )}
                    </div>
                    {abData && (
                      <p className="text-[11px] text-slate-400 mt-0.5 truncate">
                        {isTuning
                          ? 'SR config tuning'
                          : `${algorithmName(abData.control_algorithm_id)} → ${algorithmName(abData.variant_algorithm_id)}`
                        }
                      </p>
                    )}
                  </button>
                )
              })}
            </div>
          )}
        </div>

        {/* Right: detail panel or create form */}
        <div>
          {rightPanelContent === 'detail' && selectedAlgo && merchantId && (
            <ExperimentDetailPanel
              algorithm={selectedAlgo}
              isActive={activeAbTest?.id === selectedAlgo.id}
              merchantId={merchantId}
              algorithmName={algorithmName}
              onActivate={() => handleActivate(selectedAlgo.id)}
              onStop={() => setPendingDeactivateId(selectedAlgo.id)}
            />
          )}

          {rightPanelContent === 'create' && (
            <CreateForm
              form={form}
              setForm={setForm}
              eligibleAlgorithms={eligibleAlgorithms}
              saving={saving}
              error={error}
              success={success}
              createdId={createdId}
              merchantId={merchantId}
              onCreate={handleCreate}
              onActivateCreated={(id) => handleActivate(id)}
            />
          )}

          {rightPanelContent === 'empty' && (
            <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-200 dark:border-[#222226] py-16 text-center">
              <FlaskConical size={24} className="text-slate-300 dark:text-slate-600 mb-2" />
              <p className="text-sm text-slate-500">Select an experiment to view details</p>
            </div>
          )}
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
