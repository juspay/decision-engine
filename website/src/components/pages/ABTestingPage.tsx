import { useState, useEffect, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
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
import { ShieldAlert, PowerOff, Plus, FlaskConical, CheckCircle2, XCircle, Clock, AlertTriangle, Sliders, Pencil, Trash2, Info, GitCompare, Copy } from 'lucide-react'
import * as type from '../ui/typography'
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
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300">
      <Sliders size={9} /> SR Config Tuning
    </span>
  )
  return null
}

// Display label for an arm (algorithm_id + sr_config) — resolves the four SR strategies
// (cost-awareness × autopilot).
function armLabel(id: string, config: SrConfigOverride | undefined, algorithmName: (id: string) => string): string {
  if (id === 'sr_routing') {
    // Resolve the two config dials (cost-awareness, autopilot) to a SrStrategy key and label it
    // through the shared map, so the results/config views read identically to the create-form
    // dropdown — same combo → same string, defined once in SR_STRATEGY_LABELS.
    const key: SrStrategy = config?.enable_multi_objective === true
      ? (config.use_autopilot === true ? 'sr_mo_autopilot' : 'sr_mo_manual')
      : (config?.use_autopilot === true ? 'sr_auth_autopilot' : 'sr_auth')
    return SR_STRATEGY_LABELS[key]
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
          <span key={i} className="rounded-full bg-brand-50 dark:bg-brand-900/20 px-2 py-0.5 text-xs font-medium text-brand-700 dark:text-brand-300">
            {i + 1}. {g.gateway_name}
          </span>
        ))}
      </div>
    ) : <p className="text-sm text-slate-400 italic">No connectors configured.</p>
  }
  if (type === 'volume_split') {
    const splits = (Array.isArray(data) ? data : []) as { split: number; output: { gateway_name: string } }[]
    return splits.length > 0 ? (
      <div className="flex flex-wrap gap-1">
        {splits.map((s, i) => (
          <span key={i} className="rounded-full bg-emerald-50 dark:bg-emerald-900/20 px-2 py-0.5 text-xs font-medium text-emerald-700 dark:text-emerald-300">
            {s.output.gateway_name} {s.split}%
          </span>
        ))}
      </div>
    ) : <p className="text-sm text-slate-400 italic">No splits configured.</p>
  }
  if (type === 'single') {
    const conn = data as { gateway_name: string } | undefined
    return conn ? (
      <span className="rounded-full bg-slate-100 dark:bg-[#1a1f2a] px-2 py-0.5 text-xs font-medium text-slate-600 dark:text-[#8090a8]">
        {conn.gateway_name}
      </span>
    ) : <p className="text-sm text-slate-400 italic">No connector configured.</p>
  }
  return null
}

// Hover/focus affordance that moves a long explanation off the page into an info icon. The tooltip
// renders through a portal to document.body with fixed positioning, because the enclosing Card is
// overflow-hidden and would clip an ordinary absolutely-positioned bubble. The native `title`
// attribute (its previous implementation) only showed the browser's slow, unreliable tooltip.
function InfoHint({ text }: { text: string }) {
  const [coords, setCoords] = useState<{ x: number; y: number } | null>(null)
  return (
    <span
      tabIndex={0}
      aria-label={text}
      className="inline-flex cursor-help align-middle text-slate-400 hover:text-slate-600 focus:text-slate-600 focus:outline-none dark:text-slate-500 dark:hover:text-slate-300 dark:focus:text-slate-300"
      onMouseEnter={e => setCoords({ x: e.clientX, y: e.clientY })}
      onMouseLeave={() => setCoords(null)}
      onFocus={e => { const r = e.currentTarget.getBoundingClientRect(); setCoords({ x: r.left + r.width / 2, y: r.bottom }) }}
      onBlur={() => setCoords(null)}
    >
      <Info size={12} />
      {coords && createPortal(
        <span
          role="tooltip"
          style={{
            position: 'fixed',
            left: Math.min(coords.x + 14, window.innerWidth - 252),
            top: coords.y + 16,
            maxWidth: 240,
          }}
          className="pointer-events-none z-[200] block w-max rounded-lg bg-slate-900 px-2.5 py-1.5 text-xs font-normal leading-snug text-white shadow-lg dark:bg-slate-700"
        >
          {text}
        </span>,
        document.body,
      )}
    </span>
  )
}

// Compact field label: name + required marker + optional info tooltip, replacing verbose
// helper paragraphs under each input.
function FieldLabel({ children, hint, required }: { children: ReactNode; hint?: string; required?: boolean }) {
  return (
    <label className={`mb-1.5 flex items-center gap-1 ${type.label}`}>
      {children}{required && <span className="text-slate-400">*</span>}
      {hint && <InfoHint text={hint} />}
    </label>
  )
}

/** One input/select treatment for the experiment form, so fields don't drift apart field by field. */
const fieldCls =
  'border border-slate-200 bg-transparent rounded-lg px-3 py-1.5 text-sm ' +
  'focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]'

// Visually separates the create form's three groups (what you're comparing / traffic & duration /
// safety) with a hairline divider. No headings: each field labels itself and the arms are already
// visually distinct, so a "What you're comparing" heading just names the obvious.
function FormSection({ children, divide }: { children: ReactNode; divide?: boolean }) {
  return (
    <section className={`space-y-4 ${divide ? 'border-t border-slate-200 dark:border-[#262d3a] pt-6' : ''}`}>
      {children}
    </section>
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
function ArmSelector({ label, help, accent, algorithms, value, excludeId, allowedSrStrategies, liveSrConfig, onChange }: {
  label: string
  help: string
  // Variant arm (accent) vs control arm — drives the colored pill + panel tint so the two are
  // visually distinct and can't be misread for each other across the form.
  accent?: boolean
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

  const selectCls = `w-full ${fieldCls}`

  return (
    <div className={`rounded-xl border p-3 ${accent
      ? 'border-brand-200 bg-brand-50/40 dark:border-brand-800/50 dark:bg-brand-900/10'
      : 'border-slate-200 bg-slate-50/50 dark:border-[#222226] dark:bg-[#0c0c10]'}`}>
      <div className="mb-2 flex items-center gap-1.5">
        <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold ${accent
          ? 'bg-brand-100 text-brand-700 dark:bg-brand-900/40 dark:text-brand-300'
          : 'bg-slate-200 text-slate-600 dark:bg-slate-700 dark:text-slate-200'}`}>
          {label}
        </span>
        <InfoHint text={help} />
      </div>
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
        <p className="mt-1.5 text-[13px] text-slate-500 dark:text-[#8d96aa]">Using <span className="font-medium text-slate-600 dark:text-slate-300">{configs[0].name}</span></p>
      )}
      {value && !isSrStrategy(value) && (
        <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
          <ArmRuleDetail algorithmId={value} algorithms={algorithms} />
        </div>
      )}
      {value && isSrStrategy(value) && (
        <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
          <p className="text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] mb-1.5">Base SR config</p>
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
  if (verdict === 'not_significant') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
      <CheckCircle2 size={11} /> No significant difference
    </span>
  )
  return null
}

// ─── SR Config param display helpers ──────────────────────────────────────────

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
      <div className="flex items-center justify-between text-[13px]">
        <span className="text-slate-500">Hedging %</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {hedging !== null ? `${hedging}%` : <span className="text-slate-400 italic">Uses default</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[13px]">
        <span className="text-slate-500">Elimination threshold</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {elimination !== null ? `Drops below ${(elimination * 100).toFixed(0)}% score` : <span className="text-slate-400 italic">Uses default</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[13px]">
        <span className="text-slate-500">Bucket size</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {bucketSize !== null ? `${bucketSize} requests` : <span className="text-slate-400 italic">Uses default</span>}
        </span>
      </div>
      {honorsAutopilot && autopilotFeatureOn && (
        <p className="flex items-center gap-1 text-[12px] text-amber-600 dark:text-amber-400 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330]">
          Auto-tunes bucket size and hedging % per segment, based on your traffic volume.
          <InfoHint text={
            autopilotSegmentCount > 0
              ? `Autopilot is tuning ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'} (by payment method / network / currency / country) — bucket size and hedging % can differ per segment and over time, so the values above are just the base config.`
              : `Autopilot will start tuning bucket size and hedging % per segment once enough traffic flows through it. Until then, every transaction uses the base config shown above.`
          } />
        </p>
      )}
      {honorsAutopilot && !autopilotFeatureOn && autopilotSegmentCount > 0 && (
        <p className="flex items-center gap-1 text-[12px] text-slate-400 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330]">
          Autopilot is off — {autopilotSegmentCount} segment{autopilotSegmentCount === 1 ? '' : 's'} from earlier tuning
          <InfoHint text={`Autopilot (auto-calibration) is currently disabled for this merchant. ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'} still carry values it tuned before being turned off, but nothing is being actively adjusted right now — every transaction uses the base config shown above.`} />
        </p>
      )}
    </div>
  )
}

// ─── Experiment detail panel ──────────────────────────────────────────────────

// A metric delta is only a "result" once the backend z-test declares a winner, loser, or guardrail
// breach. collecting_data / not_significant means the delta is still noise — the UI must not paint
// it like a decision (the premature-green-number trap).
function isSignificantVerdict(verdict: string): boolean {
  return verdict === 'variant_wins' || verdict === 'variant_loses' || verdict === 'guardrail_breached'
}

/**
 * Statistical caution shown above the metrics table while the verdict isn't a decisive win/loss.
 * Says only what the progress bar doesn't — the *interpretation*. Distinguishes the two non-decisive
 * verdicts: `collecting_data` (target not reached — keep waiting) vs `not_significant` (target
 * reached, z-test ran, no detectable winner — a final, inconclusive result, NOT a "keep waiting"
 * state). Conflating them makes a completed experiment look stuck at 100%.
 */
function ConfidenceBanner({ verdict }: { verdict: string }) {
  // `not_significant` is only returned after the sample gate passes (see compute_significance),
  // so it always means the target was reached.
  if (verdict === 'not_significant') {
    return (
      <div className="flex items-start gap-2.5 rounded-xl border border-slate-300 bg-slate-50 px-4 py-3 dark:border-slate-600/40 dark:bg-slate-500/10">
        <Info size={16} className="mt-0.5 shrink-0 text-slate-500 dark:text-slate-400" />
        <p className="text-[13px] text-slate-700 dark:text-slate-300">
          <span className="font-medium text-slate-900 dark:text-slate-100">Sample target reached — no significant difference.</span>{' '}
          Control and variant are statistically tied: the delta below sits within the 95% confidence
          interval, so neither strategy is a proven winner. Collecting more traffic won't change this
          unless the true gap is larger than the current split can detect — a wider variant allocation
          would tighten the interval faster.
        </p>
      </div>
    )
  }
  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-500/30 dark:bg-amber-500/10">
      <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-600 dark:text-amber-400" />
      <p className="text-[13px] text-amber-800 dark:text-amber-300/90">
        <span className="font-medium text-amber-900 dark:text-amber-200">Not statistically significant yet.</span>{' '}
        The deltas below are still within noise — let the experiment reach its sample target before drawing conclusions.
      </p>
    </div>
  )
}

/** Control (slate) / Variant (brand) column-header pill, reused by both comparison tables. */
function ArmTh({ label, pct, accent }: { label: string; pct: number; accent?: boolean }) {
  return (
    <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold ${accent
      ? 'bg-brand-100 text-brand-700 dark:bg-brand-900/40 dark:text-brand-300'
      : 'bg-slate-200 text-slate-600 dark:bg-slate-700 dark:text-slate-200'}`}>
      {label} · {pct}%
    </span>
  )
}

/** Config side-by-side: replaces the nested arm cards + routing-rule cards with one matrix. */
function ConfigComparisonTable({ abData, isTuning, controlPct, variantPct, live, autopilotOn, algorithmName, algorithms }: {
  abData: ABTestAlgorithmData
  isTuning: boolean
  controlPct: number
  variantPct: number
  live: { hedging: number | null; elimination: number | null; bucketSize: number | null }
  autopilotOn: boolean
  algorithmName: (id: string) => string
  algorithms: RoutingAlgorithm[]
}) {
  const controlSr = abData.control_algorithm_id === 'sr_routing'
  const variantSr = abData.variant_algorithm_id === 'sr_routing'
  const muted = (t: string) => <span className="italic text-slate-400">{t}</span>
  const dash = <span className="text-slate-300 dark:text-slate-600">—</span>
  const emphasis = (v: ReactNode) => <span className="font-medium text-brand-600 dark:text-brand-400">{v}</span>
  const elimText = (e: number) => `Drops below ${(e * 100).toFixed(0)}% score`

  const rows: { label: string; control: ReactNode; variant: ReactNode }[] = [
    {
      label: 'Strategy',
      control: armLabel(abData.control_algorithm_id, abData.control_sr_config, algorithmName),
      variant: armLabel(abData.variant_algorithm_id, abData.variant_sr_config, algorithmName),
    },
  ]

  if (isTuning) {
    // Same SR algorithm, control on live config, variant on its overrides.
    const vHedge = abData.variant_sr_config?.hedging_percent
    const vElim = abData.variant_sr_config?.elimination_threshold
    rows.push({
      label: 'Hedging %',
      control: live.hedging != null ? `${live.hedging}%` : muted('Uses default'),
      variant: typeof vHedge === 'number' ? emphasis(`${vHedge}%`) : muted('Same as control'),
    })
    rows.push({
      label: 'Elimination threshold',
      control: live.elimination != null ? elimText(live.elimination) : muted('Uses default'),
      variant: typeof vElim === 'number' ? emphasis(elimText(vElim)) : muted('Same as control'),
    })
  } else {
    if (controlSr || variantSr) {
      const cAuto = controlSr && abData.control_sr_config?.use_autopilot !== false && autopilotOn
      const vAuto = variantSr && abData.variant_sr_config?.use_autopilot !== false && autopilotOn
      const hedge = (isSr: boolean, auto: boolean) => !isSr ? dash : auto ? muted('Auto-tuned per segment') : live.hedging != null ? `${live.hedging}%` : muted('Uses default')
      const bucket = (isSr: boolean, auto: boolean) => !isSr ? dash : auto ? muted('Auto-tuned per segment') : live.bucketSize != null ? `${live.bucketSize} requests` : muted('Uses default')
      const elim = (isSr: boolean) => !isSr ? dash : live.elimination != null ? elimText(live.elimination) : muted('Uses default')
      rows.push({ label: 'Hedging %', control: hedge(controlSr, cAuto), variant: hedge(variantSr, vAuto) })
      rows.push({ label: 'Elimination threshold', control: elim(controlSr), variant: elim(variantSr) })
      rows.push({ label: 'Bucket size', control: bucket(controlSr, cAuto), variant: bucket(variantSr, vAuto) })
    }
    if (!controlSr || !variantSr) {
      rows.push({
        label: 'Routing rule',
        control: controlSr ? muted('SR scoring') : <ArmRuleDetail algorithmId={abData.control_algorithm_id} algorithms={algorithms} />,
        variant: variantSr ? muted('SR scoring') : <ArmRuleDetail algorithmId={abData.variant_algorithm_id} algorithms={algorithms} />,
      })
    }
  }

  return (
    <div className="overflow-x-auto rounded-xl border border-slate-200 dark:border-[#222226]">
      <table className="w-full min-w-[520px] text-sm">
        <thead>
          <tr className="border-b border-slate-200 bg-slate-50 text-left dark:border-[#222226] dark:bg-[#0c0c10]">
            <th className="w-[28%] px-4 py-2.5 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">Attribute</th>
            <th className="px-4 py-2.5"><ArmTh label="Control" pct={controlPct} /></th>
            <th className="px-4 py-2.5"><ArmTh label="Variant" pct={variantPct} accent /></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={r.label} className={i > 0 ? 'border-t border-slate-100 dark:border-[#1a1a22]' : ''}>
              <td className="px-4 py-2.5 align-top text-[13px] font-medium text-slate-500 dark:text-[#8d96aa]">{r.label}</td>
              <td className="px-4 py-2.5 align-top text-slate-700 dark:text-slate-200">{r.control}</td>
              <td className="px-4 py-2.5 align-top text-slate-700 dark:text-slate-200">{r.variant}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

/** Row-aligned performance comparison. Deltas stay neutral until the verdict is significant. */
function MetricsComparisonTable({ results, controlPct, variantPct, costKind, significant }: {
  results: ExperimentResultsResponse
  controlPct: number
  variantPct: number
  costKind: boolean
  significant: boolean
}) {
  const c = results.control
  const v = results.variant
  const narDelta = (v.auth_rate - c.auth_rate) * 100
  const faarDelta = (v.first_attempt_auth_rate - c.first_attempt_auth_rate) * 100
  const tcsDelta = (v.total_cost_saved ?? 0) - (c.total_cost_saved ?? 0)
  // Colour a delta only once the verdict is real — otherwise an early, noisy number would read as a win.
  const deltaCls = (d: number) => !significant
    ? 'text-slate-400 dark:text-slate-500'
    : d > 0 ? 'text-emerald-600 dark:text-emerald-400' : d < 0 ? 'text-red-500' : 'text-slate-500'
  const num = 'px-4 py-2.5 text-right text-slate-700 dark:text-slate-200'
  const metricCell = (label: string, sub: string) => (
    <td className="px-4 py-2.5"><span className="font-medium text-slate-700 dark:text-slate-200">{label}</span> <span className="text-[12px] text-slate-400">{sub}</span></td>
  )

  return (
    <div className="overflow-x-auto rounded-xl border border-slate-200 dark:border-[#222226]">
      <table className="w-full min-w-[520px] text-sm [font-variant-numeric:tabular-nums]">
        <thead>
          <tr className="border-b border-slate-200 bg-slate-50 text-left dark:border-[#222226] dark:bg-[#0c0c10]">
            <th className="px-4 py-2.5 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">Metric</th>
            <th className="px-4 py-2.5 text-right"><ArmTh label="Control" pct={controlPct} /></th>
            <th className="px-4 py-2.5 text-right"><ArmTh label="Variant" pct={variantPct} accent /></th>
            <th className="px-4 py-2.5 text-right text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">Delta</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            {metricCell('Net auth rate', 'NAR')}
            <td className={num}>{authRatePct(c.auth_rate)}</td>
            <td className={num}>{authRatePct(v.auth_rate)}</td>
            <td className={`px-4 py-2.5 text-right font-medium ${deltaCls(narDelta)}`}>{deltaLabel(narDelta)}</td>
          </tr>
          <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
            {metricCell('First-attempt rate', 'FAAR')}
            <td className={num}>{authRatePct(c.first_attempt_auth_rate)}</td>
            <td className={num}>{authRatePct(v.first_attempt_auth_rate)}</td>
            <td className={`px-4 py-2.5 text-right font-medium ${deltaCls(faarDelta)}`}>{deltaLabel(faarDelta)}</td>
          </tr>
          <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
            {metricCell('Transactions', 'count')}
            <td className={num}>{c.transaction_count.toLocaleString()}</td>
            <td className={num}>{v.transaction_count.toLocaleString()}</td>
            <td className="px-4 py-2.5 text-right text-slate-300 dark:text-slate-600">—</td>
          </tr>
          <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
            {metricCell('Outcome', 'success / fail')}
            <td className="px-4 py-2.5 text-right"><span className="text-emerald-600 dark:text-emerald-400">{c.success_count.toLocaleString()}</span> <span className="text-slate-300 dark:text-slate-600">/</span> <span className="text-red-500">{c.failure_count.toLocaleString()}</span></td>
            <td className="px-4 py-2.5 text-right"><span className="text-emerald-600 dark:text-emerald-400">{v.success_count.toLocaleString()}</span> <span className="text-slate-300 dark:text-slate-600">/</span> <span className="text-red-500">{v.failure_count.toLocaleString()}</span></td>
            <td className="px-4 py-2.5 text-right text-slate-300 dark:text-slate-600">—</td>
          </tr>
          {costKind && (
            <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
              {metricCell('Cost saved', 'TCS')}
              <td className="px-4 py-2.5 text-right text-sky-600 dark:text-sky-400">{c.total_cost_saved != null ? c.total_cost_saved.toLocaleString(undefined, { maximumFractionDigits: 2 }) : '—'}</td>
              <td className="px-4 py-2.5 text-right text-sky-600 dark:text-sky-400">{v.total_cost_saved != null ? v.total_cost_saved.toLocaleString(undefined, { maximumFractionDigits: 2 }) : '—'}</td>
              <td className={`px-4 py-2.5 text-right font-medium ${deltaCls(tcsDelta)}`}>{`${tcsDelta > 0 ? '+' : ''}${tcsDelta.toLocaleString(undefined, { maximumFractionDigits: 2 })}`}</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  )
}

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
  // Duplicate the experiment's config into a fresh create form. Safe in any state (it only reads
  // this experiment and pre-fills a new one), so it's offered for active and inactive alike.
  onClone: () => void
  // Live-traffic (ab-test-real-payments) flag state — used only to derive the header status
  // ("Active" vs "Not collecting"). The pause/resume toggle was removed; recovery from a flag-off
  // state is handled by the page-level drift banner.
  realPaymentsOn: boolean
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
  onClone,
  realPaymentsOn,
}: DetailPanelProps) {
  const abData = (algorithm.algorithm_data || algorithm.algorithm)?.data as ABTestAlgorithmData | undefined
  const kind = abExperimentKind(abData)
  const isTuning = kind === 'sr_config_tuning'
  const costKind = hasCostArm(abData)
  const { liveHedging, liveElimination, liveBucketSize } = useLiveSrConfig(merchantId || undefined)
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

  // Page numbers to render in the transaction pager: always the first and last page, plus a
  // sliding window of up to WINDOW pages centred on the current one. `null` marks a gap that renders
  // as an ellipsis. The window is nudged inward near either edge so it stays WINDOW wide.
  function buildPageList(current: number, totalPages: number): (number | null)[] {
    const WINDOW = 5
    // When everything fits (window + the two anchors), just list every page.
    if (totalPages <= WINDOW + 2) {
      return Array.from({ length: totalPages }, (_, i) => i + 1)
    }
    const half = Math.floor(WINDOW / 2)
    let start = current - half
    let end = current + half
    if (start < 2) { end += 2 - start; start = 2 }
    if (end > totalPages - 1) { start -= end - (totalPages - 1); end = totalPages - 1 }
    start = Math.max(2, start)
    end = Math.min(totalPages - 1, end)

    const pages: (number | null)[] = [1]
    if (start > 2) pages.push(null)
    for (let p = start; p <= end; p++) pages.push(p)
    if (end < totalPages - 1) pages.push(null)
    pages.push(totalPages)
    return pages
  }

  const totalTxns = results ? results.control.transaction_count + results.variant.transaction_count : 0
  const minSample = abData?.min_sample_size ?? 1000
  const progress = Math.min(100, Math.round((totalTxns / minSample) * 100))
  const controlPct = 100 - (abData?.variant_split_pct ?? 10)
  const variantPct = abData?.variant_split_pct ?? 10

  const status: 'active' | 'paused' | 'inactive' = isActive ? (realPaymentsOn ? 'active' : 'paused') : 'inactive'
  const significant = results ? isSignificantVerdict(results.verdict) : false

  // Rough "time to target" projection for the progress card. There's no served ingestion rate, so
  // the rate is derived from txns-so-far over the time the experiment has been live — `modified_at`
  // is a sound proxy for activation, since edits are blocked once an experiment is running. Only
  // shown while actively collecting with real progress; any missing/degenerate input returns null
  // rather than a misleading estimate.
  const remainingEta: string | null = (() => {
    if (status !== 'active' || totalTxns <= 0 || totalTxns >= minSample) return null
    const startMs = algorithm.modified_at ? Date.parse(algorithm.modified_at) : NaN
    if (!Number.isFinite(startMs)) return null
    const elapsedMs = Date.now() - startMs
    if (elapsedMs <= 0) return null
    const remainMs = (elapsedMs / totalTxns) * (minSample - totalTxns)
    const hrs = remainMs / 3_600_000
    if (!Number.isFinite(hrs) || hrs <= 0) return null
    const at = 'at current volume'
    if (hrs < 1) return `~${Math.max(1, Math.round(hrs * 60))} min remaining ${at}`
    if (hrs < 48) return `~${Math.round(hrs)} hrs remaining ${at}`
    return `~${Math.round(hrs / 24)} days remaining ${at}`
  })()

  const statCols = abData
    ? [
        { label: 'Traffic split', value: `${controlPct}% / ${variantPct}%` },
        { label: 'Sample target', value: `${minSample.toLocaleString()} txns` },
        { label: 'Guardrail', value: `${abData.guardrail_threshold_pp}pp` },
      ]
    : []

  return (
    <div className="space-y-6">
      {/* ── Summary card: header + config-at-a-glance + collection progress ── */}
      <div className="rounded-2xl border border-slate-200 bg-white px-5 py-5 dark:border-[#222226] dark:bg-[#0c0c10]">
        {/* Header + status + action bar */}
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-lg font-semibold tracking-tight text-slate-900 dark:text-white">{algorithm.name}</h2>
              <KindBadge kind={kind} />
              {status === 'active' && <Badge variant="green">Active</Badge>}
              {/* Flag-off drift (formerly "paused") — the live-traffic flag is off while the
                  experiment is active. Surfaced as a warning; the drift banner below offers recovery. */}
              {status === 'paused' && <Badge variant="orange">Not collecting</Badge>}
              {status === 'inactive' && <Badge variant="gray">Inactive</Badge>}
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {isActive ? (
              <>
                {/* Pause/Resume are disabled for now — live-traffic collection is governed by a
                    merchant-level flag, not per-experiment state. A flag that's off is surfaced as
                    the "not collecting" drift banner (with recovery) rather than a pause control. */}
                {/* Duplicate is safe on a running experiment — it only reads the config into a new
                    create form, never touching this one's traffic or results. */}
                <Button size="sm" variant="secondary" onClick={onClone}><Copy size={13} /> Duplicate</Button>
                <Button size="sm" variant="danger" onClick={onStop}><PowerOff size={13} /> Stop</Button>
              </>
            ) : (
              <>
                {/* Edit / Delete are only offered while inactive — a running experiment must be
                    stopped first to avoid corrupting its collected results (enforced server-side too).
                    Duplicate, being read-only on this experiment, is offered in every state. */}
                <Button size="sm" variant="secondary" onClick={onClone}><Copy size={13} /> Duplicate</Button>
                <Button size="sm" variant="secondary" onClick={onEdit}><Pencil size={13} /> Edit</Button>
                <Button size="sm" variant="secondary" onClick={onDelete}><Trash2 size={13} /> Delete</Button>
                <Button size="sm" variant="primary" onClick={onActivate}>Activate</Button>
              </>
            )}
          </div>
        </div>

        {/* Config at a glance — labelled columns split by hairline dividers */}
        {statCols.length > 0 && (
          <div className="mt-4 flex flex-wrap gap-y-3">
            {statCols.map((s, i) => (
              <div
                key={s.label}
                className={`min-w-[8rem] pr-6 ${i > 0 ? 'border-l border-slate-200 pl-6 dark:border-[#222226]' : ''}`}
              >
                <p className="text-[12px] text-slate-400 dark:text-[#8d96aa]">{s.label}</p>
                <p className="mt-0.5 text-[15px] font-semibold text-slate-800 dark:text-slate-100 [font-variant-numeric:tabular-nums]">{s.value}</p>
              </div>
            ))}
          </div>
        )}

        {/* Progress toward the sample target */}
        {results && (
          <div className="mt-5 space-y-2 border-t border-slate-100 pt-4 dark:border-[#1a1a22]">
            <div className="flex items-end justify-between">
              <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-slate-400 dark:text-[#8d96aa]">Transactions collected</p>
              <p className="text-[15px] font-semibold text-slate-800 dark:text-slate-100 [font-variant-numeric:tabular-nums]">
                {totalTxns.toLocaleString()} <span className="text-slate-400">/ {minSample.toLocaleString()}</span>
              </p>
            </div>
            <div className="h-2.5 overflow-hidden rounded-full bg-slate-100 dark:bg-[#232833]">
              <div
                className={`h-full rounded-full transition-all duration-500 ${significant ? 'bg-emerald-500' : 'bg-brand-500'}`}
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="text-[12px] text-slate-400">
              {progress}% of the {minSample.toLocaleString()}-transaction target{remainingEta ? ` · ${remainingEta}` : ''}
            </p>
          </div>
        )}
      </div>

      {/* ── Configuration comparison ── */}
      {abData && (
        <section className="space-y-2.5 border-t border-slate-200 dark:border-[#262d3a] pt-6">
          <h3 className={type.heading}>Configuration</h3>
          <ConfigComparisonTable
            abData={abData}
            isTuning={isTuning}
            controlPct={controlPct}
            variantPct={variantPct}
            live={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize }}
            autopilotOn={autopilotFeatureOn}
            algorithmName={algorithmName}
            algorithms={algorithms}
          />
        </section>
      )}

      {/* ── Results ── */}
      <section className="space-y-3 border-t border-slate-200 dark:border-[#262d3a] pt-6">
        <div className="flex items-center justify-between">
          <div>
            <h3 className={type.heading}>Results</h3>
            <p className="mt-0.5 text-[12px] text-slate-400">Updates every 60 seconds</p>
          </div>
          {results && <VerdictChip verdict={results.verdict} />}
        </div>
        {isLoading && !results ? (
          <div className="flex items-center gap-2 text-sm text-slate-400"><Spinner size={14} /> Loading stats…</div>
        ) : !results ? (
          <p className="text-sm italic text-slate-400">
            Stats unavailable — analytics pipeline may not be configured in this environment.
          </p>
        ) : (
          <div className="space-y-4">
            {/* Statistical rigor first: while the verdict isn't trustworthy, lead with the confidence
                notice and keep every delta neutral (handled inside the table). */}
            {!significant && <ConfidenceBanner verdict={results.verdict} />}
            <MetricsComparisonTable
              results={results}
              controlPct={controlPct}
              variantPct={variantPct}
              costKind={costKind}
              significant={significant}
            />
            {results.verdict === 'guardrail_breached' && (
              <div className="flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600 dark:border-red-800 dark:bg-red-900/20 dark:text-red-400">
                <ShieldAlert size={12} />
                Variant auth rate dropped {Math.abs(results.delta_pp).toFixed(2)}pp below control — beyond the {abData?.guardrail_threshold_pp}pp guardrail. Consider stopping the experiment.
              </div>
            )}
          </div>
        )}
      </section>

      {/* ── Transactions ── */}
      <section className="space-y-3 border-t border-slate-200 dark:border-[#262d3a] pt-6">
        <div className="flex items-center justify-between">
          <div>
            <h3 className={type.heading}>Transactions</h3>
            <p className="mt-0.5 text-[12px] text-slate-400">
              {txnData ? `${txnData.total.toLocaleString()} decisions` : 'Loading…'}
            </p>
          </div>
          {txnsLoading && <Spinner size={14} />}
        </div>
        <div className="overflow-x-auto rounded-xl border border-slate-200 dark:border-[#222226]">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs text-slate-500 bg-slate-50 dark:bg-[#0c0c10] border-b border-slate-200 dark:border-[#222226]">
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
            <table className="w-full text-base">
              <tbody>
                {!txnData?.transactions.length ? (
                  <tr>
                    <td colSpan={6} className="px-4 py-8 text-base text-slate-400 text-center">
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
                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold ${txn.variant_arm === 'control'
                            ? 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
                            : 'bg-brand-100 text-brand-700 dark:bg-brand-900/30 dark:text-brand-300'
                          }`}>
                          {txn.variant_arm === 'control' ? 'Control' : 'Variant'}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-sm text-slate-500 dark:text-slate-400 whitespace-nowrap">
                        {routingType(txn.variant_arm)}
                      </td>
                      <td className="px-4 py-3 font-mono text-sm text-slate-600 dark:text-slate-400 max-w-[180px] truncate">
                        {txn.payment_id}
                      </td>
                      <td className="px-4 py-3 text-sm text-slate-700 dark:text-slate-300">
                        {txn.gateway ?? '—'}
                      </td>
                      <td className="px-4 py-3">
                        {txn.status === 'success' ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400">success</span>
                        ) : txn.status === 'failure' ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400">failure</span>
                        ) : (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400" title="Payment was routed but no outcome was recorded — counted against auth rate">
                            <Clock size={9} /> no outcome
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-sm text-slate-400 whitespace-nowrap">
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
                <div className="flex items-center justify-between px-5 py-4 border-t border-slate-100 dark:border-[#1e2330]">
                  <p className="text-sm text-slate-500">
                    Page {txnPage} of {totalPages} · {txnData.total.toLocaleString()} total
                  </p>
                  <div className="flex items-center gap-1">
                    <button
                      type="button"
                      onClick={() => setTxnPage(p => Math.max(1, p - 1))}
                      disabled={txnPage === 1 || txnsLoading}
                      className="px-2.5 py-1 rounded-md border border-slate-200 dark:border-[#222226] text-sm text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                    >
                      ← Prev
                    </button>
                    {buildPageList(txnPage, totalPages).map((page, idx) => (
                      page === null
                        ? <span key={`ellipsis-${idx}`} className="px-1 text-sm text-slate-400">…</span>
                        : <button
                          key={page}
                          type="button"
                          onClick={() => setTxnPage(page)}
                          disabled={txnsLoading}
                          className={`min-w-[28px] px-2 py-1 rounded-md border text-sm transition-colors ${page === txnPage
                              ? 'border-brand-500 bg-brand-500 text-white'
                              : 'border-slate-200 dark:border-[#222226] text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22]'
                            }`}
                        >
                          {page}
                        </button>
                    ))}
                    <button
                      type="button"
                      onClick={() => setTxnPage(p => Math.min(totalPages, p + 1))}
                      disabled={txnPage === totalPages || txnsLoading}
                      className="px-2.5 py-1 rounded-md border border-slate-200 dark:border-[#222226] text-sm text-slate-600 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-[#1a1a22] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                    >
                      Next →
                    </button>
                  </div>
                </div>
              )
            })()}
          </div>
        </div>
      </section>
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
      <span className="inline-flex items-center rounded-full bg-brand-100 px-2 py-0.5 text-[11px] font-semibold text-brand-700 dark:bg-brand-900/40 dark:text-brand-300">
        {label} ({splitPct}%)
      </span>

      <div className="space-y-3">
        <div>
          <label className={`mb-1.5 flex items-center gap-1 ${type.label}`}>
            Hedging %
            <InfoHint text="Share of traffic sent to non-top gateways to keep their scores fresh (the explore-exploit tradeoff)." />
          </label>
          <input
            type="number" min={0} max={100} step={1}
            value={config.hedgingPercent ?? ''}
            placeholder="e.g. 5"
            onChange={e => onChange(c => ({ ...c, hedgingPercent: e.target.value === '' ? null : Number(e.target.value) }))}
            className={`w-full ${fieldCls}`}
          />
        </div>

        <div>
          <label className={`mb-1.5 flex items-center gap-1 ${type.label}`}>
            Elimination threshold (0–1)
            <InfoHint text="SR score (0–1) below which a gateway is dropped from routing." />
          </label>
          <input
            type="number" min={0} max={1} step={0.01}
            value={config.eliminationThreshold ?? ''}
            placeholder="e.g. 0.70"
            onChange={e => onChange(c => ({ ...c, eliminationThreshold: e.target.value === '' ? null : Number(e.target.value) }))}
            className={`w-full ${fieldCls}`}
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
  // Present only when there's a list to return to (i.e. experiments already exist).
  onCancel?: () => void
}

function CreateForm({
  form, setForm, eligibleAlgorithms, saving, error, success, createdId,
  merchantId, isEditing, onCreate, onActivateCreated, onCancel,
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

  // "Custom" is active when the sample target isn't one of the presets — either the user chose it,
  // or an edited experiment carries an off-preset value.
  const [customSample, setCustomSample] = useState(!SAMPLE_SIZE_PRESETS.includes(form.minSampleSize))

  // iOS-style segmented control: both options live in one gray track, the active one gets a clean
  // white fill and a soft shadow rather than a harsh solid-black fill.
  const tabClass = (type: ABTestExperimentType) =>
    `inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${form.experimentType === type
      ? 'bg-white text-slate-900 shadow-sm dark:bg-[#2a3140] dark:text-white'
      : 'text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-white'
    }`

  return (
    <Card>
      <CardHeader>
        <h2 className={type.heading}>{isEditing ? 'Edit experiment' : 'New experiment'}</h2>
      </CardHeader>
      <CardBody className="space-y-6">

        {/* Edit mode rebuilds no config — only the name is mutable — so it shows just that field and
            skips the grouped create sections below. */}
        {isEditing && (
          <div>
            <FieldLabel required>Experiment name</FieldLabel>
            <input
              className={`w-full ${fieldCls}`}
              value={form.name}
              onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
            />
            <p className="mt-1.5 text-[13px] text-slate-500 dark:text-[#8d96aa]">
              Only the name can be changed. To test a different routing setup, create a new experiment.
            </p>
          </div>
        )}

        {!isEditing && (
          <>
            {/* ── 1 · What you're comparing ── */}
            <FormSection>
              <div>
                <label className={`mb-2 block ${type.label}`}>Experiment type</label>
                <div className="inline-flex rounded-lg border border-slate-200 bg-slate-100 p-0.5 dark:border-[#222226] dark:bg-[#14181f]">
                  <button type="button" className={tabClass('algorithm_comparison')} onClick={() => setForm(f => ({ ...f, experimentType: 'algorithm_comparison' }))}>
                    <GitCompare size={13} /> Algorithm comparison
                  </button>
                  <button type="button" className={tabClass('sr_config_tuning')} onClick={() => setForm(f => ({ ...f, experimentType: 'sr_config_tuning' }))}>
                    <Sliders size={13} /> SR config tuning
                  </button>
                </div>
                {/* Helper text in a dedicated info box, so it reads as guidance rather than floating
                    muted text under the control. */}
                <div className="mt-2.5 flex items-start gap-2 rounded-lg bg-slate-50 px-3 py-2 dark:bg-[#14181f]">
                  <Info size={14} className="mt-0.5 shrink-0 text-slate-400" />
                  <p className="text-[13px] text-slate-500 dark:text-[#9ca7ba]">
                    {EXPERIMENT_TYPE_HELP[form.experimentType]}
                  </p>
                </div>
              </div>

              <div>
                <FieldLabel required>Experiment name</FieldLabel>
                <input
                  className={`w-full ${fieldCls}`}
                  placeholder={form.experimentType === 'sr_config_tuning' ? 'e.g. Hedging 10% vs 5%' : 'e.g. Stripe vs Checkout.com'}
                  value={form.name}
                  onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                />
              </div>

              {form.experimentType === 'algorithm_comparison' && (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <ArmSelector
                    label="Control"
                    help="Your current strategy — the baseline."
                    algorithms={eligibleAlgorithms}
                    value={form.controlAlgorithmId}
                    excludeId={form.variantAlgorithmId}
                    allowedSrStrategies={allowedSrStrategies}
                    liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
                    onChange={id => setForm(f => ({ ...f, controlAlgorithmId: id }))}
                  />
                  <ArmSelector
                    label="Variant"
                    help="The new strategy you want to test."
                    accent
                    algorithms={eligibleAlgorithms}
                    value={form.variantAlgorithmId}
                    excludeId={form.controlAlgorithmId}
                    allowedSrStrategies={allowedSrStrategies}
                    liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
                    onChange={id => setForm(f => ({ ...f, variantAlgorithmId: id }))}
                  />
                </div>
              )}

              {form.experimentType === 'sr_config_tuning' && (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {/* Control — live config, non-editable */}
                  <div className="rounded-xl border border-slate-200 dark:border-[#222226] bg-slate-50/50 dark:bg-[#0c0c10] px-4 py-4 space-y-3">
                    <div className="flex items-center gap-2">
                      <span className="inline-flex items-center rounded-full bg-slate-200 px-2 py-0.5 text-[11px] font-semibold text-slate-600 dark:bg-slate-700 dark:text-slate-200">
                        Control ({100 - form.variantSplitPct}%)
                      </span>
                      <span className="text-[12px] text-slate-400">current config</span>
                    </div>
                    <div className="space-y-2.5">
                      <div>
                        <p className="text-[13px] text-slate-500 mb-0.5">Hedging %</p>
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                          {liveHedging !== null ? `${liveHedging}%` : <span className="text-slate-400 italic text-xs">Uses default</span>}
                        </p>
                      </div>
                      <div>
                        <p className="text-[13px] text-slate-500 mb-0.5">Elimination threshold</p>
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                          {liveElimination !== null ? `Drops below ${(liveElimination * 100).toFixed(0)}% score` : <span className="text-slate-400 italic text-xs">Uses default</span>}
                        </p>
                      </div>
                    </div>
                    <p className="text-[12px] text-slate-400 pt-1 border-t border-slate-100 dark:border-[#1e2330]">
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
            </FormSection>

            {/* ── 2 · Traffic & duration ── */}
            <FormSection divide>
              <div>
                <div className="mb-2 flex items-center justify-between">
                  <span className={type.label}>Traffic allocation</span>
                  <span className="text-[13px] tabular-nums text-slate-500 dark:text-slate-400">
                    {100 - form.variantSplitPct}% control
                    <span className="mx-1.5 text-slate-300 dark:text-slate-600">/</span>
                    <span className="font-semibold text-brand-600 dark:text-brand-400">{form.variantSplitPct}% variant</span>
                  </span>
                </div>
                {/* One control, not two: the dual-color bar IS the slider. A transparent native range
                    sits on top for drag + keyboard + a11y (min/max 0–100 so the thumb tracks the true
                    proportion), and the value is clamped to 5–30% on change. The visible handle and the
                    segment widths are both driven by variantSplitPct, so they stay aligned. */}
                <div className="relative rounded-full py-2 focus-within:ring-2 focus-within:ring-brand-500/40">
                  <div className="flex h-2.5 w-full overflow-hidden rounded-full">
                    <div className="bg-slate-200 dark:bg-[#232833] transition-all duration-100" style={{ width: `${100 - form.variantSplitPct}%` }} />
                    <div className="flex-1 bg-brand-500" />
                  </div>
                  <div
                    className="pointer-events-none absolute top-1/2 h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-brand-500 bg-white shadow-sm dark:bg-slate-900"
                    style={{ left: `${100 - form.variantSplitPct}%` }}
                  />
                  <input
                    type="range" min={0} max={100} step={1}
                    value={100 - form.variantSplitPct}
                    onChange={e => {
                      const variant = Math.min(30, Math.max(5, 100 - Number(e.target.value)))
                      setForm(f => ({ ...f, variantSplitPct: variant }))
                    }}
                    className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                    aria-label="Control traffic percentage"
                  />
                </div>
              </div>

              <div>
                <FieldLabel hint="Transactions the experiment collects before it reports a significance verdict.">Sample target</FieldLabel>
                {/* One segmented control: presets plus a Custom slot that turns into an input in place,
                    so it reads as a single choice rather than buttons competing with a stray field. */}
                <div className="inline-flex flex-wrap items-center gap-0.5 rounded-lg border border-slate-200 p-0.5 dark:border-[#222226]">
                  {SAMPLE_SIZE_PRESETS.map(n => {
                    const active = !customSample && form.minSampleSize === n
                    return (
                      <button
                        key={n} type="button"
                        onClick={() => { setCustomSample(false); setForm(f => ({ ...f, minSampleSize: n })) }}
                        className={`rounded-md px-3 py-1.5 text-xs font-medium tabular-nums transition-colors ${active
                            ? 'bg-brand-500 text-white'
                            : 'text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-white'
                          }`}
                      >
                        {n.toLocaleString()}
                      </button>
                    )
                  })}
                  {customSample ? (
                    // type=text (not number) keeps the thousands separator so the value reads like the presets.
                    <input
                      type="text" inputMode="numeric" autoFocus
                      placeholder="Custom"
                      className="w-24 rounded-md bg-transparent px-2.5 py-1.5 text-xs tabular-nums focus:outline-none focus:ring-1 focus:ring-brand-500"
                      value={form.minSampleSize ? form.minSampleSize.toLocaleString() : ''}
                      onChange={e => setForm(f => ({ ...f, minSampleSize: Number(e.target.value.replace(/[^\d]/g, '')) }))}
                    />
                  ) : (
                    <button
                      type="button"
                      onClick={() => setCustomSample(true)}
                      className="rounded-md px-3 py-1.5 text-xs font-medium text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-white"
                    >
                      Custom…
                    </button>
                  )}
                </div>
              </div>
            </FormSection>

            {/* ── 3 · Safety ── */}
            <FormSection divide>
              {/* One inline sentence with the value set into it, rather than a stacked label + input +
                  suffix. The pp-vs-% nuance lives in the tooltip so the line stays a single row. */}
              <label className="flex flex-wrap items-center gap-x-2 gap-y-2 text-sm text-slate-700 dark:text-[#c7cfdd]">
                <span>Flag if variant auth drops by</span>
                <input
                  type="number" min={0.5} max={20} step={0.5}
                  className={`w-16 ${fieldCls}`}
                  value={form.guardrailThresholdPp}
                  onChange={e => setForm(f => ({ ...f, guardrailThresholdPp: Number(e.target.value) }))}
                />
                <span>percentage points below control</span>
                <InfoHint text="Percentage points, not percent — a 3 here flags the test when the variant's auth rate is 3+ points under control (say 89% vs 92%)." />
              </label>
            </FormSection>

          </>
        )}

        <ErrorMessage error={error} />

        {success && (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-200 bg-emerald-50 px-5 py-4 text-base text-emerald-800 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-200">
            <span>{success}</span>
            {createdId && (
              <Button size="sm" variant="primary" onClick={() => onActivateCreated(createdId)}>
                Activate now
              </Button>
            )}
          </div>
        )}

        {/* Primary action sits bottom-right, the conventional resting place for a form's commit;
            Cancel only appears when there's a list to go back to. */}
        <div className="flex items-center justify-end gap-2 border-t border-slate-100 dark:border-[#1e2330] pt-5">
          {onCancel && (
            <Button variant="secondary" onClick={onCancel} disabled={saving}>Cancel</Button>
          )}
          <Button variant="primary" onClick={onCreate} disabled={saving || !merchantId}>
            {saving ? <><Spinner size={14} /> {isEditing ? 'Saving…' : 'Creating…'}</> : isEditing ? 'Save changes' : 'Create experiment'}
          </Button>
        </div>
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

  // Activation routes live traffic through the experiment, but stats only record when the
  // `ab-test-real-payments` feature is on — a flag that lives on the SR Feature Flags tab. Rather
  // than let the two drift, activation enables it in the same step (see handleActivate/doActivate).
  const features = useMerchantFeatures(merchantId || undefined)
  const realPaymentsOn = features.isEnabled('ab-test-real-payments')

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
  const [enablingFlag, setEnablingFlag] = useState(false)
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

  // Clone = pre-fill the create form from an existing experiment's full config (arms, split,
  // sample, guardrail, SR overrides), with a distinct name. Unlike edit, this runs the normal
  // create path (validate + build payload), so it yields a brand-new experiment with its own
  // fresh data window — the original is left completely untouched.
  function openClone(algo: RoutingAlgorithm) {
    const values = toABTestFormValues(algo)
    if (!values) return
    setSearchParams({}, { replace: true })
    setForm({ ...values, name: `${values.name} (copy)` })
    setEditingId(null)
    setShowCreate(true)
    setSuccess(null)
    setError(null)
  }

  // Leave the form and return to the list/detail. Only reachable when experiments already exist —
  // with an empty list the form is the whole page, so there's nothing to cancel back to.
  function closeCreate() {
    setShowCreate(false)
    setEditingId(null)
    setForm({ ...DEFAULT_FORM })
    setError(null)
    setSuccess(null)
    if (activeAbTest) setSearchParams({ experiment: activeAbTest.id }, { replace: true })
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
    // Confirm when activating would replace a running experiment, OR when the live-traffic flag
    // still needs turning on — the dialog explains whichever applies (and both when both do).
    const switching = Boolean(activeAbTest && activeAbTest.id !== id)
    if (switching || !realPaymentsOn) { setPendingActivateId(id); return }
    await doActivate(id)
  }

  async function doActivate(id: string) {
    if (!merchantId) return
    try {
      // Turn on live-traffic A/B testing as part of activation, so an activated experiment always
      // actually records stats instead of silently collecting nothing.
      if (!realPaymentsOn) {
        await features.setFeatureEnabled('ab-test-real-payments', true)
      }
      await apiPost('/routing/activate', { created_by: merchantId, routing_algorithm_id: id })
      await Promise.all([mutateActive(), mutateAll()])
      setCreatedId(null); setSuccess(null)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to activate experiment')
    }
  }

  // Toggle live-traffic A/B testing — used by the drift banner (enable), and by the detail panel's
  // Pause/Resume (an active experiment keeps its results but stops/starts splitting traffic).
  async function toggleRealPayments(enabled: boolean) {
    setEnablingFlag(true)
    setError(null)
    try {
      await features.setFeatureEnabled('ab-test-real-payments', enabled)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : `Failed to ${enabled ? 'enable' : 'pause'} A/B testing on live traffic`)
    } finally {
      setEnablingFlag(false)
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
          <h1 className="text-lg font-semibold text-slate-900 dark:text-white">A/B Testing</h1>
          <p className={`mt-0.5 ${type.subheading}`}>
            Compare routing strategies on live traffic with statistical significance.
          </p>
        </div>
        {/* Only offer "New experiment" when the form isn't already open — otherwise it's a third
            door to an action the on-screen form already is (see the empty-state copy below). */}
        {rightPanelContent !== 'create' && (
          <Button variant="secondary" size="sm" onClick={openCreate}>
            <Plus size={14} /> New experiment
          </Button>
        )}
      </div>

      {/* Drift guard: an experiment is active but the live-traffic flag got turned off afterward
          (e.g. toggled on the SR Feature Flags tab), so it's silently collecting nothing. Now that
          the detail header no longer offers Pause/Resume, this banner is the single recovery path —
          shown on the detail view too. Gated on features.data so it never flashes before the flag
          state has loaded. */}
      {activeAbTest && features.data && !realPaymentsOn && (
        <div className="flex flex-wrap items-start gap-3 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-500/30 dark:bg-amber-500/10">
          <AlertTriangle size={18} className="mt-0.5 shrink-0 text-amber-600 dark:text-amber-400" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-amber-900 dark:text-amber-200">
              “{activeAbTest.name}” is active but not collecting results
            </p>
            <p className="mt-0.5 text-[13px] text-amber-800/90 dark:text-amber-300/90">
              “A/B test on real payments” is off, so live traffic isn’t being split between the arms. Turn it back on to resume recording stats.
            </p>
          </div>
          <Button size="sm" variant="primary" onClick={() => toggleRealPayments(true)} disabled={enablingFlag}>
            {enablingFlag ? <><Spinner size={13} /> Enabling…</> : 'Enable live-traffic testing'}
          </Button>
        </div>
      )}

      {/* With no experiments yet, the list rail would be an empty void beside the form — so the form
          takes the full width, centered. The split layout returns once there's a list to show. */}
      {merchantId && allAlgorithms && savedAbTests.length === 0 ? (
        <div className="mx-auto w-full max-w-3xl">
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
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-5">

          {/* Left: experiment list */}
          <div className="space-y-1">
            {!merchantId ? (
              <p className="px-2 py-2 text-base text-slate-400">Set merchant ID to load experiments.</p>
            ) : !allAlgorithms ? (
              <p className="px-2 py-2 text-sm text-slate-400">Loading…</p>
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
                      className={`relative w-full text-left pl-4 pr-3 py-3 transition-colors ${idx > 0 ? 'border-t border-slate-100 dark:border-[#1e2330]' : ''
                        } ${isSelected
                          ? 'bg-brand-50 dark:bg-brand-900/20'
                          : 'hover:bg-slate-50 dark:hover:bg-[#0f0f16]'
                        }`}
                    >
                      {/* Left accent bar makes the selected row identifiable at a glance,
                        independent of the (fairly subtle) background tint. */}
                      {isSelected && <span className="absolute left-0 top-0 bottom-0 w-0.5 bg-brand-500" />}
                      <div className="flex items-center gap-1.5 min-w-0">
                        <p className={`truncate text-base font-medium ${isSelected ? 'text-brand-700 dark:text-brand-300' : 'text-slate-800 dark:text-white'
                          }`}>
                          {algo.name}
                        </p>
                        {isActive && (
                          <span className="shrink-0 inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-full text-[11px] font-semibold bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400">
                            ● Active
                          </span>
                        )}
                      </div>
                      {abData && (
                        <p className="text-[13px] text-slate-500 dark:text-[#8d96aa] mt-0.5 truncate">
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
                onClone={() => openClone(selectedAlgo)}
                onDelete={() => setPendingDeleteId(selectedAlgo.id)}
                realPaymentsOn={realPaymentsOn}
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
                onCancel={closeCreate}
              />
            )}

            {rightPanelContent === 'empty' && (
              <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-200 dark:border-[#222226] py-16 text-center">
                <FlaskConical size={26} className="text-slate-300 dark:text-slate-600 mb-2" />
                <p className="text-base text-slate-500">Select an experiment to view details</p>
              </div>
            )}
          </div>
        </div>
      )}

      {(() => {
        // One dialog, three shapes: replacing a running experiment, first-time enabling live
        // traffic, or both at once. The copy states exactly what activating will do.
        const switching = Boolean(pendingActivateId && activeAbTest && activeAbTest.id !== pendingActivateId)
        const willEnableFlag = !realPaymentsOn
        const description = [
          switching ? 'An experiment is already running — activating this one replaces it.' : null,
          willEnableFlag
            ? 'This also turns on “A/B test on real payments”, so live traffic is split between the arms and results start collecting. You can stop the experiment at any time to revert to standard routing.'
            : null,
        ].filter(Boolean).join(' ')
        return (
          <ConfirmDialog
            open={pendingActivateId !== null}
            title={switching ? 'Switch active experiment?' : 'Route live traffic through this experiment?'}
            description={description}
            confirmLabel={willEnableFlag ? 'Enable & activate' : 'Yes, activate'}
            variant="primary"
            onConfirm={() => { const id = pendingActivateId!; setPendingActivateId(null); void doActivate(id) }}
            onCancel={() => setPendingActivateId(null)}
          />
        )
      })()}
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
