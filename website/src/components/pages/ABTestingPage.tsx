import { useState, useEffect, type ReactNode } from 'react'
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
  SrConfigOverride,
  ExperimentResultsResponse,
  ExperimentTransactionsResponse,
} from '../../types/api'
import { ShieldAlert, PowerOff, Plus, FlaskConical, CheckCircle2, XCircle, Clock, AlertTriangle, Sliders, Pencil, Trash2, Info } from 'lucide-react'
import { RuleBreakdown } from './EuclidRulesPage'
import { validateABTestForm } from '../../features/routing/abTesting/schema'
import { toABTestCreatePayload } from '../../features/routing/abTesting/payload'
import { toABTestFormValues } from '../../features/routing/abTesting/state'
import { ABTestFormValues, ABTestExperimentType, SrConfigOverrideForm, DEFAULT_VARIANT_SR_CONFIG, SR_STRATEGY_LABELS, SrStrategy } from '../../features/routing/abTesting/types'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'

const SAMPLE_SIZE_PRESETS = [1000, 5000, 10000, 50000]

const EXPERIMENT_TYPE_HELP: Record<ABTestExperimentType, string> = {
  algorithm_comparison: 'Compare any two routing strategies — SR (auth), SR cost-aware (manual/autopilot), rule-based, or volume split.',
  sr_config_tuning: 'Same SR algorithm, different hedging % or elimination threshold on the variant.',
}

// Detect the experiment type from the persisted arm shape (the backend stores no "type").
function abExperimentKind(abData?: ABTestAlgorithmData): ABTestExperimentType {
  const v = abData?.variant_sr_config
  if (v && (v.hedging_percent !== undefined || v.elimination_threshold !== undefined)) return 'sr_config_tuning'
  return 'algorithm_comparison'
}

// Cost/net-value metrics are meaningful when either arm runs multi-objective (cost-aware) routing.
function hasCostArm(abData?: ABTestAlgorithmData): boolean {
  return abData?.control_sr_config?.enable_multi_objective === true
    || abData?.variant_sr_config?.enable_multi_objective === true
}

function KindBadge({ kind }: { kind: ABTestExperimentType }) {
  if (kind === 'sr_config_tuning') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300">
      <Sliders size={9} /> SR Config Tuning
    </span>
  )
  return null
}

// Display label for an arm (algorithm_id + sr_config) — resolves the four SR strategies
// (cost-awareness × autopilot).
function armLabel(id: string, config: SrConfigOverride | undefined, algorithmName: (id: string) => string): string {
  if (id === 'sr_routing') {
    if (config?.enable_multi_objective === true) return config.use_autopilot === true ? 'SR Routing (MO autopilot)' : 'SR Routing (MO manual)'
    return config?.use_autopilot === true ? 'SR Routing (auth based autopilot)' : 'SR Routing (auth based)'
  }
  return algorithmName(id)
}

// Renders the actual routing logic behind a static arm (rule-based / priority / volume split /
// single connector) so the merchant can see what the algorithm ID label stands for, instead of
// just its name. Returns nothing for SR-based arms (`sr_routing`) — those have no static config.
function ArmRuleDetail({ algorithmId, algorithms }: { algorithmId: string; algorithms: RoutingAlgorithm[] }) {
  const algo = algorithms.find(a => a.id === algorithmId)
  if (!algo) return null
  const algorithm = algo.algorithm_data || algo.algorithm
  const type = algorithm?.type
  const data = algorithm?.data

  if (type === 'advanced') {
    return <RuleBreakdown algo={algo} />
  }
  if (type === 'priority') {
    const gateways = (Array.isArray(data) ? data : []) as { gateway_name: string }[]
    return gateways.length > 0 ? (
      <div className="flex flex-wrap gap-1">
        {gateways.map((g, i) => (
          <span key={i} className="rounded-full bg-brand-50 dark:bg-brand-900/20 px-2 py-0.5 text-[11px] font-medium text-brand-700 dark:text-brand-300">
            {i + 1}. {g.gateway_name}
          </span>
        ))}
      </div>
    ) : <p className="text-xs text-slate-400 italic">No connectors configured.</p>
  }
  if (type === 'volume_split') {
    const splits = (Array.isArray(data) ? data : []) as { split: number; output: { gateway_name: string } }[]
    return splits.length > 0 ? (
      <div className="flex flex-wrap gap-1">
        {splits.map((s, i) => (
          <span key={i} className="rounded-full bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:text-emerald-300">
            {s.output.gateway_name} {s.split}%
          </span>
        ))}
      </div>
    ) : <p className="text-xs text-slate-400 italic">No splits configured.</p>
  }
  if (type === 'single') {
    const conn = data as { gateway_name: string } | undefined
    return conn ? (
      <span className="rounded-full bg-slate-100 dark:bg-[#1a1f2a] px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:text-[#8090a8]">
        {conn.gateway_name}
      </span>
    ) : <p className="text-xs text-slate-400 italic">No connector configured.</p>
  }
  return null
}

// Hover affordance that moves a long explanation off the page into an info icon — keeps
// labels short while the detail stays one hover away.
function InfoHint({ text }: { text: string }) {
  return (
    <span title={text} className="inline-flex cursor-help align-middle text-slate-300 hover:text-slate-500 dark:text-slate-600 dark:hover:text-slate-400">
      <Info size={12} />
    </span>
  )
}

// Compact field label: name + required marker + optional info tooltip, replacing verbose
// helper paragraphs under each input.
function FieldLabel({ children, hint, required }: { children: ReactNode; hint?: string; required?: boolean }) {
  return (
    <label className="mb-1.5 flex items-center gap-1 text-xs font-medium text-slate-600 dark:text-slate-300">
      {children}{required && <span className="text-slate-400">*</span>}
      {hint && <InfoHint text={hint} />}
    </label>
  )
}

// Human labels for routing-strategy types (the `algorithm_data.type` values).
const ALGO_TYPE_LABELS: Record<string, string> = {
  advanced: 'Rule-based',
  volume_split: 'Volume split',
  priority: 'Priority list',
  single: 'Single connector',
}

// The three SR strategies are top-level arm choices (each resolves to a distinct override).
const SR_STRATEGIES = Object.keys(SR_STRATEGY_LABELS) as (keyof typeof SR_STRATEGY_LABELS)[]
const isSrStrategy = (v: string): boolean => (SR_STRATEGIES as string[]).includes(v)

// Cascading arm picker for Algorithm comparison. Level 1 picks the strategy: an SR strategy
// (auth / auth+autopilot / multi-objective manual / multi-objective autopilot) resolves directly to
// an arm; a saved config type (Rule-based / Volume split / …) shows a 2nd dropdown when it has more
// than one config (a single-config type is auto-selected). `value` is the resolved arm form value:
// '' | 'sr_auth' | 'sr_auth_autopilot' | 'sr_mo_manual' | 'sr_mo_autopilot' | <algorithmId>.
function ArmSelector({ label, help, algorithms, value, excludeId, allowedSrStrategies, liveSrConfig, onChange }: {
  label: string
  help: string
  algorithms: RoutingAlgorithm[]
  value: string
  excludeId: string
  // SR strategies the merchant's features permit; the currently-selected value is always kept
  // visible so editing an experiment whose feature was later disabled still works.
  allowedSrStrategies: SrStrategy[]
  // The merchant's base SR config (hedging / elimination / bucket size) plus how many segments
  // autopilot is actively tuning — shown when the resolved arm is SR-based. All three SR
  // strategies share the same base config; they differ in whether they honor autopilot's
  // per-segment overrides on top of it (see `honorsAutopilot` below). `autopilotFeatureOn` is
  // the merchant's actual auto-calibration flag — segment count alone can't distinguish "tuning
  // right now" from "tuned before the feature was switched off".
  liveSrConfig: { hedging: number | null; elimination: number | null; bucketSize: number | null; autopilotSegmentCount: number; autopilotFeatureOn: boolean }
  onChange: (id: string) => void
}) {
  const srOptions = SR_STRATEGIES.filter(s => allowedSrStrategies.includes(s) || value === s)
  const typeOf = (id: string): string => {
    if (isSrStrategy(id)) return id // SR strategies are their own top-level "strategy"
    const a = algorithms.find(x => x.id === id)
    return a ? ((a.algorithm_data || a.algorithm)?.type ?? '') : ''
  }
  const [strategy, setStrategy] = useState<string>(() => typeOf(value))
  // Keep the strategy in sync when the value is set externally (edit prefill) or once algorithms load.
  useEffect(() => {
    const t = typeOf(value)
    if (t) setStrategy(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, algorithms])

  const realTypes = Array.from(
    new Set(algorithms.map(a => (a.algorithm_data || a.algorithm)?.type).filter(Boolean) as string[]),
  )
  const configs = strategy && !isSrStrategy(strategy)
    ? algorithms.filter(a => (a.algorithm_data || a.algorithm)?.type === strategy)
    : []

  function pickStrategy(t: string) {
    setStrategy(t)
    if (isSrStrategy(t)) { onChange(t); return } // SR strategy resolves directly to the arm value
    if (!t) { onChange(''); return }
    const c = algorithms.filter(a => (a.algorithm_data || a.algorithm)?.type === t)
    // One config → auto-select it; multiple → clear so the 2nd dropdown forces a choice.
    onChange(c.length === 1 ? c[0].id : '')
  }

  const selectCls = 'w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500'

  return (
    <div>
      <FieldLabel hint={help} required>{label}</FieldLabel>
      <select className={selectCls} value={strategy} onChange={e => pickStrategy(e.target.value)}>
        <option value="">Select strategy</option>
        {srOptions.map(s => (
          <option key={s} value={s} disabled={excludeId === s}>{SR_STRATEGY_LABELS[s]}</option>
        ))}
        {realTypes.map(t => (
          <option key={t} value={t}>{ALGO_TYPE_LABELS[t] ?? t}</option>
        ))}
      </select>
      {configs.length > 1 && (
        <select className={`${selectCls} mt-2`} value={value} onChange={e => onChange(e.target.value)}>
          <option value="">Select {ALGO_TYPE_LABELS[strategy]?.toLowerCase() ?? 'config'}</option>
          {configs.map(a => (
            <option key={a.id} value={a.id} disabled={a.id === excludeId}>{a.name}</option>
          ))}
        </select>
      )}
      {configs.length === 1 && (
        <p className="mt-1.5 text-[11px] text-slate-400">Using <span className="font-medium text-slate-600 dark:text-slate-300">{configs[0].name}</span></p>
      )}
      {value && !isSrStrategy(value) && (
        <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
          <ArmRuleDetail algorithmId={value} algorithms={algorithms} />
        </div>
      )}
      {value && isSrStrategy(value) && (
        <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
          <p className="text-[10px] font-medium uppercase tracking-wide text-slate-400 mb-1.5">Base SR config</p>
          <LiveSrConfigPanel
            hedging={liveSrConfig.hedging}
            elimination={liveSrConfig.elimination}
            bucketSize={liveSrConfig.bucketSize}
            autopilotSegmentCount={liveSrConfig.autopilotSegmentCount}
            autopilotFeatureOn={liveSrConfig.autopilotFeatureOn}
            // The two autopilot strategies honor autopilot-tuned segments; "auth based" and
            // "MO manual" run on the merchant's static/manual config (see resolveArm in payload.ts).
            honorsAutopilot={value === 'sr_mo_autopilot' || value === 'sr_auth_autopilot'}
          />
        </div>
      )}
    </div>
  )
}

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

// Fetches the merchant's live SR config (hedging %, elimination threshold) — the same values
// SR-based routing actually applies right now. Shared by the create form and the results view so
// any SR-backed arm (auth, MO manual, MO autopilot, or SR config tuning's control) can show its
// real current config instead of just a strategy name.
// The autopilot auto-calibration job writes cluster-specific hedging/bucket overrides tagged
// with this source string (see `sr_auto_calibration::AUTOPILOT_SOURCE` in the backend). A
// sub-level entry carrying it means autopilot is actively tuning that segment away from the
// merchant's flat default — the value below can't be read as "the" current hedging/bucket size.
const AUTOPILOT_SOURCE = 'autopilot'

function useLiveSrConfig(merchantId: string | undefined) {
  const { data: srConfig } = useSWR(
    merchantId ? ['rule-sr-live', merchantId] : null,
    () => apiPost<{
      config: {
        data: {
          defaultHedgingPercent: number | null
          defaultBucketSize: number | null
          subLevelInputConfig: { source?: string | null }[] | null
        }
      }
    }>('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false, revalidateOnFocus: false },
  )
  const { data: elimConfig } = useSWR(
    merchantId ? ['rule-elim-live', merchantId] : null,
    () => apiPost<{ config: { data: { threshold: number } } }>(
      '/rule/get', { merchant_id: merchantId, algorithm: 'elimination' }
    ),
    { shouldRetryOnError: false, revalidateOnFocus: false },
  )
  return {
    liveHedging: srConfig?.config?.data?.defaultHedgingPercent ?? null,
    liveElimination: elimConfig?.config?.data?.threshold ?? null,
    liveBucketSize: srConfig?.config?.data?.defaultBucketSize ?? null,
    autopilotSegmentCount: (srConfig?.config?.data?.subLevelInputConfig ?? [])
      .filter(c => c.source === AUTOPILOT_SOURCE).length,
  }
}

// `honorsAutopilot` reflects the arm's `use_autopilot` resolution (see `get_sr_v3_hedging_percent`
// / `get_sr_v3_bucket_size` in gw_scoring — absent override defaults to true). Only the "MO
// manual" strategy forces it false; auth and "MO autopilot" both honor autopilot-tuned segments
// by default, so both need the caveat when any exist.
function LiveSrConfigPanel({ hedging, elimination, bucketSize, autopilotSegmentCount, honorsAutopilot, autopilotFeatureOn }: {
  hedging: number | null
  elimination: number | null
  bucketSize: number | null
  autopilotSegmentCount: number
  honorsAutopilot: boolean
  autopilotFeatureOn: boolean
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between text-[11px]">
        <span className="text-slate-500">Hedging % (explore-exploit)</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {hedging !== null ? `${hedging}%` : <span className="text-slate-400 italic">Not configured</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[11px]">
        <span className="text-slate-500">Elimination threshold</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {elimination !== null ? `SR < ${(elimination * 100).toFixed(0)}%` : <span className="text-slate-400 italic">Not configured</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[11px]">
        <span className="text-slate-500">Bucket size (score window)</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {bucketSize !== null ? `${bucketSize} requests` : <span className="text-slate-400 italic">Not configured</span>}
        </span>
      </div>
      {honorsAutopilot && autopilotFeatureOn && (
        <p className="flex items-center gap-1 text-[10px] text-amber-600 dark:text-amber-400 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330]">
          Auto configures the Learning window and Discovery share based on your traffic volume.
          <InfoHint text={
            autopilotSegmentCount > 0
              ? `Autopilot is tuning ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'} (by payment method / network / currency / country) — hedging % (Discovery share) and bucket size (Learning window) can differ per segment and over time, so the values above are just the base config.`
              : `Autopilot will start tuning hedging % (Discovery share) and bucket size (Learning window) per segment once enough traffic flows through it. Until then, every transaction uses the base config shown above.`
          } />
        </p>
      )}
      {honorsAutopilot && !autopilotFeatureOn && autopilotSegmentCount > 0 && (
        <p className="flex items-center gap-1 text-[10px] text-slate-400 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330]">
          Autopilot is off — {autopilotSegmentCount} segment{autopilotSegmentCount === 1 ? '' : 's'} from earlier tuning
          <InfoHint text={`Autopilot (auto-calibration) is currently disabled for this merchant. ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'} still carry values it tuned before being turned off, but nothing is being actively adjusted right now — every transaction uses the base config shown above.`} />
        </p>
      )}
    </div>
  )
}

interface SrParamDiffProps {
  abData: ABTestAlgorithmData
  liveHedging: number | null
  liveElimination: number | null
}

// Only the numeric SR-tuning params are shown here (hedging / elimination). Other override
// fields (enable_multi_objective, margin, use_autopilot) belong to the cost/autopilot experiment
// types, which render their own arm-config views.
const SR_TUNING_KEYS = ['hedging_percent', 'elimination_threshold'] as const

function SrParamDiff({ abData, liveHedging, liveElimination }: SrParamDiffProps) {
  const vari = abData.variant_sr_config ?? {}
  const keys = SR_TUNING_KEYS.filter(k => typeof vari[k] === 'number')
  const liveValue: Record<string, number | null> = {
    hedging_percent: liveHedging,
    elimination_threshold: liveElimination,
  }

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
            const lv = liveValue[k]
            return (
              <tr key={String(k)} className="border-b border-slate-50 dark:border-[#131318]">
                <td className="px-4 py-2 text-slate-500">{srParamLabel(String(k))}</td>
                <td className="px-4 py-2 font-mono text-slate-600 dark:text-slate-400">
                  {lv !== null && lv !== undefined ? srParamFormat(String(k), lv) : <span className="italic text-slate-400">Not configured</span>}
                </td>
                <td className="px-4 py-2 font-mono font-semibold text-brand-600 dark:text-brand-400">
                  {typeof vv === 'number' ? srParamFormat(String(k), vv) : '—'}
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
  algorithms: RoutingAlgorithm[]
  onActivate: () => void
  onStop: () => void
  onEdit: () => void
  onDelete: () => void
}

function formatTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'short' }).format(new Date(ms))
}

function ExperimentDetailPanel({
  algorithm,
  isActive,
  merchantId,
  algorithmName,
  algorithms,
  onActivate,
  onStop,
  onEdit,
  onDelete,
}: DetailPanelProps) {
  const abData = (algorithm.algorithm_data || algorithm.algorithm)?.data as ABTestAlgorithmData | undefined
  const kind = abExperimentKind(abData)
  const isTuning = kind === 'sr_config_tuning'
  const costKind = hasCostArm(abData)
  const { liveHedging, liveElimination, liveBucketSize, autopilotSegmentCount } = useLiveSrConfig(merchantId || undefined)
  const merchantFeatures = useMerchantFeatures(merchantId || undefined)
  const autopilotFeatureOn = merchantFeatures.isEnabled('auto-calibration') || merchantFeatures.isEnabled('autopilot')

  // If the variant carries a margin override, value net EV at it; otherwise the backend default.
  const evalMargin = abData?.variant_sr_config?.margin
  const resultsUrl = abData
    ? `/analytics/experiment/${algorithm.id}/results?min_sample_size=${abData.min_sample_size}&guardrail_threshold_pp=${abData.guardrail_threshold_pp}${evalMargin !== undefined ? `&evaluation_margin=${evalMargin}` : ''}`
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
    const config = variantArm === 'control' ? abData.control_sr_config : abData.variant_sr_config
    if (algorithmId === 'sr_routing' && isTuning) {
      return variantArm === 'variant' ? 'SR Routing (custom params)' : 'SR Routing (live config)'
    }
    // armLabel resolves the SR strategies (auth / MO manual / MO autopilot) and real algo names.
    return armLabel(algorithmId, config, algorithmName)
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
            <KindBadge kind={kind} />
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
          {isActive ? (
            <Button size="sm" variant="danger" onClick={onStop}><PowerOff size={13} /> Stop</Button>
          ) : (
            <>
              {/* Edit / Delete are only offered while inactive — a running experiment must be
                  stopped first to avoid corrupting its collected results (enforced server-side too). */}
              <Button size="sm" variant="secondary" onClick={onEdit}><Pencil size={13} /> Edit</Button>
              <Button size="sm" variant="secondary" onClick={onDelete}><Trash2 size={13} /> Delete</Button>
              <Button size="sm" variant="primary" onClick={onActivate}>Activate</Button>
            </>
          )}
        </div>
      </div>

      {/* Arm config */}
      {abData && (
        isTuning ? (
          <SrParamDiff abData={abData} liveHedging={liveHedging} liveElimination={liveElimination} />
        ) : (
          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-xl border border-slate-200 dark:border-[#222226] bg-slate-50 dark:bg-[#0c0c10] px-4 py-3">
              <p className="text-[10px] font-medium uppercase tracking-wide text-slate-400 mb-1">Control ({controlPct}%)</p>
              <p className="text-sm font-medium text-slate-800 dark:text-white truncate">{armLabel(abData.control_algorithm_id, abData.control_sr_config, algorithmName)}</p>
              <p className="text-[10px] text-slate-400 mt-0.5">Baseline</p>
            </div>
            <div className="rounded-xl border border-brand-200 dark:border-brand-800/50 bg-brand-50/50 dark:bg-brand-900/10 px-4 py-3">
              <p className="text-[10px] font-medium uppercase tracking-wide text-brand-400 mb-1">Variant ({variantPct}%)</p>
              <p className="text-sm font-medium text-slate-800 dark:text-white truncate">{armLabel(abData.variant_algorithm_id, abData.variant_sr_config, algorithmName)}</p>
              <p className="text-[10px] text-slate-400 mt-0.5">Being tested</p>
            </div>
          </div>
        )
      )}

      {/* Routing rule breakdown — the actual logic behind each arm's algorithm ID. Static
          algorithms (rule-based, priority, volume split, single) show their configured rule;
          SR-based arms (auth / MO manual / MO autopilot) show the merchant's live SR config,
          fetched the same way SR config tuning's create-form panel does. */}
      {abData && !isTuning && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3 items-start">
          <div className="rounded-xl border border-slate-200 dark:border-[#222226] px-4 py-3">
            <p className="text-[10px] font-medium uppercase tracking-wide text-slate-400 mb-2">Control routing rule</p>
            {abData.control_algorithm_id === 'sr_routing'
              ? <LiveSrConfigPanel
                  hedging={liveHedging}
                  elimination={liveElimination}
                  bucketSize={liveBucketSize}
                  autopilotSegmentCount={autopilotSegmentCount}
                  autopilotFeatureOn={autopilotFeatureOn}
                  // Honors autopilot unless the arm explicitly set `use_autopilot: false` (the
                  // "auth based" and "MO manual" strategies); absent → honors it (gw_scoring's
                  // `unwrap_or(true)`).
                  honorsAutopilot={abData.control_sr_config?.use_autopilot !== false}
                />
              : <ArmRuleDetail algorithmId={abData.control_algorithm_id} algorithms={algorithms} />}
          </div>
          <div className="rounded-xl border border-brand-200 dark:border-brand-800/50 px-4 py-3">
            <p className="text-[10px] font-medium uppercase tracking-wide text-brand-400 mb-2">Variant routing rule</p>
            {abData.variant_algorithm_id === 'sr_routing'
              ? <LiveSrConfigPanel
                  hedging={liveHedging}
                  elimination={liveElimination}
                  bucketSize={liveBucketSize}
                  autopilotSegmentCount={autopilotSegmentCount}
                  autopilotFeatureOn={autopilotFeatureOn}
                  honorsAutopilot={abData.variant_sr_config?.use_autopilot !== false}
                />
              : <ArmRuleDetail algorithmId={abData.variant_algorithm_id} algorithms={algorithms} />}
          </div>
        </div>
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
                      <p className="text-xl font-bold text-slate-800 dark:text-white">
                        {authRatePct(metrics.auth_rate)} <span className="text-[10px] font-normal text-slate-400" title="Net auth rate — payments that succeeded on any attempt">NAR</span>
                      </p>
                      <p className="text-xs text-slate-500" title="First-attempt auth rate — payments that succeeded without a retry">
                        FAAR {authRatePct(metrics.first_attempt_auth_rate)}
                      </p>
                      {costKind && metrics.total_cost_saved !== null && (
                        <p className="text-xs text-sky-600 dark:text-sky-400" title="Total cost saved — gateway fees saved on successful payments">
                          TCS {metrics.total_cost_saved.toLocaleString(undefined, { maximumFractionDigits: 2 })}
                        </p>
                      )}
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
                      {costKind && metrics.avg_cost_saved_bps !== null && (
                        <p className="text-xs text-sky-600 dark:text-sky-400" title="Average bps saved vs the SR head on cost-routed payments">
                          {metrics.avg_cost_saved_bps.toFixed(1)} bps saved
                        </p>
                      )}
                    </div>
                  )
                })}

                {/* Merchant-facing delta: NAR / FAAR / TCS differences only. The verdict itself
                    still comes from the backend's EV z-test (shown as the VerdictChip); the
                    statistical internals (p, CI, EV bps) are deliberately not displayed. */}
                <div key="delta" className="rounded-xl border border-slate-200 dark:border-[#222226] px-4 py-3 space-y-1">
                  <p className="text-[10px] text-slate-400">Delta (variant − control)</p>
                  <p className={`text-xl font-bold ${results.delta_pp > 0 ? 'text-emerald-600 dark:text-emerald-400' : results.delta_pp < 0 ? 'text-red-500' : 'text-slate-800 dark:text-white'}`}>
                    {deltaLabel(results.delta_pp)} <span className="text-[10px] font-normal text-slate-400">NAR</span>
                  </p>
                  <p className="text-xs text-slate-500">
                    FAAR {deltaLabel((results.variant.first_attempt_auth_rate - results.control.first_attempt_auth_rate) * 100)}
                  </p>
                  {costKind && (
                    <p className="text-xs text-sky-600 dark:text-sky-400" title="Extra gateway fees saved by the variant vs the control">
                      TCS {(() => {
                        const d = (results.variant.total_cost_saved ?? 0) - (results.control.total_cost_saved ?? 0)
                        return `${d > 0 ? '+' : ''}${d.toLocaleString(undefined, { maximumFractionDigits: 2 })}`
                      })()}
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
              {txnData ? `${txnData.total.toLocaleString()} decisions` : 'Loading…'}
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
          <label className="mb-1 flex items-center gap-1 text-[11px] text-slate-500">
            Hedging % (explore-exploit)
            <InfoHint text="Share of traffic sent to non-top gateways to keep their scores fresh." />
          </label>
          <input
            type="number" min={0} max={100} step={1}
            value={config.hedgingPercent ?? ''}
            placeholder="e.g. 5"
            onChange={e => onChange(c => ({ ...c, hedgingPercent: e.target.value === '' ? null : Number(e.target.value) }))}
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
        </div>

        <div>
          <label className="mb-1 flex items-center gap-1 text-[11px] text-slate-500">
            Elimination threshold (0–1)
            <InfoHint text="SR score (0–1) below which a gateway is dropped from routing." />
          </label>
          <input
            type="number" min={0} max={1} step={0.01}
            value={config.eliminationThreshold ?? ''}
            placeholder="e.g. 0.70"
            onChange={e => onChange(c => ({ ...c, eliminationThreshold: e.target.value === '' ? null : Number(e.target.value) }))}
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
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
  isEditing: boolean
  onCreate: () => void
  onActivateCreated: (id: string) => void
}

function CreateForm({
  form, setForm, eligibleAlgorithms, saving, error, success, createdId,
  merchantId, isEditing, onCreate, onActivateCreated,
}: CreateFormProps) {
  // Only offer the Multi-Objective SR strategies when the merchant has the backing features on:
  //  - MO manual needs cost-aware (multi-objective) routing enabled
  //  - MO autopilot additionally needs autopilot self-tuning (auto-calibration) enabled, otherwise
  //    there are no autopilot-tuned values and it would behave identically to manual.
  const features = useMerchantFeatures(merchantId || undefined)
  const moOn = features.isEnabled('multi-objective-routing')
  const autopilotOn = features.isEnabled('auto-calibration') || features.isEnabled('autopilot')
  const allowedSrStrategies: SrStrategy[] = [
    'sr_auth',
    // Auth + autopilot needs only the autopilot feature (no cost-awareness required).
    ...(autopilotOn ? (['sr_auth_autopilot'] as SrStrategy[]) : []),
    ...(moOn ? (['sr_mo_manual'] as SrStrategy[]) : []),
    ...(moOn && autopilotOn ? (['sr_mo_autopilot'] as SrStrategy[]) : []),
  ]

  // Shared across both experiment types: SR config tuning needs it for the control panel below,
  // and any SR-based arm in Algorithm comparison (auth / MO manual / MO autopilot) shows it too.
  const { liveHedging, liveElimination, liveBucketSize, autopilotSegmentCount } = useLiveSrConfig(merchantId || undefined)

  const tabClass = (type: ABTestExperimentType) =>
    `px-3 py-1.5 text-xs font-medium rounded-md border transition-colors ${
      form.experimentType === type
        ? 'bg-slate-900 text-white border-slate-900 dark:bg-white dark:text-slate-900 dark:border-white'
        : 'border-slate-200 dark:border-[#222226] text-slate-600 dark:text-slate-400 hover:border-slate-400 dark:hover:border-slate-500'
    }`

  return (
    <Card>
      <CardHeader>
        <h2 className="text-sm font-semibold text-slate-800 dark:text-white">{isEditing ? 'Edit Experiment' : 'New Experiment'}</h2>
      </CardHeader>
      <CardBody className="space-y-5">

        {/* Experiment type toggle — creation only; the type is fixed once an experiment exists. */}
        {!isEditing && (
          <div>
            <label className="block text-xs text-slate-500 mb-2">Experiment type</label>
            <div className="flex flex-wrap items-center gap-2">
              <button type="button" className={tabClass('algorithm_comparison')} onClick={() => setForm(f => ({ ...f, experimentType: 'algorithm_comparison' }))}>
                Algorithm comparison
              </button>
              <button type="button" className={tabClass('sr_config_tuning')} onClick={() => setForm(f => ({ ...f, experimentType: 'sr_config_tuning' }))}>
                <Sliders size={12} className="inline mr-1" />SR config tuning
              </button>
            </div>
            <p className="mt-1.5 text-[11px] text-slate-400">
              {EXPERIMENT_TYPE_HELP[form.experimentType]}
            </p>
          </div>
        )}

        {/* Name — the only field editable on an existing experiment. */}
        <div>
          <FieldLabel required>Experiment name</FieldLabel>
          <input
            className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
            placeholder={form.experimentType === 'sr_config_tuning' ? 'e.g. Hedging 10% vs 5%' : 'e.g. Stripe vs Checkout.com'}
            value={form.name}
            onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
          />
          {isEditing && (
            <p className="mt-1.5 text-[11px] text-slate-400">
              Only the name can be changed. To test a different routing setup, create a new experiment.
            </p>
          )}
        </div>

        {/* Config fields (arms, split, sample, guardrail) — creation only. Editing an existing
            experiment must not rebuild its config, so everything below is hidden in edit mode. */}

        {/* ── Algorithm comparison arms ── */}
        {!isEditing && form.experimentType === 'algorithm_comparison' && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <ArmSelector
              label="Control arm"
              help="Your current strategy — the baseline."
              algorithms={eligibleAlgorithms}
              value={form.controlAlgorithmId}
              excludeId={form.variantAlgorithmId}
              allowedSrStrategies={allowedSrStrategies}
              liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
              onChange={id => setForm(f => ({ ...f, controlAlgorithmId: id }))}
            />
            <ArmSelector
              label="Variant arm"
              help="The new strategy you want to test."
              algorithms={eligibleAlgorithms}
              value={form.variantAlgorithmId}
              excludeId={form.controlAlgorithmId}
              allowedSrStrategies={allowedSrStrategies}
              liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
              onChange={id => setForm(f => ({ ...f, variantAlgorithmId: id }))}
            />
          </div>
        )}

        {/* ── SR Config Tuning arms ── */}
        {!isEditing && form.experimentType === 'sr_config_tuning' && (
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
        {!isEditing && (
        <div>
          <FieldLabel hint="Keep this small (5–15%) to limit exposure while the variant is unproven.">
            Variant traffic — <span className="ml-1 font-semibold text-slate-700 dark:text-slate-300">{form.variantSplitPct}% variant / {100 - form.variantSplitPct}% control</span>
          </FieldLabel>
          <input
            type="range" min={5} max={30} step={1}
            value={form.variantSplitPct}
            onChange={e => setForm(f => ({ ...f, variantSplitPct: Number(e.target.value) }))}
            className="w-full accent-brand-500"
          />
          <div className="flex justify-between text-[10px] text-slate-400 mt-0.5"><span>5%</span><span>30%</span></div>
        </div>
        )}

        {/* Min sample */}
        {!isEditing && (
        <div>
          <FieldLabel hint="Transactions needed before a significance verdict is reported.">Minimum sample size</FieldLabel>
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
        )}

        {/* Guardrail */}
        {!isEditing && (
        <div>
          <FieldLabel hint="Flag the experiment if the variant's auth rate falls this many percentage points below control.">Safety guardrail (pp)</FieldLabel>
          <input
            type="number" min={0.5} max={20} step={0.5}
            className="w-28 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
            value={form.guardrailThresholdPp}
            onChange={e => setForm(f => ({ ...f, guardrailThresholdPp: Number(e.target.value) }))}
          />
        </div>
        )}

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
          {saving ? <><Spinner size={14} /> {isEditing ? 'Saving…' : 'Creating…'}</> : isEditing ? 'Save Changes' : 'Create Experiment'}
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
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null)
  // When set, the form is editing an existing (inactive) experiment rather than creating one.
  const [editingId, setEditingId] = useState<string | null>(null)

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
    setEditingId(null)
    setForm({ ...DEFAULT_FORM })
    setSuccess(null)
    setError(null)
  }

  function openEdit(algo: RoutingAlgorithm) {
    const values = toABTestFormValues(algo)
    if (!values) return
    setForm(values)
    setEditingId(algo.id)
    setShowCreate(true)
    setSuccess(null)
    setError(null)
  }

  async function handleCreate() {
    if (!merchantId) return

    // Edit = rename only. The routing config (arms, split, sample, guardrail) is never
    // rebuilt on edit — we send the stored algorithm back untouched — so editing can never
    // silently change how a running/collected experiment routes. To change the setup, the
    // user creates a new experiment.
    if (editingId) {
      if (!form.name.trim()) { setError('Enter an experiment name'); return }
      const original = savedAbTests.find(a => a.id === editingId)
      const originalAlgorithm = original && (original.algorithm_data || original.algorithm)
      if (!original || !originalAlgorithm) { setError('Could not load the experiment to edit'); return }
      setSaving(true); setError(null); setSuccess(null)
      try {
        await apiPost('/routing/update', {
          created_by: merchantId,
          routing_algorithm_id: editingId,
          name: form.name.trim(),
          description: original.description ?? '',
          algorithm: originalAlgorithm,
        })
        await mutateAll()
        setSuccess(`"${form.name.trim()}" updated.`)
        setSearchParams({ experiment: editingId }, { replace: true })
        setEditingId(null)
        setForm({ ...DEFAULT_FORM })
        setShowCreate(false)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : 'Failed to update experiment')
      } finally {
        setSaving(false)
      }
      return
    }

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

  async function doDelete(id: string) {
    if (!merchantId) return
    try {
      await apiPost('/routing/delete', { created_by: merchantId, routing_algorithm_id: id })
      await Promise.all([mutateActive(), mutateAll()])
      if (selectedId === id) setSearchParams({}, { replace: true })
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to delete experiment')
    } finally {
      setPendingDeleteId(null)
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
                const kind = abExperimentKind(abData)

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
                        {kind === 'sr_config_tuning'
                          ? 'SR config tuning'
                          : `${armLabel(abData.control_algorithm_id, abData.control_sr_config, algorithmName)} → ${armLabel(abData.variant_algorithm_id, abData.variant_sr_config, algorithmName)}`
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
              algorithms={allAlgorithms ?? []}
              onActivate={() => handleActivate(selectedAlgo.id)}
              onStop={() => setPendingDeactivateId(selectedAlgo.id)}
              onEdit={() => openEdit(selectedAlgo)}
              onDelete={() => setPendingDeleteId(selectedAlgo.id)}
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
              isEditing={editingId !== null}
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
      <ConfirmDialog
        open={pendingDeleteId !== null}
        title="Delete experiment?"
        description="This permanently deletes the experiment definition. Any results already collected remain in analytics. This cannot be undone."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={() => { const id = pendingDeleteId!; void doDelete(id) }}
        onCancel={() => setPendingDeleteId(null)}
      />
    </div>
  )
}
