import { useState, useEffect, useRef, type ReactNode } from 'react'
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
import { useAuthStore, useCanEditRouting } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import {
  RoutingAlgorithm,
  ABTestAlgorithmData,
  ExperimentArm,
  ExperimentEndpoint,
  SrConfigOverride,
  ExperimentResultsResponse,
  ExperimentTransactionsResponse,
} from '../../types/api'
import { ShieldAlert, PowerOff, Plus, CheckCircle2, XCircle, Clock, AlertTriangle, Sliders, Pencil, Trash2, Info, Copy, ArrowLeft, ChevronRight, Zap, BarChart3 } from 'lucide-react'
import { HeaderFilter, HeaderSearch, RowMenu } from '../ui/TableControls'
import { formatLastModified, lastModifiedMs } from '../../lib/routingRuleTimestamps'
import * as type from '../ui/typography'
import { RuleBreakdown } from '../routing/euclid/RuleBreakdown'
import { validateABTestForm, validateEvaluationSettings } from '../../features/routing/abTesting/schema'
import { toABTestCreatePayload, withEvaluationSettings } from '../../features/routing/abTesting/payload'
import { currentSetupArm, inferExperimentType, srStrategyOf, toABTestFormValues } from '../../features/routing/abTesting/state'
import { ABTestFormValues, ABTestExperimentType, EditScope, ArmLayersForm, EMPTY_ARM, SrConfigOverrideForm, DEFAULT_VARIANT_SR_CONFIG, SR_STRATEGY_LABELS, SrStrategy } from '../../features/routing/abTesting/types'
import {
  ENDPOINT_LABELS,
  ENDPOINT_LAYERS,
  ENDPOINT_PATHS,
  EXPERIMENT_ENDPOINTS,
  projectArm,
  resolvedArm,
  sameArm,
  scopedEndpoints,
  splitEndpoints,
} from '../../features/routing/abTesting/arms'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'
import { useConnectorFees } from '../../hooks/useCostRouting'

import { PageHeading } from '../ui/PageHeading'
import { Notice } from '../ui/Notice'
import { FEATURE_FLAGS } from '../../lib/featureFlags'
const SAMPLE_SIZE_PRESETS = [1000, 5000, 10000, 50000]

// Detect the experiment type from the persisted arm shape (the backend stores no "type").
function abExperimentKind(abData?: ABTestAlgorithmData): ABTestExperimentType {
  return abData ? inferExperimentType(abData) : 'algorithm_comparison'
}

// Cost/net-value metrics are meaningful when either arm runs multi-objective (cost-aware) routing.
function hasCostArm(abData?: ABTestAlgorithmData): boolean {
  return !!abData && (resolvedArm(abData, 'control').sr?.enable_multi_objective === true
    || resolvedArm(abData, 'variant').sr?.enable_multi_objective === true)
}

// Hyperswitch SSO sessions route only through /routing/hybrid, so their experiments are scoped to it
// and only its results are shown.
function useHybridOnlySession(): boolean {
  return useAuthStore(state => state.user?.isRedirectSession === true)
}

function KindBadge({ kind }: { kind: ABTestExperimentType }) {
  if (kind === 'sr_config_tuning') return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-violet-100 text-violet-700 dark:bg-violet-900/30 dark:text-violet-300 leading-4">
      <Sliders size={9} /> SR Config Tuning
    </span>
  )
  return null
}

// SR layer label — resolves the two config dials (cost-awareness, autopilot) through the shared map,
// so the results/config views read identically to the create-form dropdown.
function srLabel(config: SrConfigOverride): string {
  return SR_STRATEGY_LABELS[srStrategyOf(config)]
}

// Decision Audit view that holds an endpoint's trail.
const AUDIT_ROUTING_KIND: Record<ExperimentEndpoint, string> = {
  hybrid_routing: 'hybrid',
  decide_gateway: 'multi_objective',
  evaluate: 'rule_based',
}

// Display label for an arm: its rule layer and SR layer, joined. `endpoint` names what an arm with
// no layer there does instead: `/routing/evaluate` returns the request's fallback connectors, and
// `/decide-gateway` runs SR with the merchant's settings.
function armLabel(arm: ExperimentArm, algorithmName: (id: string) => string, endpoint?: ExperimentEndpoint): string {
  const parts = [
    arm.rule_algorithm_id ? algorithmName(arm.rule_algorithm_id) : null,
    arm.sr ? `SR: ${srLabel(arm.sr)}` : null,
  ].filter(Boolean)
  if (parts.length > 0) return parts.join(' + ')
  if (endpoint === 'evaluate') return "No rule (request's fallback connectors)"
  if (endpoint === 'decide_gateway') return 'SR with merchant settings'
  return 'No layers'
}

// Renders the actual routing logic behind a static arm (rule-based / priority / volume split /
// single connector) so the merchant can see what the algorithm ID label stands for, instead of
// just its name. Returns nothing for SR-based arms (`sr_routing`) — those have no static config.
// An experiment records nothing before it was created, so its results are read from a day before
// `created_at` onward (the margin covers the timestamp carrying no zone). ClickHouse then skips
// the months before it instead of scanning the merchant's whole history.
const EXPERIMENT_START_MARGIN_MS = 24 * 60 * 60 * 1000

function experimentStartParam(algorithm: RoutingAlgorithm): string {
  if (!algorithm.created_at) return ''
  const raw = algorithm.created_at.replace(' ', 'T')
  const createdAtMs = Date.parse(/[zZ]|[+-]\d\d:?\d\d$/.test(raw) ? raw : `${raw}Z`)
  if (!Number.isFinite(createdAtMs)) return ''
  return `&start_ms=${Math.max(0, createdAtMs - EXPERIMENT_START_MARGIN_MS)}`
}

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
    ) : <p className="text-sm text-slate-500 italic">No connectors configured.</p>
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
    ) : <p className="text-sm text-slate-500 italic">No splits configured.</p>
  }
  if (type === 'single') {
    const conn = data as { gateway_name: string } | undefined
    return conn ? (
      <span className="rounded-full bg-slate-100 dark:bg-[#1a1f2a] px-2 py-0.5 text-xs font-medium text-slate-600 dark:text-[#8090a8]">
        {conn.gateway_name}
      </span>
    ) : <p className="text-sm text-slate-500 italic">No connector configured.</p>
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
      className="inline-flex cursor-help align-middle text-slate-500 hover:text-slate-600 focus:text-slate-600 focus:outline-none dark:text-slate-400 dark:hover:text-slate-300 dark:focus:text-slate-300 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-brand-500/60"
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
      {children}{required && <span className="text-slate-500">*</span>}
      {hint && <InfoHint text={hint} />}
    </label>
  )
}

/** One input/select treatment for the experiment form, so fields don't drift apart field by field. */
const fieldCls =
  'border border-slate-200 bg-transparent rounded-lg px-3 py-1.5 text-sm ' +
  'focus:outline-none focus:border-brand-500 dark:border-[#222226]'

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

// SR layer choices (each resolves to a distinct override).
const SR_STRATEGIES = Object.keys(SR_STRATEGY_LABELS) as (keyof typeof SR_STRATEGY_LABELS)[]

// Arm editor for Algorithm comparison: one picker per layer. The rule layer cascades — a config type
// (Rule-based / Volume split / …) shows a 2nd dropdown when it has more than one config (a
// single-config type is auto-selected). The SR layer is one of the SR strategies. Either layer may
// be left empty; the arm needs at least one.
function ArmLayersEditor({ label, help, accent, algorithms, value, costDataAvailable, srRoutingOn, liveSrConfig, onChange }: {
  label: string
  help: string
  // Variant arm (accent) vs control arm — drives the colored pill + panel tint so the two are
  // visually distinct and can't be misread for each other across the form.
  accent?: boolean
  algorithms: RoutingAlgorithm[]
  value: ArmLayersForm
  // Whether any connector has a fee the router can use. `null` while loading. The cost-savings
  // strategies are always selectable (the arm turns cost savings on for its own traffic, whatever
  // the merchant setting); without fees they cannot find a cheaper connector, so the form warns.
  costDataAvailable: boolean | null
  // The merchant's `sr-routing` setting. An arm with an SR layer runs SR for its own traffic either
  // way; the form says so when the setting is off.
  srRoutingOn: boolean
  // The merchant's base SR config (hedging / elimination / bucket size) plus how many segments
  // autopilot is actively tuning — shown when the arm has an SR layer. All SR strategies share the
  // same base config; they differ in whether they honor autopilot's per-segment overrides on top of
  // it. `autopilotFeatureOn` is the merchant's autopilot flag — segment count alone
  // can't distinguish "tuning right now" from "tuned before the feature was switched off".
  liveSrConfig: { hedging: number | null; elimination: number | null; bucketSize: number | null; autopilotSegmentCount: number; autopilotFeatureOn: boolean }
  onChange: (value: ArmLayersForm) => void
}) {
  const typeOf = (id: string): string => {
    const a = algorithms.find(x => x.id === id)
    return a ? ((a.algorithm_data || a.algorithm)?.type ?? '') : ''
  }
  const [ruleType, setRuleType] = useState<string>(() => typeOf(value.ruleAlgorithmId))
  // Keep the rule type in sync when the value is set externally (clone prefill) or once algorithms load.
  useEffect(() => {
    const t = typeOf(value.ruleAlgorithmId)
    if (t) setRuleType(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value.ruleAlgorithmId, algorithms])

  const realTypes = Array.from(
    new Set(algorithms.map(a => (a.algorithm_data || a.algorithm)?.type).filter(Boolean) as string[]),
  )
  const configs = ruleType ? algorithms.filter(a => (a.algorithm_data || a.algorithm)?.type === ruleType) : []

  function pickRuleType(t: string) {
    setRuleType(t)
    const c = algorithms.filter(a => (a.algorithm_data || a.algorithm)?.type === t)
    // One config → auto-select it; multiple (or none picked) → clear so the 2nd dropdown forces a choice.
    onChange({ ...value, ruleAlgorithmId: t && c.length === 1 ? c[0].id : '' })
  }

  const selectCls = `w-full ${fieldCls}`
  const layerLabel = 'mb-1 block text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4'

  return (
    <div className={`rounded-xl border p-3 space-y-3 ${accent
      ? 'border-brand-200 bg-brand-50/40 dark:border-brand-800/50 dark:bg-brand-900/10'
      : 'border-slate-200 bg-slate-50/50 dark:border-[#222226] dark:bg-[#0c0c10]'}`}>
      <div className="flex items-center gap-1.5">
        <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-semibold ${accent
          ? 'bg-brand-100 text-brand-700 dark:bg-brand-900/40 dark:text-brand-300'
          : 'bg-slate-200 text-slate-600 dark:bg-slate-700 dark:text-slate-200'} leading-4`}>
          {label}
        </span>
        <InfoHint text={help} />
      </div>

      <div>
        <label className={layerLabel}>Routing rule</label>
        <select className={selectCls} value={ruleType} onChange={e => pickRuleType(e.target.value)}>
          <option value="">No rule</option>
          {realTypes.map(t => (
            <option key={t} value={t}>{ALGO_TYPE_LABELS[t] ?? t}</option>
          ))}
        </select>
        {configs.length > 1 && (
          <select className={`${selectCls} mt-2`} value={value.ruleAlgorithmId} onChange={e => onChange({ ...value, ruleAlgorithmId: e.target.value })}>
            <option value="">Select {ALGO_TYPE_LABELS[ruleType]?.toLowerCase() ?? 'config'}</option>
            {configs.map(a => (
              <option key={a.id} value={a.id}>{a.name}</option>
            ))}
          </select>
        )}
        {configs.length === 1 && (
          <p className="mt-1.5 text-[13px] text-slate-500 dark:text-[#8d96aa] leading-[18px]">Using <span className="font-medium text-slate-600 dark:text-slate-300">{configs[0].name}</span></p>
        )}
        {value.ruleAlgorithmId && (
          <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
            <ArmRuleDetail algorithmId={value.ruleAlgorithmId} algorithms={algorithms} />
          </div>
        )}
      </div>

      <div>
        <label className={layerLabel}>SR strategy</label>
        <select className={selectCls} value={value.srStrategy} onChange={e => onChange({ ...value, srStrategy: e.target.value as SrStrategy | '' })}>
          <option value="">No SR</option>
          {SR_STRATEGIES.map(s => (
            <option key={s} value={s}>{SR_STRATEGY_LABELS[s]}</option>
          ))}
        </select>
        {value.srStrategy && !srRoutingOn && (
          <p className="mt-1.5 flex items-center gap-1 text-[12px] text-slate-500 dark:text-[#8d96aa] leading-4">
            <Info className="h-3.5 w-3.5 shrink-0" />
            Auth Rate routing is off for this merchant. This arm still runs SR for its own traffic.
          </p>
        )}
        {(value.srStrategy === 'sr_mo_manual' || value.srStrategy === 'sr_mo_autopilot') && costDataAvailable === false && (
          <p className="mt-1.5 flex items-center gap-1 text-[12px] text-amber-700 dark:text-amber-400 leading-4">
            <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
            No connector fees yet, so this arm routes like approvals-only until fees are added.
          </p>
        )}
        {value.srStrategy && (
          <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
            <p className="text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] mb-1.5 leading-4">Base SR config</p>
            <LiveSrConfigPanel
              hedging={liveSrConfig.hedging}
              elimination={liveSrConfig.elimination}
              bucketSize={liveSrConfig.bucketSize}
              autopilotSegmentCount={liveSrConfig.autopilotSegmentCount}
              autopilotFeatureOn={liveSrConfig.autopilotFeatureOn}
              // The two autopilot strategies honor autopilot-tuned segments; the manual ones run on
              // the merchant's static config (see srStrategyConfig in payload.ts).
              honorsAutopilot={value.srStrategy === 'sr_mo_autopilot' || value.srStrategy === 'sr_auth_autopilot'}
            />
          </div>
        )}
      </div>
    </div>
  )
}

// Read-only control arm: the merchant's current setup (the default) or the control of a cloned
// experiment. Either can be switched to the full editor with "Customize control".
function ControlArmSummary({ arm, source, runningExperimentName, algorithms, algorithmName, liveSrConfig, onCustomize, onUseCurrent }: {
  // `null` while the current setup is still loading.
  arm: ArmLayersForm | null
  source: 'current' | 'saved'
  // Set when an experiment is running, whose control arm is what live traffic gets today.
  runningExperimentName: string | null
  algorithms: RoutingAlgorithm[]
  algorithmName: (id: string) => string
  liveSrConfig: { hedging: number | null; elimination: number | null; bucketSize: number | null; autopilotSegmentCount: number; autopilotFeatureOn: boolean }
  onCustomize: () => void
  onUseCurrent?: () => void
}) {
  const layerLabel = 'mb-1 block text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4'
  const linkCls = 'text-[13px] font-medium text-brand-600 hover:text-brand-700 dark:text-brand-400 dark:hover:text-brand-300 leading-[18px]'
  const note = source === 'current'
    ? `${runningExperimentName ? `Taken from the control arm of the running experiment "${runningExperimentName}". ` : ''}Saved into the experiment when you create it, so later changes to routing rules or merchant settings don't change it.`
    : 'The control arm of the experiment you cloned.'

  return (
    <div className="rounded-xl border border-slate-200 bg-slate-50/50 dark:border-[#222226] dark:bg-[#0c0c10] p-3 space-y-3">
      <div className="flex items-center gap-1.5">
        <span className="inline-flex items-center rounded-full bg-slate-200 px-2 py-0.5 text-[11px] font-semibold text-slate-600 dark:bg-slate-700 dark:text-slate-200 leading-4">
          Control
        </span>
        <span className="text-[12px] text-slate-500 leading-4">{source === 'current' ? 'current setup' : 'from cloned experiment'}</span>
        <InfoHint text={note} />
      </div>

      {!arm ? (
        <p className="flex items-center gap-2 text-[13px] text-slate-500 leading-[18px]"><Spinner size={14} /> Reading your current setup…</p>
      ) : (
        <>
          <div>
            <span className={layerLabel}>Routing rule</span>
            {arm.ruleAlgorithmId ? (
              <>
                <p className="text-sm font-medium text-slate-700 dark:text-slate-300">{algorithmName(arm.ruleAlgorithmId)}</p>
                <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
                  <ArmRuleDetail algorithmId={arm.ruleAlgorithmId} algorithms={algorithms} />
                </div>
              </>
            ) : (
              <p className="text-[13px] text-slate-500 italic leading-[18px]">
                {source === 'current' ? 'No active routing rule' : 'No rule'}
              </p>
            )}
          </div>

          <div>
            <span className="mb-1 flex items-center gap-1 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">
              SR settings
              <InfoHint text={source === 'current'
                ? "Follows the merchant's Auth Rate routing, Autopilot and Cost savings settings. SR applies on /decide-gateway and on /routing/hybrid requests that include dynamic_routing_request."
                : 'SR applies on /decide-gateway and on /routing/hybrid requests that include dynamic_routing_request.'} />
            </span>
            {arm.srStrategy ? (
              <>
                <p className="text-sm font-medium text-slate-700 dark:text-slate-300">{SR_STRATEGY_LABELS[arm.srStrategy]}</p>
                <div className="mt-2 rounded-lg border border-slate-100 dark:border-[#1a1f2a] bg-slate-50/60 dark:bg-[#0a0a0f]/60 p-2">
                  <p className="text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] mb-1.5 leading-4">Base SR config</p>
                  <LiveSrConfigPanel
                    hedging={liveSrConfig.hedging}
                    elimination={liveSrConfig.elimination}
                    bucketSize={liveSrConfig.bucketSize}
                    autopilotSegmentCount={liveSrConfig.autopilotSegmentCount}
                    autopilotFeatureOn={liveSrConfig.autopilotFeatureOn}
                    honorsAutopilot={arm.srStrategy === 'sr_mo_autopilot' || arm.srStrategy === 'sr_auth_autopilot'}
                  />
                </div>
              </>
            ) : (
              <p className="text-[13px] text-slate-500 italic leading-[18px]">
                {source === 'current' ? 'Auth Rate routing is off for this merchant' : 'No SR'}
              </p>
            )}
          </div>
        </>
      )}

      <div className="flex flex-wrap gap-x-4 gap-y-1 border-t border-slate-100 dark:border-[#1e2330] pt-2">
        <button type="button" className={linkCls} onClick={onCustomize} disabled={!arm}>Customize control</button>
        {onUseCurrent && <button type="button" className={linkCls} onClick={onUseCurrent}>Use current setup</button>}
      </div>
    </div>
  )
}

// Read-only preview of what the experiment compares on each endpoint. Each endpoint applies only the
// arm layers it supports, and traffic is split only where the two arms differ. Hybrid-only sessions
// see just `/routing/hybrid`.
function EndpointPreview({ form, hybridOnly, algorithmName }: {
  form: ABTestFormValues
  hybridOnly: boolean
  algorithmName: (id: string) => string
}) {
  const data = toABTestCreatePayload(form, '').algorithm.data as ABTestAlgorithmData
  const control = resolvedArm(data, 'control')
  const variant = resolvedArm(data, 'variant')
  const endpoints = hybridOnly ? (['hybrid_routing'] as ExperimentEndpoint[]) : EXPERIMENT_ENDPOINTS
  if (!(control.rule_algorithm_id || control.sr) || !(variant.rule_algorithm_id || variant.sr)) return null

  return (
    <div>
      <FieldLabel hint="The experiment runs on every endpoint where the two arms differ. Each endpoint applies only the parts of an arm it supports, so the change measured can differ per endpoint. Results are reported per endpoint.">
        What's compared on each endpoint
      </FieldLabel>
      <div className="space-y-2">
        {endpoints.map(endpoint => {
          const c = projectArm(control, endpoint)
          const v = projectArm(variant, endpoint)
          const identical = sameArm(c, v)
          return (
            <div
              key={endpoint}
              className={`rounded-lg border px-3 py-2.5 ${identical
                ? 'border-slate-200 bg-slate-50/60 dark:border-[#222226] dark:bg-[#0c0c10]'
                : 'border-slate-300 dark:border-[#2a3140]'}`}
            >
              <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span className="text-sm font-medium text-slate-800 dark:text-slate-100">{ENDPOINT_LABELS[endpoint]}</span>
                <code className="text-[12px] text-slate-500">{ENDPOINT_PATHS[endpoint]}</code>
                <span className="text-[12px] text-slate-500">· {ENDPOINT_LAYERS[endpoint]}</span>
                {identical && (
                  <span className="rounded-full bg-slate-200 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:bg-slate-700 dark:text-slate-300 leading-4">Not split</span>
                )}
              </div>
              {identical ? (
                <p className="mt-1 text-[12px] text-slate-500 dark:text-[#8d96aa] leading-4">
                  Both arms route the same here ({armLabel(c, algorithmName, endpoint)}), so this endpoint's traffic isn't part of the experiment.
                </p>
              ) : (
                <p className="mt-1 text-[12px] text-slate-500 dark:text-[#8d96aa] leading-4 break-words">
                  <span className="font-medium">Control:</span> {armLabel(c, algorithmName, endpoint)}
                  <span className="mx-1.5">vs</span>
                  <span className="font-medium">Variant:</span> {armLabel(v, algorithmName, endpoint)}
                </p>
              )}
            </div>
          )
        })}
      </div>
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
// The autopilot calibration job writes cluster-specific hedging/bucket overrides tagged
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
      <div className="flex items-center justify-between text-[13px] leading-[18px]">
        <span className="text-slate-500">Hedging %</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {hedging !== null ? `${hedging}%` : <span className="text-slate-500 italic">Uses default</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[13px] leading-[18px]">
        <span className="text-slate-500">Elimination threshold</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {elimination !== null ? `Drops below ${(elimination * 100).toFixed(0)}% score` : <span className="text-slate-500 italic">Uses default</span>}
        </span>
      </div>
      <div className="flex items-center justify-between text-[13px] leading-[18px]">
        <span className="text-slate-500">Bucket size</span>
        <span className="font-medium text-slate-700 dark:text-slate-300">
          {bucketSize !== null ? `${bucketSize} requests` : <span className="text-slate-500 italic">Uses default</span>}
        </span>
      </div>
      {honorsAutopilot && autopilotFeatureOn && (
        <p className="flex items-center gap-1 text-[12px] text-amber-700 dark:text-amber-400 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330] leading-4">
          Auto-tunes bucket size and hedging % per segment, based on your traffic volume.
          <InfoHint text={
            autopilotSegmentCount > 0
              ? `Autopilot is tuning ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'} (by payment method / network / currency / country) — bucket size and hedging % can differ per segment and over time, so the values above are just the base config.`
              : `Autopilot will start tuning bucket size and hedging % per segment once enough traffic flows through it. Until then, every transaction uses the base config shown above.`
          } />
        </p>
      )}
      {honorsAutopilot && !autopilotFeatureOn && autopilotSegmentCount > 0 && (
        <p className="flex items-center gap-1 text-[12px] text-slate-500 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330] leading-4">
          Autopilot is off — {autopilotSegmentCount} segment{autopilotSegmentCount === 1 ? '' : 's'} from earlier tuning
          <InfoHint text={`Autopilot is currently disabled for this merchant, so nothing is being adjusted right now. This arm still applies the values it tuned earlier for ${autopilotSegmentCount} segment${autopilotSegmentCount === 1 ? '' : 's'}; every other segment uses the base config shown above.`} />
        </p>
      )}
      {honorsAutopilot && !autopilotFeatureOn && autopilotSegmentCount === 0 && (
        <p className="flex items-center gap-1 text-[12px] text-slate-500 pt-1.5 mt-0.5 border-t border-slate-100 dark:border-[#1e2330] leading-4">
          Autopilot is off — same as manual tuning
          <InfoHint text="Autopilot is disabled for this merchant and has not tuned any segment, so this arm uses the base config shown above. Turn on autopilot for it to tune bucket size and hedging % per segment." />
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
        <p className="text-[13px] text-slate-700 dark:text-slate-300 leading-[18px]">
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
      <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-700 dark:text-amber-400" />
      <p className="text-[13px] text-amber-800 dark:text-amber-300/90 leading-[18px]">
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
      : 'bg-slate-200 text-slate-600 dark:bg-slate-700 dark:text-slate-200'} leading-4`}>
      {label} · {pct}%
    </span>
  )
}

/** Config side-by-side: one row per layer, so what each arm applies reads as one matrix. */
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
  const control = resolvedArm(abData, 'control')
  const variant = resolvedArm(abData, 'variant')
  const muted = (t: string) => <span className="italic text-slate-500">{t}</span>
  const dash = <span className="text-slate-500 dark:text-slate-400">—</span>
  const emphasis = (v: ReactNode) => <span className="font-medium text-brand-600 dark:text-brand-400">{v}</span>
  const elimText = (e: number) => `Drops below ${(e * 100).toFixed(0)}% score`

  const rows: { label: string; control: ReactNode; variant: ReactNode }[] = []

  if (isTuning) {
    // Same SR algorithm, control on live config, variant on its overrides.
    const vHedge = variant.sr?.hedging_percent
    const vElim = variant.sr?.elimination_threshold
    rows.push({ label: 'Strategy', control: 'SR Routing (live config)', variant: 'SR Routing (custom params)' })
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
    const rule = (arm: ExperimentArm) => arm.rule_algorithm_id
      ? <div className="space-y-1.5"><p className="font-medium">{algorithmName(arm.rule_algorithm_id)}</p><ArmRuleDetail algorithmId={arm.rule_algorithm_id} algorithms={algorithms} /></div>
      : muted('No rule — uses the request’s fallback connectors')
    rows.push({ label: 'Routing rule', control: rule(control), variant: rule(variant) })
    rows.push({
      label: 'SR strategy',
      control: control.sr ? srLabel(control.sr) : muted('No SR'),
      variant: variant.sr ? srLabel(variant.sr) : muted('No SR'),
    })
    if (control.sr || variant.sr) {
      const auto = (arm: ExperimentArm) => !!arm.sr && arm.sr.use_autopilot !== false && autopilotOn
      const hedge = (arm: ExperimentArm) => !arm.sr ? dash : auto(arm) ? muted('Auto-tuned per segment') : live.hedging != null ? `${live.hedging}%` : muted('Uses default')
      const bucket = (arm: ExperimentArm) => !arm.sr ? dash : auto(arm) ? muted('Auto-tuned per segment') : live.bucketSize != null ? `${live.bucketSize} requests` : muted('Uses default')
      const elim = (arm: ExperimentArm) => !arm.sr ? dash : live.elimination != null ? elimText(live.elimination) : muted('Uses default')
      rows.push({ label: 'Hedging %', control: hedge(control), variant: hedge(variant) })
      rows.push({ label: 'Elimination threshold', control: elim(control), variant: elim(variant) })
      rows.push({ label: 'Bucket size', control: bucket(control), variant: bucket(variant) })
    }
  }

  return (
    <div className="overflow-x-auto rounded-xl border border-slate-200 dark:border-[#222226]">
      <table className="w-full min-w-[520px] text-sm">
        <thead>
          <tr className="border-b border-slate-200 bg-slate-50 text-left dark:border-[#222226] dark:bg-[#0c0c10]">
            <th className="w-[28%] px-4 py-2.5 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">Attribute</th>
            <th className="px-4 py-2.5"><ArmTh label="Control" pct={controlPct} /></th>
            <th className="px-4 py-2.5"><ArmTh label="Variant" pct={variantPct} accent /></th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={r.label} className={i > 0 ? 'border-t border-slate-100 dark:border-[#1a1a22]' : ''}>
              <td className="px-4 py-2.5 align-top text-[13px] font-medium text-slate-500 dark:text-[#8d96aa] leading-[18px]">{r.label}</td>
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
    ? 'text-slate-500 dark:text-slate-400'
    : d > 0 ? 'text-emerald-700 dark:text-emerald-400' : d < 0 ? 'text-red-600' : 'text-slate-500'
  const num = 'px-4 py-2.5 text-right text-slate-700 dark:text-slate-200'
  const metricCell = (label: string, sub: string) => (
    <td className="px-4 py-2.5"><span className="font-medium text-slate-700 dark:text-slate-200">{label}</span> <span className="text-[12px] text-slate-500 leading-4">{sub}</span></td>
  )

  return (
    <div className="overflow-x-auto rounded-xl border border-slate-200 dark:border-[#222226]">
      <table className="w-full min-w-[520px] text-sm [font-variant-numeric:tabular-nums]">
        <thead>
          <tr className="border-b border-slate-200 bg-slate-50 text-left dark:border-[#222226] dark:bg-[#0c0c10]">
            <th className="px-4 py-2.5 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">Metric</th>
            <th className="px-4 py-2.5 text-right"><ArmTh label="Control" pct={controlPct} /></th>
            <th className="px-4 py-2.5 text-right"><ArmTh label="Variant" pct={variantPct} accent /></th>
            <th className="px-4 py-2.5 text-right text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">Delta</th>
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
            <td className="px-4 py-2.5 text-right text-slate-500 dark:text-slate-400">—</td>
          </tr>
          <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
            {metricCell('Outcome', 'success / fail')}
            <td className="px-4 py-2.5 text-right"><span className="text-emerald-700 dark:text-emerald-400">{c.success_count.toLocaleString()}</span> <span className="text-slate-500 dark:text-slate-400">/</span> <span className="text-red-600">{c.failure_count.toLocaleString()}</span></td>
            <td className="px-4 py-2.5 text-right"><span className="text-emerald-700 dark:text-emerald-400">{v.success_count.toLocaleString()}</span> <span className="text-slate-500 dark:text-slate-400">/</span> <span className="text-red-600">{v.failure_count.toLocaleString()}</span></td>
            <td className="px-4 py-2.5 text-right text-slate-500 dark:text-slate-400">—</td>
          </tr>
          {costKind && (
            <tr className="border-t border-slate-100 dark:border-[#1a1a22]">
              {metricCell('Cost saved', 'TCS')}
              <td className="px-4 py-2.5 text-right text-sky-700 dark:text-sky-400">{c.total_cost_saved != null ? c.total_cost_saved.toLocaleString(undefined, { maximumFractionDigits: 2 }) : '—'}</td>
              <td className="px-4 py-2.5 text-right text-sky-700 dark:text-sky-400">{v.total_cost_saved != null ? v.total_cost_saved.toLocaleString(undefined, { maximumFractionDigits: 2 }) : '—'}</td>
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
  // Read-only sessions still see everything; the controls that would change it are inert.
  const canEditRouting = useCanEditRouting()
  const abData = (algorithm.algorithm_data || algorithm.algorithm)?.data as ABTestAlgorithmData | undefined
  const kind = abExperimentKind(abData)
  const isTuning = kind === 'sr_config_tuning'
  const costKind = hasCostArm(abData)
  const { liveHedging, liveElimination, liveBucketSize } = useLiveSrConfig(merchantId || undefined)
  const merchantFeatures = useMerchantFeatures(merchantId || undefined)
  const autopilotFeatureOn = merchantFeatures.isEnabled('autopilot')

  // An experiment applies different layers per endpoint, so its arms are only comparable within one
  // endpoint: results are read one endpoint at a time. Hybrid-only sessions only ever see hybrid.
  const hybridOnly = useHybridOnlySession()
  const endpoints = abData
    ? splitEndpoints(abData).filter(e => !hybridOnly || e === 'hybrid_routing')
    : []
  const [pickedEndpoint, setPickedEndpoint] = useState<ExperimentEndpoint | null>(null)
  const endpoint = pickedEndpoint && endpoints.includes(pickedEndpoint) ? pickedEndpoint : endpoints[0] ?? null
  const servedArm = (side: 'control' | 'variant'): ExperimentArm | null =>
    abData && endpoint ? projectArm(resolvedArm(abData, side), endpoint) : null

  // If the variant carries a margin override, value net EV at it; otherwise the backend default.
  const evalMargin = abData ? resolvedArm(abData, 'variant').sr?.margin : undefined
  const startParam = experimentStartParam(algorithm)
  const resultsUrl = abData && endpoint
    ? `/analytics/experiment/${algorithm.id}/results?endpoint=${endpoint}${startParam}&min_sample_size=${abData.min_sample_size}&guardrail_threshold_pp=${abData.guardrail_threshold_pp}${evalMargin !== undefined ? `&evaluation_margin=${evalMargin}` : ''}`
    : null

  const { data: results, isLoading } = useSWR<ExperimentResultsResponse>(
    merchantId && resultsUrl ? resultsUrl : null,
    fetcher,
    { refreshInterval: 60_000 },
  )

  const TXN_PAGE_SIZE = 20
  const [txnPage, setTxnPage] = useState(1)

  const txnsUrl = endpoint
    ? `/analytics/experiment/${algorithm.id}/transactions?endpoint=${endpoint}${startParam}&page_size=${TXN_PAGE_SIZE}&page=${txnPage}`
    : null
  const { data: txnData, isLoading: txnsLoading } = useSWR<ExperimentTransactionsResponse>(
    merchantId && txnsUrl ? txnsUrl : null,
    fetcher,
    { refreshInterval: 60_000 },
  )

  function routingType(variantArm: string): string {
    const arm = servedArm(variantArm === 'control' ? 'control' : 'variant')
    if (!arm) return '—'
    if (isTuning) {
      return variantArm === 'variant' ? 'SR Routing (custom params)' : 'SR Routing (live config)'
    }
    // What this arm applied on the selected endpoint: its rule and/or SR strategy.
    return armLabel(arm, algorithmName, endpoint ?? undefined)
  }

  // Every endpoint records a Decision Audit trail for the payments it routes.
  function txnHasAudit(variantArm: string): boolean {
    return !!servedArm(variantArm === 'control' ? 'control' : 'variant')
  }

  function openAuditForTxn(paymentId: string, variantArm: string) {
    if (!txnHasAudit(variantArm) || !endpoint) return
    const url = `/audit?range=1d&routing_kind=${AUDIT_ROUTING_KIND[endpoint]}&payment_id=${encodeURIComponent(paymentId)}`
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
        {
          label: 'Endpoints',
          value: hybridOnly ? ENDPOINT_LABELS.hybrid_routing : scopedEndpoints(abData).map(e => ENDPOINT_LABELS[e]).join(', '),
        },
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
                <Button size="sm" variant="secondary" onClick={onClone} disabled={!canEditRouting}><Copy size={13} /> Duplicate</Button>
                <Button size="sm" variant="danger" onClick={onStop} disabled={!canEditRouting}><PowerOff size={13} /> Stop</Button>
              </>
            ) : (
              <>
                {/* Edit / Delete are only offered while inactive — a running experiment must be
                    stopped first to avoid corrupting its collected results (enforced server-side too).
                    Duplicate, being read-only on this experiment, is offered in every state. */}
                <Button size="sm" variant="secondary" onClick={onClone} disabled={!canEditRouting}><Copy size={13} /> Duplicate</Button>
                <Button size="sm" variant="secondary" onClick={onEdit} disabled={!canEditRouting}><Pencil size={13} /> Edit</Button>
                {FEATURE_FLAGS.RULE_DELETION && (
                  <Button size="sm" variant="secondary" onClick={onDelete} disabled={!canEditRouting}><Trash2 size={13} /> Delete</Button>
                )}
                <Button size="sm" variant="primary" onClick={onActivate} disabled={!canEditRouting}>Activate</Button>
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
                <p className="text-[12px] text-slate-500 dark:text-[#8d96aa] leading-4">{s.label}</p>
                <p className="mt-0.5 text-[15px] font-semibold text-slate-800 dark:text-slate-100 [font-variant-numeric:tabular-nums] leading-[22px]">{s.value}</p>
              </div>
            ))}
          </div>
        )}

        {/* Progress toward the sample target */}
        {results && (
          <div className="mt-5 space-y-2 border-t border-slate-100 pt-4 dark:border-[#1a1a22]">
            <div className="flex items-end justify-between">
              <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-slate-500 dark:text-[#8d96aa] leading-4">Transactions collected</p>
              <p className="text-[15px] font-semibold text-slate-800 dark:text-slate-100 [font-variant-numeric:tabular-nums] leading-[22px]">
                {totalTxns.toLocaleString()} <span className="text-slate-500">/ {minSample.toLocaleString()}</span>
              </p>
            </div>
            <div className="h-2.5 overflow-hidden rounded-full bg-slate-100 dark:bg-[#232833]">
              <div
                className={`h-full rounded-full transition-all duration-500 ${significant ? 'bg-emerald-500' : 'bg-brand-500'}`}
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="text-[12px] text-slate-500 max-w-[57ch] leading-4">
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
            <p className="mt-0.5 text-[12px] text-slate-500 leading-4">Updates every 60 seconds</p>
          </div>
          {results && <VerdictChip verdict={results.verdict} />}
        </div>
        {endpoints.length > 1 && (
          <div className="inline-flex rounded-lg border border-slate-200 bg-slate-100 p-0.5 dark:border-[#222226] dark:bg-[#14181f]">
            {endpoints.map(e => (
              <button
                key={e}
                type="button"
                onClick={() => { setPickedEndpoint(e); setTxnPage(1) }}
                className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${e === endpoint
                  ? 'bg-white text-slate-900 shadow-sm dark:bg-[#2a3140] dark:text-white'
                  : 'text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-white'}`}
              >
                {ENDPOINT_LABELS[e]}
              </button>
            ))}
          </div>
        )}
        {endpoint && servedArm('control') && servedArm('variant') && (
          <p className="text-[13px] text-slate-500 dark:text-[#8d96aa] leading-[18px] break-words">
            On <code className="text-[12px]">{ENDPOINT_PATHS[endpoint]}</code> this compares{' '}
            <span className="font-medium text-slate-700 dark:text-slate-200">{routingType('control')}</span> with{' '}
            <span className="font-medium text-slate-700 dark:text-slate-200">{routingType('variant')}</span>.
          </p>
        )}
        {!endpoint ? (
          <p className="text-sm italic text-slate-500">
            {hybridOnly
              ? 'Both arms are identical on hybrid routing, so this experiment does not split its traffic.'
              : 'Both arms are identical on every selected endpoint, so this experiment does not split traffic.'}
          </p>
        ) : isLoading && !results ? (
          <div className="flex items-center gap-2 text-sm text-slate-500"><Spinner size={14} /> Loading stats…</div>
        ) : !results ? (
          <p className="text-sm italic text-slate-500">
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
              <Notice tone="danger">
                <ShieldAlert size={12} />
                Variant auth rate dropped {Math.abs(results.delta_pp).toFixed(2)}pp below control — beyond the {abData?.guardrail_threshold_pp}pp guardrail. Consider stopping the experiment.
              </Notice>
            )}
          </div>
        )}
      </section>

      {/* ── Transactions ── */}
      <section className="space-y-3 border-t border-slate-200 dark:border-[#262d3a] pt-6">
        <div className="flex items-center justify-between">
          <div>
            <h3 className={type.heading}>Transactions</h3>
            <p className="mt-0.5 text-[12px] text-slate-500 leading-4">
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
                    <td colSpan={6} className="px-4 py-8 text-base text-slate-500 text-center">
                      {txnsLoading ? 'Loading…' : 'No transactions logged yet for this experiment.'}
                    </td>
                  </tr>
                ) : txnData.transactions.map((txn, idx) => {
                  const txnIsSr = txnHasAudit(txn.variant_arm)
                  return (
                    <tr
                      key={`${txn.payment_id}-${idx}`}
                      onClick={() => openAuditForTxn(txn.payment_id, txn.variant_arm)}
                      title={txnIsSr ? 'Open in Decision Audit' : 'Audit trail not available for rule-only arm payments'}
                      className={`border-b border-slate-50 dark:border-[#131318] transition-colors ${txnIsSr ? 'cursor-pointer hover:bg-slate-50 dark:hover:bg-[#0f0f16]' : 'cursor-default opacity-60'}`}
                    >
                      <td className="px-4 py-2.5">
                        <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold ${txn.variant_arm === 'control'
                            ? 'bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300'
                            : 'bg-brand-100 text-brand-700 dark:bg-brand-900/30 dark:text-brand-300'
                          } leading-4`}>
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
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400 leading-4">success</span>
                        ) : txn.status === 'failure' ? (
                          <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-red-100 text-red-600 dark:bg-red-900/30 dark:text-red-400 leading-4">failure</span>
                        ) : (
                          <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400 leading-4" title="Payment was routed but no outcome was recorded — counted against auth rate">
                            <Clock size={9} /> no outcome
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-sm text-slate-500 whitespace-nowrap">
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
                        ? <span key={`ellipsis-${idx}`} className="px-1 text-sm text-slate-500">…</span>
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
      <span className="inline-flex items-center rounded-full bg-brand-100 px-2 py-0.5 text-[11px] font-semibold text-brand-700 dark:bg-brand-900/40 dark:text-brand-300 leading-4">
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
  // Set while editing a saved experiment; `null` when creating one.
  editScope: EditScope | null
  // The edit check couldn't read recorded payments, so the setup is locked as a precaution.
  editCheckFailed: boolean
  onCreate: () => void
  onActivateCreated: (id: string) => void
  algorithmName: (id: string) => string
  // The merchant's current setup as a control arm; `null` while loading.
  currentControl: ArmLayersForm | null
  // Name of the running experiment, whose control arm is the current setup.
  runningExperimentName: string | null
  // Present only when there's a list to return to (i.e. experiments already exist).
  onCancel?: () => void
}

function CreateForm({
  form, setForm, eligibleAlgorithms, saving, error, success, createdId,
  merchantId, editScope, editCheckFailed, onCreate, onActivateCreated, algorithmName, currentControl, runningExperimentName, onCancel,
}: CreateFormProps) {
  const hybridOnly = useHybridOnlySession()
  const isEditing = editScope !== null
  // Read-only sessions still see everything; the controls that would change it are inert.
  const canEditRouting = useCanEditRouting()
  // Every SR strategy is offered whatever the merchant settings: an arm's strategy sets cost savings
  // and autopilot for that arm's traffic only. The flags drive the hints shown under the choice.
  const features = useMerchantFeatures(merchantId || undefined)
  const autopilotOn = features.isEnabled('autopilot')
  const { fees: connectorFees, isLoading: connectorFeesLoading } = useConnectorFees(merchantId || undefined)
  const costDataAvailable = connectorFeesLoading ? null : connectorFees.some(f => f.effective_pct_bps !== null || f.effective_fixed !== null)

  // Shared across both experiment types: SR config tuning needs it for the control panel below,
  // and any SR-based arm in Algorithm comparison (auth / MO manual / MO autopilot) shows it too.
  const { liveHedging, liveElimination, liveBucketSize, autopilotSegmentCount } = useLiveSrConfig(merchantId || undefined)

  // "Custom" is active when the sample target isn't one of the presets — either the user chose it,
  // or an edited experiment carries an off-preset value.
  const [customSample, setCustomSample] = useState(!SAMPLE_SIZE_PRESETS.includes(form.minSampleSize))

  const sampleTargetField = (
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
            className="w-24 rounded-md bg-transparent px-2.5 py-1.5 text-xs tabular-nums focus:outline-none focus:border-brand-500"
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
  )

  // One inline sentence with the value set into it, rather than a stacked label + input + suffix.
  // The pp-vs-% nuance lives in the tooltip so the line stays a single row.
  const guardrailField = (
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
  )

  return (
    <Card>
      <CardHeader>
        <h2 className={type.heading}>{isEditing ? 'Edit experiment' : 'New experiment'}</h2>
      </CardHeader>
      <CardBody className="space-y-6">

        {editScope === 'checking' && (
          <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8d96aa]">
            <Spinner size={14} /> Checking whether this experiment has recorded payments…
          </div>
        )}

        {/* An experiment with recorded payments keeps its routing setup, since its results are read
            against it. Only the fields that judge those results can change. */}
        {editScope === 'evaluation' && (
          <>
            <FormSection>
              <div>
                <FieldLabel required>Experiment name</FieldLabel>
                <input
                  className={`w-full ${fieldCls}`}
                  value={form.name}
                  onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                />
                <p className="mt-1.5 text-[13px] text-slate-500 dark:text-[#8d96aa] leading-[18px]">
                  {editCheckFailed
                    ? 'Couldn’t check whether this experiment has recorded payments, so its routing setup is locked for now. '
                    : 'This experiment has recorded payments, so its arms, traffic split and endpoints are locked. '}
                  To test a different setup, duplicate it.
                </p>
              </div>
            </FormSection>
            <FormSection divide>{sampleTargetField}</FormSection>
            <FormSection divide>{guardrailField}</FormSection>
          </>
        )}

        {editScope === 'full' && (
          <p className="text-[13px] text-slate-500 dark:text-[#8d96aa] leading-[18px]">
            No payments recorded yet, so every setting can change.
          </p>
        )}

        {(editScope === null || editScope === 'full') && (
          <>
            {/* ── 1 · What you're comparing ── */}
            <FormSection>
              {/* New experiments compare two routing setups. A saved SR config tuning experiment keeps
                  its type when edited or duplicated, so its tuning fields still show below. */}
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
                  {form.controlSource === 'custom' ? (
                    <div className="space-y-1.5">
                      <ArmLayersEditor
                        label="Control"
                        help="The baseline the variant is compared against."
                        algorithms={eligibleAlgorithms}
                        value={form.control}
                        costDataAvailable={costDataAvailable}
                        srRoutingOn={features.isEnabled('sr-routing')}
                        liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
                        onChange={control => setForm(f => ({ ...f, control }))}
                      />
                      <button
                        type="button"
                        className="text-[13px] font-medium text-brand-600 hover:text-brand-700 dark:text-brand-400 dark:hover:text-brand-300 leading-[18px]"
                        onClick={() => setForm(f => ({ ...f, controlSource: 'current' }))}
                      >
                        Use current setup
                      </button>
                    </div>
                  ) : (
                    <ControlArmSummary
                      arm={form.controlSource === 'current' ? currentControl : form.control}
                      source={form.controlSource}
                      runningExperimentName={runningExperimentName}
                      algorithms={eligibleAlgorithms}
                      algorithmName={algorithmName}
                      liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
                      onCustomize={() => {
                        const control = form.controlSource === 'current' ? currentControl : form.control
                        if (control) setForm(f => ({ ...f, controlSource: 'custom', control }))
                      }}
                      onUseCurrent={form.controlSource === 'saved' ? () => setForm(f => ({ ...f, controlSource: 'current' })) : undefined}
                    />
                  )}
                  <ArmLayersEditor
                    label="Variant"
                    help="The setup you want to test."
                    accent
                    algorithms={eligibleAlgorithms}
                    value={form.variant}
                    costDataAvailable={costDataAvailable}
                    srRoutingOn={features.isEnabled('sr-routing')}
                    liveSrConfig={{ hedging: liveHedging, elimination: liveElimination, bucketSize: liveBucketSize, autopilotSegmentCount, autopilotFeatureOn: autopilotOn }}
                    onChange={variant => setForm(f => ({ ...f, variant }))}
                  />
                </div>
              )}

              {form.experimentType === 'sr_config_tuning' && (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {/* Control — live config, non-editable */}
                  <div className="rounded-xl border border-slate-200 dark:border-[#222226] bg-slate-50/50 dark:bg-[#0c0c10] px-4 py-4 space-y-3">
                    <div className="flex items-center gap-2">
                      <span className="inline-flex items-center rounded-full bg-slate-200 px-2 py-0.5 text-[11px] font-semibold text-slate-600 dark:bg-slate-700 dark:text-slate-200 leading-4">
                        Control ({100 - form.variantSplitPct}%)
                      </span>
                      <span className="text-[12px] text-slate-500 leading-4">current config</span>
                    </div>
                    <div className="space-y-2.5">
                      <div>
                        <p className="text-[13px] text-slate-500 mb-0.5 leading-[18px]">Hedging %</p>
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                          {liveHedging !== null ? `${liveHedging}%` : <span className="text-slate-500 italic text-xs">Uses default</span>}
                        </p>
                      </div>
                      <div>
                        <p className="text-[13px] text-slate-500 mb-0.5 leading-[18px]">Elimination threshold</p>
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                          {liveElimination !== null ? `Drops below ${(liveElimination * 100).toFixed(0)}% score` : <span className="text-slate-500 italic text-xs">Uses default</span>}
                        </p>
                      </div>
                    </div>
                    <p className="text-[12px] text-slate-500 pt-1 border-t border-slate-100 dark:border-[#1e2330] leading-4">
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

              {FEATURE_FLAGS.AB_TEST_ENDPOINT_PREVIEW && form.experimentType === 'algorithm_comparison' && (
                <EndpointPreview form={form} hybridOnly={hybridOnly} algorithmName={algorithmName} />
              )}
            </FormSection>

            {/* ── 2 · Traffic & duration ── */}
            <FormSection divide>
              <div>
                <div className="mb-2 flex items-center justify-between">
                  <span className={type.label}>Traffic allocation</span>
                  <span className="text-[13px] tabular-nums text-slate-500 dark:text-slate-400 leading-[18px]">
                    {100 - form.variantSplitPct}% control
                    <span className="mx-1.5 text-slate-500 dark:text-slate-400">/</span>
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

              {sampleTargetField}
            </FormSection>

            {/* ── 3 · Safety ── */}
            <FormSection divide>
              {guardrailField}
            </FormSection>

          </>
        )}

        <ErrorMessage error={error} />

        {success && (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-emerald-200 bg-emerald-50 px-5 py-4 text-base text-emerald-800 dark:border-emerald-500/25 dark:bg-emerald-500/10 dark:text-emerald-200">
            <span>{success}</span>
            {createdId && (
              <Button size="sm" variant="primary" onClick={() => onActivateCreated(createdId)} disabled={!canEditRouting}>
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
          <Button variant="primary" onClick={onCreate} disabled={saving || editScope === 'checking' || !merchantId || !canEditRouting}>
            {saving ? <><Spinner size={14} /> {isEditing ? 'Saving…' : 'Creating…'}</> : isEditing ? 'Save changes' : 'Create experiment'}
          </Button>
        </div>
      </CardBody>
    </Card>
  )
}

// ─── Experiment list ───────────────────────────────────────────────────────────

type ExperimentStatusFilter = 'all' | 'active' | 'inactive'

interface ExperimentsTableProps {
  merchantId: string | null
  // `null` while the list is loading.
  experiments: RoutingAlgorithm[] | null
  activeId: string | null
  realPaymentsOn: boolean
  algorithmName: (id: string) => string
  onOpen: (id: string) => void
  onActivate: (id: string) => void
  onStop: (id: string) => void
  onEdit: (algo: RoutingAlgorithm) => void
  onClone: (algo: RoutingAlgorithm) => void
  onDelete: (id: string) => void
}

// Same layout as the Rule-Based Routing list: most recently changed first, filters in the headers,
// actions in a row menu. Opening a row shows that experiment's results.
function ExperimentsTable({
  merchantId, experiments, activeId, realPaymentsOn, algorithmName,
  onOpen, onActivate, onStop, onEdit, onClone, onDelete,
}: ExperimentsTableProps) {
  const canEditRouting = useCanEditRouting()
  const [statusFilter, setStatusFilter] = useState<ExperimentStatusFilter>('all')
  const [nameFilter, setNameFilter] = useState('')

  const sorted = [...(experiments ?? [])].sort((a, b) => lastModifiedMs(b) - lastModifiedMs(a))
  const visible = sorted.filter(algo => {
    const isActive = algo.id === activeId
    if (statusFilter === 'active' && !isActive) return false
    if (statusFilter === 'inactive' && isActive) return false
    const needle = nameFilter.trim().toLowerCase()
    return !needle || algo.name.toLowerCase().includes(needle) || algo.id.toLowerCase().includes(needle)
  })

  return (
    <Card className="!rounded-[18px]">
      {!merchantId ? (
        <p className="px-4 py-6 text-sm text-slate-500">Set merchant ID to load experiments.</p>
      ) : !experiments ? (
        <p className="px-4 py-6 text-sm text-slate-500">Loading...</p>
      ) : experiments.length === 0 ? (
        <p className="px-4 py-6 text-sm text-slate-500">No experiments yet.</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[880px] text-left">
            <thead>
              <tr className="border-b border-slate-100 text-[11px] font-semibold uppercase tracking-wide text-slate-500 dark:border-[#1e2330] dark:text-[#78849a] leading-4">
                <th className="px-5 py-3.5">
                  <HeaderSearch label="Experiment Name & ID" value={nameFilter} onChange={setNameFilter} ariaLabel="Filter experiments by name" />
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
                    onChange={(v) => setStatusFilter(v as ExperimentStatusFilter)}
                    ariaLabel="Filter by status"
                  />
                </th>
                <th className="px-5 py-3.5">Control → Variant</th>
                <th className="px-5 py-3.5">Traffic Split</th>
                <th className="px-5 py-3.5">Last Modified</th>
                <th className="px-5 py-3.5 text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {visible.length === 0 && (
                <tr>
                  <td colSpan={6} className="px-4 py-8 text-center">
                    <p className="text-sm text-slate-500">No experiments match these filters.</p>
                    <button
                      type="button"
                      onClick={() => { setStatusFilter('all'); setNameFilter('') }}
                      className="mt-1.5 text-sm font-medium text-brand-600 hover:text-brand-700 dark:text-brand-400"
                    >
                      Clear filters
                    </button>
                  </td>
                </tr>
              )}
              {visible.map(algo => {
                const abData = (algo.algorithm_data || algo.algorithm)?.data as ABTestAlgorithmData | undefined
                const isActive = algo.id === activeId
                const kind = abExperimentKind(abData)
                const arms = !abData
                  ? '—'
                  : kind === 'sr_config_tuning'
                    ? 'Live SR config → tuned SR config'
                    : `${armLabel(resolvedArm(abData, 'control'), algorithmName)} → ${armLabel(resolvedArm(abData, 'variant'), algorithmName)}`
                const stamp = formatLastModified(algo)
                // /routing/update and /routing/delete reject a running experiment.
                const lockedReason = isActive
                  ? 'Stop this experiment first'
                  : !canEditRouting
                    ? 'You do not have permission to change routing'
                    : undefined
                return (
                  <tr
                    key={algo.id}
                    data-testid="experiment-row"
                    data-experiment-name={algo.name}
                    onClick={() => onOpen(algo.id)}
                    className={`cursor-pointer border-b border-slate-100 align-middle transition-colors hover:bg-slate-50 dark:border-[#1e2330] dark:hover:bg-[#11151d] ${
                      isActive ? 'bg-emerald-50/50 dark:bg-emerald-900/10' : ''
                    }`}
                  >
                    <td className="align-top px-5 py-4">
                      <div className="flex items-start gap-2">
                        <ChevronRight size={14} className="mt-1 shrink-0 text-slate-500" />
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2">
                            <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">{algo.name}</p>
                            <KindBadge kind={kind} />
                          </div>
                          <p className="mt-0.5 truncate font-mono text-xs text-slate-500 dark:text-[#78849a]">{algo.id}</p>
                        </div>
                      </div>
                    </td>
                    <td className="align-top px-5 py-4">
                      <span className={`inline-flex shrink-0 items-center rounded-full px-2.5 py-1 text-[11px] font-semibold leading-4 ${
                        !isActive
                          ? 'bg-slate-100 text-slate-500 dark:bg-[#1a1f2a] dark:text-[#8090a8]'
                          : realPaymentsOn
                            ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-400'
                            : 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400'
                      }`}>
                        {!isActive ? 'Inactive' : realPaymentsOn ? 'Active' : 'Not collecting'}
                      </span>
                    </td>
                    <td className="max-w-[420px] px-5 py-4 align-top">
                      <p className="break-words text-sm font-medium leading-5 text-slate-800 dark:text-slate-200" title={arms}>{arms}</p>
                    </td>
                    <td className="align-top whitespace-nowrap px-5 py-4 text-sm tabular-nums text-slate-700 dark:text-slate-300">
                      {abData ? `${100 - abData.variant_split_pct}% / ${abData.variant_split_pct}%` : '—'}
                    </td>
                    <td className="align-top whitespace-nowrap px-5 py-4 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                      {stamp ? (
                        <span title={stamp.full}>
                          {stamp.date}
                          <span className="block text-[12px] text-slate-500 dark:text-[#78849a] leading-4">{stamp.time}</span>
                        </span>
                      ) : '—'}
                    </td>
                    <td className="align-top px-5 py-4" onClick={(e) => e.stopPropagation()}>
                      <RowMenu
                        items={[
                          { label: 'View results', icon: BarChart3, onSelect: () => onOpen(algo.id) },
                          isActive
                            ? { label: 'Stop', icon: PowerOff, tone: 'danger', onSelect: () => onStop(algo.id), disabled: !canEditRouting }
                            : { label: 'Activate', icon: Zap, tone: 'positive', onSelect: () => onActivate(algo.id), disabled: !canEditRouting },
                          { label: 'Edit', icon: Pencil, onSelect: () => onEdit(algo), disabled: Boolean(lockedReason), hint: lockedReason },
                          { label: 'Duplicate', icon: Copy, onSelect: () => onClone(algo), disabled: !canEditRouting },
                          ...(FEATURE_FLAGS.RULE_DELETION
                            ? [{ label: 'Delete', icon: Trash2, tone: 'danger' as const, onSelect: () => onDelete(algo.id), disabled: Boolean(lockedReason), hint: lockedReason }]
                            : []),
                        ]}
                      />
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  )
}

// ─── Page ──────────────────────────────────────────────────────────────────────

const DEFAULT_FORM: ABTestFormValues = {
  name: '',
  experimentType: 'algorithm_comparison',
  controlSource: 'current',
  control: { ...EMPTY_ARM },
  variant: { ...EMPTY_ARM },
  endpoints: [...EXPERIMENT_ENDPOINTS],
  variantSplitPct: 10,
  minSampleSize: 5000,
  guardrailThresholdPp: 3,
  variantSrConfig: { ...DEFAULT_VARIANT_SR_CONFIG },
}

export function ABTestingPage() {
  // Read-only sessions still see everything; the controls that would change it are inert.
  const canEditRouting = useCanEditRouting()
  const { merchantId } = useMerchantStore()
  const { mutate: mutateCache } = useSWRConfig()
  const hybridOnly = useHybridOnlySession()
  // Hyperswitch SSO merchants route only through /routing/hybrid, so their experiments start (and
  // stay) scoped to it; the backend enforces the same scope on save.
  const newForm = (): ABTestFormValues => ({
    ...DEFAULT_FORM,
    endpoints: hybridOnly ? ['hybrid_routing'] : [...EXPERIMENT_ENDPOINTS],
  })

  const { data: allAlgorithms, mutate: mutateAll } = useSWR<RoutingAlgorithm[]>(
    merchantId ? ['routing-list', merchantId] : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/${merchantId}`),
  )
  const { data: activeAlgorithms, error: activeAlgorithmsError, mutate: mutateActive } = useSWR<RoutingAlgorithm[]>(
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

  const [form, setForm] = useState<ABTestFormValues>(newForm)
  // How live traffic routes today, used as the control arm until the user customizes it. Read at
  // create time, so the saved experiment keeps it even if routing or merchant settings change later.
  const currentControl: ArmLayersForm | null =
    (activeAlgorithms || activeAlgorithmsError) && (features.data || features.error)
      ? currentSetupArm(activeAlgorithms ?? [], {
        srRoutingOn: features.isEnabled('sr-routing'),
        costSavingsOn: features.isEnabled('cost-savings'),
        autopilotOn: features.isEnabled('autopilot'),
      })
      : null
  // A new experiment's variant starts as a copy of the current setup, so the user changes only what
  // they want to test instead of rebuilding the layers they keep.
  // Seeded once per fresh form; `resetForm` re-arms it.
  const variantSeeded = useRef(false)
  useEffect(() => {
    if (variantSeeded.current || !currentControl || form.controlSource !== 'current') return
    variantSeeded.current = true
    if (!form.variant.ruleAlgorithmId && !form.variant.srStrategy) {
      setForm(f => ({ ...f, variant: { ...currentControl } }))
    }
  })
  function resetForm() {
    variantSeeded.current = false
    setForm(newForm())
  }
  const formForCreate: ABTestFormValues =
    form.controlSource === 'current' && currentControl ? { ...form, control: currentControl } : form
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
  const [editScope, setEditScope] = useState<EditScope | null>(null)
  const [editCheckFailed, setEditCheckFailed] = useState(false)
  // The experiment whose edit check is in flight; a later edit or close makes an earlier answer stale.
  const editCheckFor = useRef<string | null>(null)

  // Opening results and going back to the list are history entries, so the browser's back button
  // walks the same path.
  function selectExperiment(id: string) {
    setSearchParams({ experiment: id })
    setShowCreate(false)
    setError(null)
    setSuccess(null)
  }

  function goToList() {
    setSearchParams({})
    setError(null)
    setSuccess(null)
  }

  function openCreate() {
    setSearchParams({}, { replace: true })
    setShowCreate(true)
    stopEditing()
    resetForm()
    setSuccess(null)
    setError(null)
  }

  // Every setting can change until the experiment records a payment; after that its results are
  // read against its setup, so only the name, sample target and guardrail can. The backend
  // enforces the same rule on save.
  async function openEdit(algo: RoutingAlgorithm) {
    const values = toABTestFormValues(algo)
    if (!values) return
    setForm(values)
    setEditingId(algo.id)
    setEditScope('checking')
    setEditCheckFailed(false)
    setShowCreate(true)
    setSuccess(null)
    setError(null)
    editCheckFor.current = algo.id
    try {
      const results = await fetcher<ExperimentResultsResponse>(
        `/analytics/experiment/${algo.id}/results?${experimentStartParam(algo).slice(1)}`,
      )
      if (editCheckFor.current !== algo.id) return
      const recorded = results.control.transaction_count + results.variant.transaction_count
      setEditScope(recorded > 0 ? 'evaluation' : 'full')
    } catch {
      if (editCheckFor.current !== algo.id) return
      setEditCheckFailed(true)
      setEditScope('evaluation')
    }
  }

  function stopEditing() {
    editCheckFor.current = null
    setEditingId(null)
    setEditScope(null)
    setEditCheckFailed(false)
  }

  // Clone = pre-fill the create form from an existing experiment's full config (arms, split,
  // sample, guardrail, SR overrides), with a distinct name. Unlike edit, this runs the normal
  // create path (validate + build payload), so it yields a brand-new experiment with its own
  // fresh data window — the original is left completely untouched.
  function openClone(algo: RoutingAlgorithm) {
    const values = toABTestFormValues(algo)
    if (!values) return
    setSearchParams({}, { replace: true })
    setForm({ ...values, name: `${values.name} (copy)`, endpoints: newForm().endpoints })
    stopEditing()
    setShowCreate(true)
    setSuccess(null)
    setError(null)
  }

  // Leave the form and return to the list/detail. Only reachable when experiments already exist —
  // with an empty list the form is the whole page, so there's nothing to cancel back to.
  function closeCreate() {
    setShowCreate(false)
    stopEditing()
    resetForm()
    setError(null)
    setSuccess(null)
    setSearchParams({}, { replace: true })
  }

  async function handleCreate() {
    if (!merchantId) return

    // Edit keeps the experiment's id. With recorded payments only the evaluation settings change and
    // the stored routing setup is sent back as is; without, the form is rebuilt like a create.
    if (editingId) {
      if (editScope === 'checking') return
      const original = savedAbTests.find(a => a.id === editingId)
      const originalAlgorithm = original && (original.algorithm_data || original.algorithm)
      if (!original || !originalAlgorithm) { setError('Could not load the experiment to edit'); return }
      let update: { name: string; description: string; algorithm: unknown }
      if (editScope === 'full') {
        const validationError = validateABTestForm(formForCreate)
        if (validationError) { setError(validationError); return }
        const payload = toABTestCreatePayload(formForCreate, merchantId)
        update = { name: payload.name, description: payload.description, algorithm: payload.algorithm }
      } else {
        const validationError = validateEvaluationSettings(form)
        if (validationError) { setError(validationError); return }
        update = {
          name: form.name.trim(),
          description: original.description ?? '',
          algorithm: withEvaluationSettings(originalAlgorithm, form),
        }
      }
      setSaving(true); setError(null); setSuccess(null)
      try {
        await apiPost('/routing/update', {
          created_by: merchantId,
          routing_algorithm_id: editingId,
          ...update,
        })
        await mutateAll()
        setSuccess(`"${form.name.trim()}" updated.`)
        setSearchParams({ experiment: editingId }, { replace: true })
        stopEditing()
        resetForm()
        setShowCreate(false)
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : 'Failed to update experiment')
      } finally {
        setSaving(false)
      }
      return
    }

    if (form.experimentType === 'algorithm_comparison' && form.controlSource === 'current' && !currentControl) {
      setError('Still reading your current setup. Try again in a moment.')
      return
    }
    const validationError = validateABTestForm(formForCreate)
    if (validationError) { setError(validationError); return }
    setSaving(true); setError(null); setSuccess(null)
    try {
      const payload = toABTestCreatePayload(formForCreate, merchantId)
      const result = await apiPost<RoutingAlgorithm>('/routing/create', payload)
      const id = result.rule_id || result.id
      setCreatedId(id)
      setSuccess(`"${formForCreate.name}" created.`)
      resetForm()
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

  // Three views, like Rule-Based Routing: the experiment list, one experiment's results
  // (`?experiment=<id>`), and the create/edit form. The results and form views lead back to the list.
  const view: 'list' | 'detail' | 'form' = showCreate ? 'form' : selectedAlgo ? 'detail' : 'list'
  const backToList = (
    <button
      type="button"
      onClick={view === 'form' ? closeCreate : goToList}
      className="mb-2 inline-flex items-center gap-2 text-sm font-medium text-slate-500 transition-colors hover:text-brand-600 dark:text-[#8d96a8] dark:hover:text-brand-400"
    >
      <ArrowLeft size={16} /> A/B Testing
    </button>
  )

  return (
    <div className="flex flex-col gap-6">
      {view === 'list' ? (
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <PageHeading title="A/B Testing" description="Compare routing strategies on live traffic with statistical significance." />
          </div>
          <Button onClick={openCreate} disabled={!canEditRouting || !merchantId}>
            <Plus size={15} /> New experiment
          </Button>
        </div>
      ) : (
        // The form is centered, so its back link lines up with the form's left edge.
        <div className={view === 'form' ? 'mx-auto w-full max-w-4xl' : undefined}>{backToList}</div>
      )}

      {/* Drift guard: an experiment is active but the live-traffic flag got turned off afterward
          (e.g. toggled on the SR Feature Flags tab), so it's silently collecting nothing. Now that
          the detail header no longer offers Pause/Resume, this banner is the single recovery path —
          shown on every view. Gated on features.data so it never flashes before the flag state has
          loaded. */}
      {activeAbTest && features.data && !realPaymentsOn && (
        <div className="flex flex-wrap items-start gap-3 rounded-xl border border-amber-300 bg-amber-50 px-4 py-3 dark:border-amber-500/30 dark:bg-amber-500/10">
          <AlertTriangle size={18} className="mt-0.5 shrink-0 text-amber-700 dark:text-amber-400" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-amber-900 dark:text-amber-200">
              “{activeAbTest.name}” is active but not collecting results
            </p>
            <p className="mt-0.5 text-[13px] text-amber-800/90 dark:text-amber-300/90 max-w-[57ch] leading-[18px]">
              “A/B test on real payments” is off, so live traffic isn’t being split between the arms. Turn it back on to resume recording stats.
            </p>
          </div>
          <Button size="sm" variant="primary" onClick={() => toggleRealPayments(true)} disabled={enablingFlag}>
            {enablingFlag ? <><Spinner size={13} /> Enabling…</> : 'Enable live-traffic testing'}
          </Button>
        </div>
      )}

      {/* The form shows its own errors; the list and results views show action errors here. */}
      {view !== 'form' && <ErrorMessage error={error} />}
      {view !== 'form' && success && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-sm text-emerald-600 dark:text-emerald-400">
          {success}
        </div>
      )}

      {view === 'list' && (
        <ExperimentsTable
          merchantId={merchantId}
          experiments={allAlgorithms ? savedAbTests : null}
          activeId={activeAbTest?.id ?? null}
          realPaymentsOn={realPaymentsOn}
          algorithmName={algorithmName}
          onOpen={selectExperiment}
          onActivate={handleActivate}
          onStop={setPendingDeactivateId}
          onEdit={openEdit}
          onClone={openClone}
          onDelete={setPendingDeleteId}
        />
      )}

      {view === 'detail' && selectedAlgo && merchantId && (
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

      {view === 'form' && (
        <div className="mx-auto w-full max-w-4xl">
          <CreateForm
            form={formForCreate}
            setForm={setForm}
            eligibleAlgorithms={eligibleAlgorithms}
            saving={saving}
            error={error}
            success={success}
            createdId={createdId}
            merchantId={merchantId}
            editScope={editScope}
            editCheckFailed={editCheckFailed}
            onCreate={handleCreate}
            onActivateCreated={(id) => handleActivate(id)}
            algorithmName={algorithmName}
            currentControl={currentControl}
            runningExperimentName={activeAbTest?.name ?? null}
            onCancel={closeCreate}
          />
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
            ? 'This also turns on “A/B test on real payments”, so live traffic is split between the arms and results start collecting. Your active routing rule stays active, and routes all payments again when you stop the experiment.'
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
        description="Payments go back to your active routing rule and SR settings, which stay as they are. Results remain available."
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
