import { useDeferredValue, useEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { RuleEvaluationPanel } from './RuleEvaluationPanel'
import { ErrorInfoFields, ErrorInfoState, GsmOptionRow, DEFAULT_ERROR_INFO } from './ErrorInfoFields'
import { PenaltyClassificationGuide } from './PenaltyClassificationGuide'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { BarChart, Bar, LineChart, Line, ComposedChart, Area, CartesianGrid, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { ConfirmDialog } from '../ui/ConfirmDialog'
import { useMerchantStore } from '../../store/merchantStore'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import { CHART_TOOLTIP_ITEM_STYLE, CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import { DecideGatewayResponse, GatewayConnector, MultiObjectiveInfo, PaymentAuditEvent, PaymentAuditResponse, RoutingEvent, RoutingEventType, UpdateScoreResponse } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'
import { useConnectorFees } from '../../hooks/useCostRouting'
import { describeRoutingEvent, useRoutingEvents } from '../../hooks/useRoutingEvents'
import { FEATURE_FLAGS } from '../../lib/featureFlags'
import { Play, Pause, RefreshCw, ChevronDown, ChevronUp, Code, Plus, Trash2, PieChart as PieChartIcon, X, Network, Settings, ArrowRightLeft, Target, TrendingDown, Flag, SlidersHorizontal } from 'lucide-react'

// UI-local algorithm tokens for the simulation dropdown. Both map to
// { rankingAlgorithm: 'SR_BASED_ROUTING' } on the backend /decide-gateway request;
// the dropdown no longer forces multi-objective. Whether cost-savings (multi-objective)
// runs is decided entirely by the merchant's Autopilot "Optimize for economic value"
// flag — we no longer send enableMultiObjective. 'SR_MULTI_OBJECTIVE' is kept because it
// still constrains the form to a card-only cluster shape.
type SimulationAlgorithm = 'SR_BASED_ROUTING' | 'SR_MULTI_OBJECTIVE'

type TabType = 'single' | 'batch' | 'rule' | 'volume' | 'debit'

interface FormState {
  amount: string
  currency: string
  payment_method_type: string
  payment_method: string
  card_brand: string
  card_program: string
  auth_type: string
  eligible_gateways: string
  ranking_algorithm: SimulationAlgorithm
}

const MULTI_OBJECTIVE_CURRENCY = 'USD'
// Currencies selectable for the multi-objective sim. USD exercises the seed-cost fallback; EUR/AUD
// (and others) let a transaction match the in-house fitted models, which are keyed by currency.
const MULTI_OBJECTIVE_CURRENCIES = ['USD', 'EUR', 'GBP', 'AUD', 'CAD'] as const
const MULTI_OBJECTIVE_PAYMENT_METHODS = ['CREDIT'] as const
const CARD_PROGRAM_OPTIONS = ['STANDARD', 'PREMIUM'] as const
const MULTI_OBJECTIVE_CARD_BRANDS = ['VISA', 'MASTERCARD'] as const

// Each variant is one card scenario. Cost is keyed by issuer region (cardIssuerCountry →
// us/eu/intl), funding (CREDIT/DEBIT), network, and program. The US variants span the full
// Stripe-vs-Adyen seed-cost spread. The EU / INTL variants (Visa/Mastercard, standard debit &
// credit) mirror the shape of the in-house fitted models — so paired with EUR (or AUD) they let a
// simulated transaction actually land on an in-house cost model (`costSource: IN_HOUSE`) instead of
// falling back to the seed table.
const MULTI_OBJECTIVE_CLUSTER_VARIANTS: Array<{
  label: string
  paymentMethod: 'CREDIT' | 'DEBIT'
  cardSwitchProvider: 'VISA' | 'MASTERCARD' | 'AMEX'
  cardProgram: 'STANDARD' | 'PREMIUM' | 'COMMERCIAL'
  // Raw issuer country a BIN lookup would supply (ISO-2, e.g. 'US', 'FR', 'AU'). The engine buckets
  // it to a pricing region for the coarse fallback and uses the raw value for the fine predictor.
  cardIssuerCountry: string
}> = [
  { label: 'US debit', paymentMethod: 'DEBIT',  cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD',   cardIssuerCountry: 'US'   },
  { label: 'US standard credit', paymentMethod: 'CREDIT', cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD',   cardIssuerCountry: 'US'   },
  { label: 'US premium credit', paymentMethod: 'CREDIT', cardSwitchProvider: 'VISA',       cardProgram: 'PREMIUM',    cardIssuerCountry: 'US'   },
  { label: 'US corporate',       paymentMethod: 'CREDIT', cardSwitchProvider: 'MASTERCARD', cardProgram: 'COMMERCIAL', cardIssuerCountry: 'US'   },
  { label: 'Amex US consumer',   paymentMethod: 'CREDIT', cardSwitchProvider: 'AMEX',       cardProgram: 'PREMIUM',    cardIssuerCountry: 'US'   },
  // EU / AU scenarios carry a *raw* issuer country (as a BIN lookup would supply), so the in-house
  // category predictor resolves the specific fitted cluster. Pair the EU cards with EUR and the AU
  // card with AUD to land on in-house models.
  { label: 'FR debit (Visa)',        paymentMethod: 'DEBIT',  cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD', cardIssuerCountry: 'FR' },
  { label: 'IT debit (Mastercard)',  paymentMethod: 'DEBIT',  cardSwitchProvider: 'MASTERCARD', cardProgram: 'STANDARD', cardIssuerCountry: 'IT' },
  { label: 'FR credit (Visa)',       paymentMethod: 'CREDIT', cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD', cardIssuerCountry: 'FR' },
  { label: 'IT credit (Mastercard)', paymentMethod: 'CREDIT', cardSwitchProvider: 'MASTERCARD', cardProgram: 'STANDARD', cardIssuerCountry: 'IT' },
  { label: 'AU debit (Visa)',        paymentMethod: 'DEBIT',  cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD', cardIssuerCountry: 'AU' },
]

// Scenario the simulator opens on. 'US debit' is the headline Adyen-wins-on-cost case, so it's the
// default (falls back to the first variant if the label is ever renamed).
const DEFAULT_MULTI_OBJ_SCENARIO: number | 'ALL' =
  Math.max(0, MULTI_OBJECTIVE_CLUSTER_VARIANTS.findIndex(v => v.label === 'US debit'))

interface DebitRoutingFormState {
  amount: string
  currency: string
  auth_type: string
  eligible_gateways: string
  merchant_category_code: string
  acquirer_country: string
  co_badged_networks: string
  issuer_country: string
  is_regulated: boolean
  regulated_name: string
  card_type: 'debit' | 'credit'
}

interface SimulationConfig {
  totalPayments: string
  // Number of transactions fired concurrently per batch. 1 = the original strictly
  // sequential loop; higher values send more feedback per unit time so gateway scores
  // move faster. Read once at run start (see `runSimulation`).
  tps: number
  // Inclusive amount range each multi-objective transaction's amount is drawn from
  // (uniform). Drives how much the fixed-fee term shows up in cost.
  minAmount: number
  maxAmount: number
}

interface GatewaySimConfig {
  successRate: number
  failureMode: 'decline' | 'timeout'
  gsmDecision: 'retry' | 'do_default'
  penalized: boolean
  errorInfo: ErrorInfoState
}

const DEFAULT_GW_SIM_CONFIG: GatewaySimConfig = {
  successRate: 70,
  failureMode: 'decline',
  gsmDecision: 'retry',
  penalized: true,
  errorInfo: { ...DEFAULT_ERROR_INFO },
}

// Per-gateway default for the SR slider. Any gateway not listed falls back to
// DEFAULT_GW_SIM_CONFIG.successRate. Keyed lowercase; look-ups normalize case.
const DEFAULT_GW_SUCCESS_RATE: Record<string, number> = {
  stripe: 94,
  adyen: 93,
  braintree: 92,
  chase: 92,
}

// Default sim config for a gateway, applying its per-gateway default SR.
function defaultGwSimConfig(gw: string): GatewaySimConfig {
  return {
    ...DEFAULT_GW_SIM_CONFIG,
    successRate: DEFAULT_GW_SUCCESS_RATE[gw.trim().toLowerCase()] ?? DEFAULT_GW_SIM_CONFIG.successRate,
    errorInfo: { ...DEFAULT_ERROR_INFO },
  }
}

interface SimulationResult {
  paymentId: string
  decidedGateway: string
  status: 'CHARGED' | 'FAILURE' | 'PENDING_VBV'
  timestamp: string
  routingApproach: string | null
  gatewayPriorityMap: Record<string, number> | null
  retryGateway?: string
  retryStatus?: 'CHARGED' | 'FAILURE' | 'PENDING_VBV'
  costSavedBps?: number | null
  costWon?: boolean
  authWon?: boolean
  // Captured on cost-override decisions so the run can value the auth-rate the
  // override risked: (headAuthRate − chosenAuthRate) × amount × margin.
  headAuthRate?: number | null
  chosenAuthRate?: number | null
  margin?: number | null
  // EV gap between the top-two EV-ranked PSPs (fraction of ticket) — the decision's
  // margin of victory. Null when fewer than two PSPs had cost data to rank on EV.
  evGapTop2?: number | null
  amount: number
  currency: string
  // Card attributes that seeded the cost lookup (the cluster the decision priced).
  cardNetwork?: string
  cardProgram?: string
  cardIssuerRegion?: string
  cardScenario?: string
}

// Soft, sentence-case stat label (vs the all-caps SurfaceLabel) for the cost/auth summary.
// Acronyms render as a small muted suffix so the title itself stays gentle.
function StatLabel({ label, abbr }: { label: string; abbr?: string }) {
  return (
    <p className="text-[14px] font-semibold leading-snug text-slate-500 dark:text-slate-400">
      {label}
      {abbr && (
        <span className="ml-1 text-[10px] font-normal uppercase tracking-wide text-slate-400 dark:text-slate-500">
          {abbr}
        </span>
      )}
    </p>
  )
}

function formatCurrencyValue(value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      maximumFractionDigits: 2,
    }).format(value)
  } catch {
    return `${value.toFixed(2)} ${currency}`
  }
}

function formatSavingsCurrency(bps: number, amount: number, currency: string): string {
  return formatCurrencyValue((bps / 10000) * amount, currency)
}

type TransactionOutcome = 'CHARGED' | 'FAILURE' | 'PENDING_VBV'

type AuditInspectorTab = 'summary' | 'input' | 'response' | 'raw'

interface SetupPromptState {
  title: string
  body: string
  detail?: string
  configurePath?: string
}

interface RuleEvaluateParams {
  key: string
  type: 'enum_variant' | 'str_value' | 'number' | 'metadata_variant'
  value: string
  metadataKey?: string
}

interface RuleEvaluateResponse {
  payment_id: string | null
  status: string
  output: {
    type: 'single' | 'priority' | 'volume_split'
    connector?: GatewayConnector
    connectors?: GatewayConnector[]
    splits?: { connector: GatewayConnector; split: number }[]
  }
  evaluated_output?: GatewayConnector[]
  eligible_connectors?: GatewayConnector[]
}

function approachColor(approach: string): string {
  for (const [k, v] of Object.entries(ROUTING_APPROACH_COLORS)) {
    if (approach.includes(k) || k.includes(approach)) return v
  }
  return 'bg-white/5 text-slate-600 ring-1 ring-inset ring-white/8'
}

const COLORS = ['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16']
const GW_PALETTE = ['#3b82f6', '#8b5cf6', '#f97316', '#ec4899', '#14b8a6']
// Per-connector color overrides by name (case-insensitive), applied before the positional
// palette so a connector keeps its color regardless of its order in the eligible list.
// Stripe → purple, Adyen → green.
const GW_COLOR_OVERRIDES: Record<string, string> = { stripe: '#8b5cf6', adyen: '#0abf53' }

// Routing events bucket at second granularity and the analytics pipeline lags a
// little, so allow a small margin before the run-start when scoping events to a run.
const EVENTS_RUN_START_MARGIN_MS = 3000

// Icon/colour per routing-event type for the simulator's Events panel. Events come
// from the /analytics/routing-events feed: leader flips and auth-band entries/exits
// as gateway success-rate scores shift during a run.
const SIM_EVENT_META: Record<RoutingEventType, { icon: React.ElementType; iconClass: string }> = {
  leader_changed: { icon: ArrowRightLeft, iconClass: 'text-sky-500' },
  gateway_entered_auth_band: { icon: Target, iconClass: 'text-emerald-500' },
  gateway_exited_auth_band: { icon: TrendingDown, iconClass: 'text-amber-500' },
  calibration_applied: { icon: SlidersHorizontal, iconClass: 'text-violet-500' },
}

function formatSimEventTime(ms: number) {
  return new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

type VolumePaymentEntry = {
  paymentId: string
  connector: string
}

const EXPLORER_STORAGE_KEY_PREFIX = 'decision-explorer-state-v2'
const EXPLORER_RESULT_TTL_MS = 10 * 60 * 1000

const DEFAULT_FORM: FormState = {
  amount: '1000',
  currency: '',
  payment_method_type: '',
  payment_method: '',
  card_brand: '',
  card_program: 'STANDARD',
  auth_type: 'THREE_DS',
  eligible_gateways: 'stripe, adyen',
  ranking_algorithm: 'SR_MULTI_OBJECTIVE',
}

const DEFAULT_DEBIT_FORM: DebitRoutingFormState = {
  amount: '1000',
  currency: 'USD',
  auth_type: 'THREE_DS',
  eligible_gateways: 'stripe, adyen',
  merchant_category_code: 'merchant_category_code_0001',
  acquirer_country: 'US',
  co_badged_networks: 'VISA, NYCE, PULSE, STAR',
  issuer_country: 'US',
  is_regulated: false,
  regulated_name: '',
  card_type: 'debit',
}

// The baseline pair of processors the cost comparison always shows, even before anything is
// ingested, so there's always at least two connectors to compare. Ingested and user-added
// connectors are unioned on top of this (see the eligible-gateways seeding effect).
const DEFAULT_ELIGIBLE_GATEWAYS = DEFAULT_FORM.eligible_gateways
  .split(',')
  .map(s => s.trim().toLowerCase())
  .filter(Boolean)

// Merge connector lists into one deduped, order-preserving set (normalized lowercase/trimmed).
// Used to build the eligible-gateway set from default + ingested + manually-added connectors.
function unionConnectors(...lists: string[][]): string[] {
  const seen = new Set<string>()
  const out: string[] = []
  for (const list of lists) {
    for (const raw of list) {
      const c = raw.trim().toLowerCase()
      if (c && !seen.has(c)) {
        seen.add(c)
        out.push(c)
      }
    }
  }
  return out
}

// Auth-rate simulation runs a fixed batch — no user input; it starts on Run
// and stops at this many transactions.
const SIMULATION_TOTAL_PAYMENTS = '5000'

// Default / ceiling for the parallel-requests (TPS) lever. 1 preserves the original
// sequential cadence; the ceiling caps how many decide+feedback round-trips we fan out
// at once so a slider drag can't flood the backend.
const DEFAULT_SIMULATION_TPS = 1
const MAX_SIMULATION_TPS = 50

// Bounds + defaults for the per-transaction amount range slider (multi-objective sim).
const SIMULATION_AMOUNT_BOUND_MIN = 1
const SIMULATION_AMOUNT_BOUND_MAX = 100000
const DEFAULT_SIMULATION_MIN_AMOUNT = 10
const DEFAULT_SIMULATION_MAX_AMOUNT = 100
// Hidden for now — the amount-range control isn't needed yet. The sim still runs
// with the default range above; flip to true to bring the slider back.
const SHOW_AMOUNT_RANGE_SLIDER = false

// Share of simulated failures modeled as soft declines (GSM `retry`) that are retried on an
// alternate processor when smart retry is enabled. The rest are hard declines (no retry).
const SIM_RETRYABLE_FAILURE_SHARE = 0.5

const DEFAULT_SIMULATION_CONFIG: SimulationConfig = {
  totalPayments: SIMULATION_TOTAL_PAYMENTS,
  tps: DEFAULT_SIMULATION_TPS,
  minAmount: DEFAULT_SIMULATION_MIN_AMOUNT,
  maxAmount: DEFAULT_SIMULATION_MAX_AMOUNT,
}


const DEFAULT_RULE_PARAMS: RuleEvaluateParams[] = [
  { key: 'payment_method_type', type: 'enum_variant', value: '', metadataKey: '' },
  { key: 'currency', type: 'enum_variant', value: '', metadataKey: '' },
]

const DEFAULT_FALLBACK_CONNECTORS: GatewayConnector[] = [
  { gateway_name: 'stripe', gateway_id: 'gateway_001' },
  { gateway_name: 'adyen', gateway_id: 'gateway_002' },
]

interface ExplorerPersistedState {
  scopeKey: string | null
  resultDataUpdatedAtMs: number | null
  activeTab: TabType
  form: FormState
  simulationConfig: SimulationConfig
  gatewaySimConfigs: Record<string, GatewaySimConfig>
  errorInfo: ErrorInfoState
  debitForm: DebitRoutingFormState
  ruleParams: RuleEvaluateParams[]
  fallbackConnectors: GatewayConnector[]
  volumePayments: string
  result: DecideGatewayResponse | null
  debitResult: DecideGatewayResponse | null
  debitPaymentId: string | null
  singleRunPaymentId: string | null
  singleRunOutcome: TransactionOutcome
  ruleResult: RuleEvaluateResponse | null
  volumeDistribution: { name: string; count: number; percentage: number }[]
  volumeEvaluationLog: VolumePaymentEntry[]
  volumeProgress: number
  simulationResults: SimulationResult[]
  responseOpen: boolean
  debitResponseOpen: boolean
  volumeResponseOpen: boolean
  smartRetryEnabled: boolean
  multiObjScenario: number | 'ALL'
  moCurrency: string
  resumableRun: { total: number; nextIndex: number } | null
}

function cloneRuleParams(params: RuleEvaluateParams[]) {
  return params.map((param) => ({ ...param }))
}

function cloneConnectors(connectors: GatewayConnector[]) {
  return connectors.map((connector) => ({ ...connector }))
}

function normalizeDebitCardCategory(value: unknown): DebitRoutingFormState['card_type'] {
  return `${value || ''}`.toLowerCase() === 'credit' ? 'credit' : 'debit'
}

function getDefaultExplorerState(): ExplorerPersistedState {
  return {
    scopeKey: null,
    resultDataUpdatedAtMs: null,
    activeTab: 'batch',
    form: { ...DEFAULT_FORM },
    simulationConfig: { ...DEFAULT_SIMULATION_CONFIG },
    gatewaySimConfigs: {},
    errorInfo: { ...DEFAULT_ERROR_INFO },
    debitForm: { ...DEFAULT_DEBIT_FORM },
    ruleParams: cloneRuleParams(DEFAULT_RULE_PARAMS),
    fallbackConnectors: cloneConnectors(DEFAULT_FALLBACK_CONNECTORS),
    volumePayments: '100',
    result: null,
    debitResult: null,
    debitPaymentId: null,
    singleRunPaymentId: null,
    singleRunOutcome: 'CHARGED',
    ruleResult: null,
    volumeDistribution: [],
    volumeEvaluationLog: [],
    volumeProgress: 0,
    simulationResults: [],
    responseOpen: false,
    debitResponseOpen: false,
    volumeResponseOpen: false,
    smartRetryEnabled: false,
    multiObjScenario: DEFAULT_MULTI_OBJ_SCENARIO,
    moCurrency: MULTI_OBJECTIVE_CURRENCY,
    resumableRun: null,
  }
}

function explorerScopeKey(userId: string, userEmail: string, merchantId: string) {
  return `${userId || userEmail || 'anonymous'}:${merchantId || 'no-merchant'}`
}

function explorerStorageKey(scopeKey: string) {
  return `${EXPLORER_STORAGE_KEY_PREFIX}:${encodeURIComponent(scopeKey)}`
}

function hasExpiredExplorerResults(resultDataUpdatedAtMs?: number | null) {
  return Boolean(
    resultDataUpdatedAtMs &&
    Date.now() - resultDataUpdatedAtMs > EXPLORER_RESULT_TTL_MS,
  )
}

function removeExplorerState(scopeKey: string) {
  if (typeof window === 'undefined') return
  window.localStorage.removeItem(explorerStorageKey(scopeKey))
}

function saveExplorerState(scopeKey: string, state: ExplorerPersistedState) {
  if (typeof window === 'undefined') return
  window.localStorage.setItem(explorerStorageKey(scopeKey), JSON.stringify(state))
}

function loadExplorerState(scopeKey: string): ExplorerPersistedState {
  if (typeof window === 'undefined') return getDefaultExplorerState()

  try {
    const raw = window.localStorage.getItem(explorerStorageKey(scopeKey))
      || window.localStorage.getItem(EXPLORER_STORAGE_KEY_PREFIX)
    if (!raw) return { ...getDefaultExplorerState(), scopeKey }
    const parsed = JSON.parse(raw) as Partial<ExplorerPersistedState>
    const defaults = getDefaultExplorerState()
    if (parsed.scopeKey !== scopeKey || hasExpiredExplorerResults(parsed.resultDataUpdatedAtMs)) {
      removeExplorerState(scopeKey)
      return { ...defaults, scopeKey }
    }

    return {
      ...defaults,
      ...parsed,
      scopeKey,
      resultDataUpdatedAtMs: parsed.resultDataUpdatedAtMs || null,
      activeTab: defaults.activeTab,
      form: {
        ...defaults.form,
        ...(parsed.form || {}),
        // Only Success Rate Based + Multi-Objective is supported now; the
        // dynamic simulator drives the transaction variant, not the form.
        ranking_algorithm: 'SR_MULTI_OBJECTIVE',
      },
      simulationConfig: { ...defaults.simulationConfig, ...(parsed.simulationConfig || {}), totalPayments: SIMULATION_TOTAL_PAYMENTS },
      gatewaySimConfigs: parsed.gatewaySimConfigs || defaults.gatewaySimConfigs,
      errorInfo: { ...defaults.errorInfo, ...(parsed.errorInfo || {}) },
      debitForm: {
        ...defaults.debitForm,
        ...(parsed.debitForm || {}),
        card_type: normalizeDebitCardCategory(parsed.debitForm?.card_type),
      },
      ruleParams: parsed.ruleParams?.length ? cloneRuleParams(parsed.ruleParams) : defaults.ruleParams,
      fallbackConnectors: parsed.fallbackConnectors?.length ? cloneConnectors(parsed.fallbackConnectors) : defaults.fallbackConnectors,
      volumeDistribution: parsed.volumeDistribution || defaults.volumeDistribution,
      volumeEvaluationLog: parsed.volumeEvaluationLog || defaults.volumeEvaluationLog,
      simulationResults: parsed.simulationResults || defaults.simulationResults,
    }
  } catch {
    return { ...getDefaultExplorerState(), scopeKey }
  }
}

function toUpperOptions(values: string[] = []): string[] {
  return values.map(v => v.trim()).filter(Boolean).map(v => v.toUpperCase())
}

function uniqueUpperOptions(values: string[] = []): string[] {
  return Array.from(new Set(toUpperOptions(values)))
}

function extractVolumeConnector(response: RuleEvaluateResponse) {
  return (
    response.evaluated_output?.[0]?.gateway_name ||
    response.output.connector?.gateway_name ||
    response.output.connectors?.[0]?.gateway_name ||
    null
  )
}

function mapRoutingTypeToRuleParamType(
  keyType?: 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'
): RuleEvaluateParams['type'] {
  if (keyType === 'enum') return 'enum_variant'
  if (keyType === 'integer') return 'number'
  if (keyType === 'udf' || keyType === 'global_ref') return 'metadata_variant'
  return 'str_value'
}

function queryString(params: Record<string, string | number | undefined>) {
  const search = new URLSearchParams()
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== '') {
      search.set(key, String(value))
    }
  })
  return search.toString()
}

function formatDateTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(ms))
}

function humanizeAuditValue(value?: string | null) {
  if (!value) return ''
  const normalized = value
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase()

  return normalized.replace(/\b\w/g, (char) => char.toUpperCase())
}

function routeLabel(route?: string | null) {
  if (!route) return 'Unknown route'
  if (route === 'decision_gateway' || route === 'decide_gateway') return 'Decide Gateway'
  if (route === 'update_gateway_score') return 'Update Gateway'
  if (route === 'routing_evaluate') return 'Rule Evaluate'
  return humanizeAuditValue(route)
}

function eventTypeLabel(eventType?: string | null) {
  if (!eventType) return 'Unknown event'
  if (eventType === 'decide_gateway_decision') return 'Decide Gateway'
  if (
    eventType === 'update_gateway_score_update' ||
    eventType === 'update_gateway_score_score_snapshot' ||
    eventType === 'update_score_legacy_score_snapshot'
  ) return 'Update Gateway'
  if (eventType === 'decide_gateway_rule_hit') return 'Rule Evaluate'
  if (eventType.startsWith('routing_evaluate_') && eventType !== 'routing_evaluate_request_hit') return 'Decision Result'
  if (eventType.endsWith('_error')) return 'Errors'
  return humanizeAuditValue(eventType)
}

function flowTypeValue(event: PaymentAuditEvent) {
  return event.flow_type || ''
}

function stageLabel(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if (event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_stage === 'score_updated') return 'Update Gateway'
  if (event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_stage === 'preview_evaluated' || (flowType.startsWith('routing_evaluate_') && flowType !== 'routing_evaluate_request_hit')) return 'Decision Result'
  if (flowType.endsWith('_error')) return 'Errors'
  return humanizeAuditValue(event.event_stage || flowType)
}

function eventPhase(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if ((flowType.startsWith('decide_gateway_') && flowType !== 'decide_gateway_rule_hit') || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (flowType === 'decide_gateway_rule_hit' || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (flowType.startsWith('update_gateway_score_') || flowType.startsWith('update_score_legacy_') || event.event_stage === 'score_updated') return 'Update Gateway'
  if ((flowType.startsWith('routing_evaluate_') && flowType !== 'routing_evaluate_request_hit') || event.event_stage === 'preview_evaluated') return 'Decision'
  return 'Errors'
}

function badgeVariantForEvent(event: PaymentAuditEvent): 'blue' | 'green' | 'purple' | 'red' | 'orange' | 'gray' {
  const flowType = flowTypeValue(event)
  const normalizedStatus = (event.status || '').toUpperCase()
  if (
    flowType.endsWith('_error') ||
    normalizedStatus === 'FAILURE' ||
    normalizedStatus.includes('FAILED') ||
    normalizedStatus.includes('DECLINED')
  ) return 'red'
  if (flowType === 'decide_gateway_rule_hit') return 'purple'
  if (
    normalizedStatus === 'CHARGED' ||
    normalizedStatus === 'AUTHORIZED' ||
    normalizedStatus === 'SUCCESS'
  ) return 'green'
  if (flowType.startsWith('routing_evaluate_') && flowType !== 'routing_evaluate_request_hit') return 'purple'
  if (flowType.startsWith('update_gateway_score_') || flowType.startsWith('update_score_legacy_')) return 'green'
  if (flowType.startsWith('decide_gateway_')) return 'blue'
  return 'orange'
}

function summaryBadgeVariant(status?: string | null): 'blue' | 'green' | 'purple' | 'red' | 'orange' | 'gray' {
  const normalizedStatus = (status || '').toUpperCase()
  if (
    normalizedStatus === 'FAILURE' ||
    normalizedStatus.includes('FAILED') ||
    normalizedStatus.includes('DECLINED')
  ) return 'red'
  if (
    normalizedStatus === 'SUCCESS' ||
    normalizedStatus === 'CHARGED' ||
    normalizedStatus === 'AUTHORIZED'
  ) return 'green'
  return 'gray'
}

function phaseBadgeVariant(phase: string): 'blue' | 'green' | 'purple' | 'red' | 'orange' | 'gray' {
  if (phase === 'Decide Gateway') return 'blue'
  if (phase === 'Rule Evaluate') return 'purple'
  if (phase === 'Decision') return 'purple'
  if (phase === 'Update Gateway') return 'green'
  if (phase === 'Errors') return 'red'
  return 'gray'
}

function isTraceIndexingError(error: unknown) {
  const status = typeof error === 'object' && error ? (error as { status?: number }).status : undefined
  const message = error instanceof Error ? error.message : String(error || '')
  return status === 404 || message.includes('API error 404')
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
}

function cleanRecord(record: Record<string, unknown>) {
  return Object.fromEntries(
    Object.entries(record).filter(([, value]) => value !== undefined && value !== null && value !== ''),
  )
}

function stringifyValue(value: unknown) {
  if (typeof value === 'string') return value
  return JSON.stringify(value, null, 2)
}

function buildAuditUrl(paymentId: string) {
  const qs = queryString({
    range: '1d',
    page: 1,
    page_size: 25,
    payment_id: paymentId,
  })
  return `/analytics/payment-audit?${qs}`
}

function buildPreviewTraceUrl(paymentId: string) {
  const qs = queryString({
    range: '1d',
    page: 1,
    page_size: 25,
    payment_id: paymentId,
  })
  return `/analytics/preview-trace?${qs}`
}

function buildInspectorModel(event: PaymentAuditEvent | null) {
  if (!event) return null

  const details = isRecord(event.details_json) ? event.details_json : {}
  const explicitResponse =
    details.response ??
    details.response_payload ??
    details.result ??
    details.output ??
    null
  const requestPayload =
    details.request ??
    details.request_payload ??
    details.input ??
    details.payload ??
    cleanRecord({
      payment_id: event.payment_id,
      request_id: event.request_id,
      payment_method_type: event.payment_method_type,
      payment_method: event.payment_method,
      gateway: event.gateway,
    })
  const responsePayload =
    explicitResponse ??
    cleanRecord({
      flow_type: event.flow_type,
      status: event.status,
      error_code: event.error_code,
      error_message: event.error_message,
      score_value: event.score_value,
      sigma_factor: event.sigma_factor,
      average_latency: event.average_latency,
      tp99_latency: event.tp99_latency,
      transaction_count: event.transaction_count,
      rule_name: event.rule_name,
      routing_approach: event.routing_approach,
    })
  const responseRecord = isRecord(explicitResponse) ? explicitResponse : null
  const decidedGatewayRecord = isRecord(responseRecord?.['decided_gateway']) ? responseRecord['decided_gateway'] : null
  const scoreContext =
    details.score_context ??
    (decidedGatewayRecord ? decidedGatewayRecord['gateway_priority_map'] : null) ??
    (responseRecord ? responseRecord['gateway_priority_map'] : null) ??
    null
  const selectionReason = details.selection_reason ?? null

  const summaryRows = [
    { label: 'Phase', value: eventPhase(event) },
    { label: 'Stage', value: stageLabel(event) },
    { label: 'Route', value: routeLabel(event.route) },
    { label: 'Timestamp', value: formatDateTime(event.created_at_ms) },
    ...(event.merchant_id ? [{ label: 'Merchant', value: event.merchant_id }] : []),
    ...(event.payment_id ? [{ label: 'Payment ID', value: event.payment_id }] : []),
    ...(event.request_id ? [{ label: 'Request ID', value: event.request_id }] : []),
    ...(event.gateway ? [{ label: 'Gateway', value: event.gateway }] : []),
    ...(event.status ? [{ label: 'Status', value: humanizeAuditValue(event.status) }] : []),
  ]

  const signalRecord = cleanRecord(
    Object.fromEntries(
      Object.entries(details).filter(([key]) => ![
        'request',
        'request_payload',
        'input',
        'payload',
        'response',
        'response_payload',
        'result',
        'output',
        'score_context',
        'selection_reason',
      ].includes(key)),
    ),
  )

  return {
    summaryRows,
    requestPayload: isRecord(requestPayload) && !Object.keys(requestPayload).length ? null : requestPayload,
    responsePayload: isRecord(responsePayload) && !Object.keys(responsePayload).length ? null : responsePayload,
    scoreContext,
    selectionReason,
    signalRecord: Object.keys(signalRecord).length ? signalRecord : null,
    rawEvent: {
      ...event,
      details_json: event.details_json,
    },
  }
}

function sectionButtonClass(active: boolean) {
  return active
    ? '!border-slate-200 !bg-white !text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.28)] dark:!border-[#2a303a] dark:!bg-[#161b24] dark:!text-white'
    : '!border-transparent !bg-slate-100 !text-slate-600 hover:!bg-slate-200 hover:!text-slate-900 dark:!bg-[#161b24] dark:!text-[#a7b2c6] dark:hover:!bg-[#1c2330] dark:hover:!text-white'
}

function isMissingRoutingSetupError(message: string) {
  const normalized = message.toLowerCase()
  return normalized.includes('no active routing algorithm')
    || normalized.includes('active routing algorithm is not a volume split')
    || normalized.includes('debit_routing_not_enabled')
    || normalized.includes('debit routing is disabled')
}

function setupPromptForTab(tab: TabType, detail?: string): SetupPromptState {
  if (tab === 'volume') {
    return {
      title: 'Configure volume split first',
      body: 'Volume evaluation needs an active volume split rule before it can calculate distribution.',
      detail,
      configurePath: '/routing/volume',
    }
  }

  if (tab === 'rule') {
    return {
      title: 'Configure rule-based routing first',
      body: 'Rule evaluation needs an active rule-based strategy before it can return a policy decision.',
      detail,
      configurePath: '/routing/rules',
    }
  }

  if (tab === 'debit') {
    return {
      title: 'Enable debit routing first',
      body: 'Debit network decisions need the merchant debit routing flag enabled before this explorer can run network routing.',
      detail,
      configurePath: '/routing/debit',
    }
  }

  return {
    title: 'Configure auth-rate routing first',
    body: 'Auth-rate simulation needs success-rate routing configured before it can run gateway decisions.',
    detail,
    configurePath: '/routing/sr',
  }
}


function EmptyAuditState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-[22px] border border-dashed border-slate-200 bg-slate-50/80 px-6 py-12 text-center dark:border-[#2a303a] dark:bg-[#161b24]/80">
      <p className="text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#b2bdd1]">{body}</p>
    </div>
  )
}

function PendingAuditState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-[22px] border border-slate-200 bg-slate-50/80 px-6 py-10 text-center dark:border-[#2a303a] dark:bg-[#161b24]/80">
      <div className="flex justify-center">
        <Spinner size={18} />
      </div>
      <p className="mt-4 text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#b2bdd1]">{body}</p>
      <div className="mt-5 h-2 overflow-hidden rounded-full bg-slate-200 dark:bg-[#202734]">
        <div className="h-full w-1/3 animate-pulse rounded-full bg-brand-500" />
      </div>
      <p className="mt-3 text-[11px] uppercase tracking-[0.16em] text-slate-400 dark:text-[#8390a7]">
        Waiting for analytics
      </p>
    </div>
  )
}

function InspectorKeyValueGrid({ rows }: { rows: Array<{ label: string; value: string }> }) {
  if (!rows.length) return null

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {rows.map((row) => (
        <div
          key={`${row.label}-${row.value}`}
          className="rounded-[22px] border border-slate-200 bg-white/80 px-4 py-3 shadow-[0_14px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#161b24] dark:shadow-none"
        >
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8390a7]">
            {row.label}
          </p>
          <p className="mt-2 break-words text-sm text-slate-900 dark:text-white">{row.value}</p>
        </div>
      ))}
    </div>
  )
}

function InspectorJsonPanel({
  title,
  value,
  emptyMessage,
}: {
  title: string
  value: unknown
  emptyMessage: string
}) {
  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-semibold text-slate-900 dark:text-white">{title}</h3>
      </div>
      {value ? (
        <pre className="overflow-x-auto rounded-[22px] border border-slate-200/80 bg-slate-50/90 px-4 py-4 font-mono text-xs leading-6 text-slate-800 shadow-[inset_0_1px_0_rgba(255,255,255,0.75),0_16px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
          {stringifyValue(value)}
        </pre>
      ) : (
        <EmptyAuditState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Extracts RuleEvaluateParams from a routing algorithm's Euclid conditions.
// Walks all rules and statements, collecting unique lhs keys with their first
// concrete value. Non-Advanced algorithms (Priority/Single) have no conditions
// so return an empty array.
// ---------------------------------------------------------------------------
export function DecisionSimulatorPage() {
  const navigate = useNavigate()
  const { merchantId } = useMerchantStore()
  const authUser = useAuthStore((state) => state.user)
  const authMerchantId = authUser?.merchantId || ''
  const effectiveMerchantId = merchantId || authMerchantId
  const currentScopeKey = explorerScopeKey(
    authUser?.userId || '',
    authUser?.email || '',
    effectiveMerchantId,
  )
  const merchantFeatures = useMerchantFeatures(effectiveMerchantId || undefined)
  const gsmScoringFilterEnabled = merchantFeatures.isEnabled('gsm-scoring-filter')
  const debitRoutingFlag = useDebitRoutingFlag(effectiveMerchantId)
  // Connectors we've learned a cost model for from ingested settlement reports. In the
  // multi-objective (economic-value) sim these drive the eligible-gateway set, so the ranking is
  // scored on real fitted costs instead of the static stripe+adyen default — see the seeding effect
  // below. Only connectors with a fit (`model_pct_bps != null`) qualify.
  const { fees: connectorFees } = useConnectorFees(effectiveMerchantId || undefined)
  const ingestedConnectors = useMemo(
    () => connectorFees.filter(f => f.model_pct_bps != null).map(f => f.connector.toLowerCase()),
    [connectorFees],
  )
  const { routingKeysConfig, isLoading: routingKeysLoading, error: routingKeysError } = useDynamicRoutingConfig()
  const hasRoutingKeys = Object.keys(routingKeysConfig).length > 0
  const routingConfigUnavailable = !routingKeysLoading && (!hasRoutingKeys || Boolean(routingKeysError))
  const initialState = useMemo(() => loadExplorerState(currentScopeKey), [currentScopeKey])
  const [activeTab, setActiveTab] = useState<TabType>(initialState.activeTab)
  const [stateScopeKey, setStateScopeKey] = useState(initialState.scopeKey || currentScopeKey)
  const [resultDataUpdatedAtMs, setResultDataUpdatedAtMs] = useState<number | null>(
    initialState.resultDataUpdatedAtMs,
  )

  const [form, setForm] = useState<FormState>(initialState.form)
  // The run loop closes over `form` when it starts and idles on that same closure
  // across a pause, so a plain read would freeze at the pre-pause values. Mirror
  // it into a ref (like amountRangeRef / multiObjScenarioRef) and read that live so
  // a currency switch made while paused takes effect on the next resumed txn.
  const formRef = useRef(form)
  useEffect(() => { formRef.current = form }, [form])

  const [simulationConfig, setSimulationConfig] = useState<SimulationConfig>(initialState.simulationConfig)
  // Live amount range: the run loop reads this ref (not the captured config) so dragging
  // the Amount-range slider mid-run takes effect on the next transaction.
  const amountRangeRef = useRef({ min: simulationConfig.minAmount, max: simulationConfig.maxAmount })
  useEffect(() => {
    amountRangeRef.current = { min: simulationConfig.minAmount, max: simulationConfig.maxAmount }
  }, [simulationConfig.minAmount, simulationConfig.maxAmount])
  const [errorInfo, setErrorInfo] = useState<ErrorInfoState>(initialState.errorInfo)
  const [gatewaySimConfigs, setGatewaySimConfigs] = useState<Record<string, GatewaySimConfig>>(initialState.gatewaySimConfigs)
  const gatewaySimConfigsRef = useRef(gatewaySimConfigs)
  useEffect(() => { gatewaySimConfigsRef.current = gatewaySimConfigs }, [gatewaySimConfigs])
  const eliminationEnabled = Object.values(gatewaySimConfigs).some(c => c.failureMode === 'timeout')
  const simulationAbortRef = useRef(false)
  useEffect(() => () => { simulationAbortRef.current = true }, [])

  const [debitForm, setDebitForm] = useState<DebitRoutingFormState>(initialState.debitForm)

  const [ruleResetSignal, setRuleResetSignal] = useState(0)

  const [ruleParams, setRuleParams] = useState<RuleEvaluateParams[]>(initialState.ruleParams)

  const [fallbackConnectors, setFallbackConnectors] = useState<GatewayConnector[]>(initialState.fallbackConnectors)

  const [volumePayments, setVolumePayments] = useState<string>(initialState.volumePayments)

  const [result, setResult] = useState<DecideGatewayResponse | null>(initialState.result)
  const [debitResult, setDebitResult] = useState<DecideGatewayResponse | null>(initialState.debitResult)
  const [debitPaymentId, setDebitPaymentId] = useState<string | null>(initialState.debitPaymentId)
  const [singleRunPaymentId, setSingleRunPaymentId] = useState<string | null>(initialState.singleRunPaymentId)
  const [singleRunOutcome, setSingleRunOutcome] = useState<TransactionOutcome>(initialState.singleRunOutcome)
  const [ruleResult, setRuleResult] = useState<RuleEvaluateResponse | null>(initialState.ruleResult)
  const [volumeDistribution, setVolumeDistribution] = useState<{ name: string; count: number; percentage: number }[]>(initialState.volumeDistribution)
  const [volumeEvaluationLog, setVolumeEvaluationLog] = useState<VolumePaymentEntry[]>(initialState.volumeEvaluationLog)
  const [volumeProgress, setVolumeProgress] = useState(initialState.volumeProgress)
  const [simulationResults, setSimulationResults] = useState<SimulationResult[]>(initialState.simulationResults)
  // Live mirror of simulationResults so a resumed run can seed its local `results`
  // array from the completed rows without a stale closure.
  const simulationResultsRef = useRef(simulationResults)
  useEffect(() => { simulationResultsRef.current = simulationResults }, [simulationResults])
  const [isSimulating, setIsSimulating] = useState(false)
  // Pause/resume: the run loop stays alive and idles on this ref instead of being
  // aborted, so position, outcome accumulators, the feed, and backend scores are
  // all preserved across a pause.
  const [isPaused, setIsPaused] = useState(false)
  const simulationPausedRef = useRef(false)
  // Batch index (loop `start`) the run should continue from. The pause-idle block
  // writes the next-unrun index here so it survives leaving and returning to the page.
  const runProgressRef = useRef(0)
  // A paused run that outlived an unmount (e.g. the user paused, opened Multi-Objective
  // config, then came back). Persisted so returning offers "Resume" — which continues
  // from runProgressRef's saved index instead of restarting — rather than a fresh run.
  const [resumableRun, setResumableRun] = useState<{ total: number; nextIndex: number } | null>(initialState.resumableRun)
  // Per-column filters for the Transaction Log table.
  const [txFilters, setTxFilters] = useState<Record<string, string>>({})
  const [smartRetryEnabled, setSmartRetryEnabled] = useState(initialState.smartRetryEnabled)
  // The batch run loop closes over this at start, so mirror it into a ref (like formRef /
  // gatewaySimConfigsRef) and read that live — toggling Retry mid-run then takes effect on the
  // next transaction instead of only on the next run.
  const smartRetryEnabledRef = useRef(smartRetryEnabled)
  useEffect(() => { smartRetryEnabledRef.current = smartRetryEnabled }, [smartRetryEnabled])
  const [isHardRefreshing, setIsHardRefreshing] = useState(false)
  // Confirmation popup guard for the destructive hard refresh.
  const [hardRefreshConfirmOpen, setHardRefreshConfirmOpen] = useState(false)
  // Connectors the user manually added to the comparison via the "+" control. Unioned onto the
  // default pair + ingested set in the seeding effect below, so a third (or fourth) processor
  // gets its own SR slider alongside the ingested ones.
  const [extraConnectors, setExtraConnectors] = useState<string[]>([])
  // Connectors the user explicitly removed from the comparison (the "×" on a slider). Subtracted
  // from the default+ingested+extra union in the seeding effect, so even a default (stripe/adyen)
  // or ingested connector can be dropped and won't be re-added on the next render. Lowercased.
  const [removedConnectors, setRemovedConnectors] = useState<string[]>([])
  // Open state + draft input for the "add connector" modal.
  const [addConnectorOpen, setAddConnectorOpen] = useState(false)
  const [newConnectorName, setNewConnectorName] = useState('')
  const [addConnectorError, setAddConnectorError] = useState<string | null>(null)
  // Collapses the advanced batch controls (TPS, Add processor, Retry) behind a "More" toggle so the
  // default control bar stays uncluttered.
  const [showMoreInputs, setShowMoreInputs] = useState(false)
  // Multi-objective card scenario: 'ALL' rotates the 8 variants round-robin (mixes dimensions
  // on one chart); a numeric index pins every txn to a single scenario so the SR Trend shows
  // one clean per-segment bucket instead of an interleaved sawtooth. The run loop reads the
  // ref (not the captured value) so the scenario can be switched while paused and the rest of
  // the run continues in the new segment — the dropdown is locked only while actively running.
  const [multiObjScenario, setMultiObjScenario] = useState<number | 'ALL'>(initialState.multiObjScenario)
  const multiObjScenarioRef = useRef<number | 'ALL'>(multiObjScenario)
  useEffect(() => { multiObjScenarioRef.current = multiObjScenario }, [multiObjScenario])
  // Currency for the multi-objective sim (was hardcoded to USD). Drives whether a transaction can
  // match an in-house fitted cost model (keyed by currency) or falls back to the seed table.
  const [moCurrency, setMoCurrency] = useState<string>(initialState.moCurrency)
  // Acceptance channel sent on each multi-objective transaction. Drives the in-house category
  // predictor (ecom vs pos resolves the online/in-person interchange split). VGS-collected cards
  // are ecommerce, so this is fixed to 'ecom'.
  const moChannel: 'ecom' | 'pos' = 'ecom'
  const [error, setError] = useState<string | null>(null)
  const txLogRef = useRef<HTMLDivElement>(null)

  const [setupPrompt, setSetupPrompt] = useState<SetupPromptState | null>(null)
  const [loading, setLoading] = useState(false)
  const [filterOpen, setFilterOpen] = useState(false)
  const [responseOpen, setResponseOpen] = useState(initialState.responseOpen)
  const [debitResponseOpen, setDebitResponseOpen] = useState(initialState.debitResponseOpen)
  const [volumeResponseOpen, setVolumeResponseOpen] = useState(initialState.volumeResponseOpen)
  const [selectedAuditPaymentId, setSelectedAuditPaymentId] = useState<string | null>(null)
  const [selectedAuditEventId, setSelectedAuditEventId] = useState<string | null>(null)
  const [auditInspectorTab, setAuditInspectorTab] = useState<AuditInspectorTab>('summary')
  const [selectedPreviewPaymentId, setSelectedPreviewPaymentId] = useState<string | null>(null)
  const [selectedPreviewEventId, setSelectedPreviewEventId] = useState<string | null>(null)
  const [previewInspectorTab, setPreviewInspectorTab] = useState<AuditInspectorTab>('summary')
  const [previewTraceLabel, setPreviewTraceLabel] = useState('Rule Evaluation Decision')
  const deferredSimulationResults = useDeferredValue(simulationResults)
  // Timestamp (ms) the active batch run started; null until the user runs one. Scopes
  // the Events panel to the current run instead of the merchant's whole last hour.
  const [simulationStartedAtMs, setSimulationStartedAtMs] = useState<number | null>(null)
  // Live routing-event stream (leader flips, auth-band crossings), filtered to the run.
  // Poll tightly while a run is producing events so the Autopilot feed keeps up;
  // relax back to the idle cadence once it finishes.
  const routingEvents = useRoutingEvents('1h', {
    refreshInterval: isSimulating && !isPaused ? 500 : 15_000,
  })
  const sessionRoutingEvents = useMemo(() => {
    if (simulationStartedAtMs == null) return []
    const sinceMs = simulationStartedAtMs - EVENTS_RUN_START_MARGIN_MS
    return routingEvents.events.filter((event) => event.bucket_ms >= sinceMs)
  }, [routingEvents.events, simulationStartedAtMs])
  // Keep the run's feed append-only. The backend re-detects every event from
  // ClickHouse on each poll, and the trailing (still-filling) 1s bucket can make a
  // just-emitted event flicker out on the next scan — so replacing the list each
  // poll makes recent rows visibly vanish. Instead we merge each snapshot into a
  // stable, ID-keyed accumulator (event IDs are deterministic), so once a row
  // appears it stays for the run. Reset whenever a new run starts/clears.
  const [accumulatedEvents, setAccumulatedEvents] = useState<RoutingEvent[]>([])
  const accumulatedEventIdsRef = useRef<Set<string>>(new Set())
  useEffect(() => {
    accumulatedEventIdsRef.current = new Set()
    setAccumulatedEvents([])
  }, [simulationStartedAtMs])
  useEffect(() => {
    const unseen = sessionRoutingEvents.filter((e) => !accumulatedEventIdsRef.current.has(e.id))
    if (unseen.length === 0) return
    unseen.forEach((e) => accumulatedEventIdsRef.current.add(e.id))
    setAccumulatedEvents((prev) =>
      prev
        .concat(unseen)
        .sort((a, b) => b.bucket_ms - a.bucket_ms || b.id.localeCompare(a.id))
        .slice(0, 200),
    )
  }, [sessionRoutingEvents])
  // Briefly highlight freshly-arrived Autopilot actions so a new leader change /
  // band crossing catches the eye, then fades back. We diff incoming event IDs
  // against the ones already seen; the very first batch is baselined silently so
  // an existing backlog doesn't all flash at once on mount.
  const [highlightedEventIds, setHighlightedEventIds] = useState<Set<string>>(new Set())
  const seenEventIdsRef = useRef<Set<string>>(new Set())
  const baselinedEventsRef = useRef(false)
  const highlightTimersRef = useRef<number[]>([])
  useEffect(() => {
    const ids = accumulatedEvents.map((e) => e.id)
    if (!baselinedEventsRef.current) {
      ids.forEach((id) => seenEventIdsRef.current.add(id))
      baselinedEventsRef.current = true
      return
    }
    const fresh = ids.filter((id) => !seenEventIdsRef.current.has(id))
    if (fresh.length === 0) return
    fresh.forEach((id) => seenEventIdsRef.current.add(id))
    setHighlightedEventIds((prev) => {
      const next = new Set(prev)
      fresh.forEach((id) => next.add(id))
      return next
    })
    const timer = window.setTimeout(() => {
      setHighlightedEventIds((prev) => {
        const next = new Set(prev)
        fresh.forEach((id) => next.delete(id))
        return next
      })
    }, 12000)
    highlightTimersRef.current.push(timer)
  }, [accumulatedEvents])
  useEffect(() => () => { highlightTimersRef.current.forEach((t) => window.clearTimeout(t)) }, [])
  // Near-tied gateways produce storms of two kinds: a score parked on the auth-band
  // edge re-crosses it on every wobble (enter/exit), and two leaders trading the #1
  // spot fire a leader_changed on every swap. Collapse each consecutive run into a
  // single "contesting" row that reports the net current state + how many times it
  // flipped, so the feed stays readable instead of scrolling identical lines.
  type FeedItem =
    | { kind: 'single'; event: RoutingEvent }
    | { kind: 'flap'; gateway: string; crossings: number; latest: RoutingEvent; inBand: boolean }
    | { kind: 'leaderFlap'; gateways: string[]; crossings: number; latest: RoutingEvent }
  const collapsedRoutingEvents = useMemo<FeedItem[]>(() => {
    const isBand = (t: RoutingEventType) =>
      t === 'gateway_entered_auth_band' || t === 'gateway_exited_auth_band'
    const list = accumulatedEvents.slice(0, 50)
    const items: FeedItem[] = []
    let i = 0
    while (i < list.length) {
      const ev = list[i]
      if (isBand(ev.event_type)) {
        // list is newest-first; gather the adjacent run for this gateway.
        let j = i
        while (j < list.length && isBand(list[j].event_type) && list[j].gateway === ev.gateway) j++
        const crossings = j - i
        if (crossings > 1) {
          items.push({
            kind: 'flap',
            gateway: ev.gateway,
            crossings,
            latest: ev,
            inBand: ev.event_type === 'gateway_entered_auth_band',
          })
        } else {
          items.push({ kind: 'single', event: ev })
        }
        i = j
      } else if (ev.event_type === 'leader_changed') {
        // Gather the adjacent run of lead swaps (any direction).
        let j = i
        while (j < list.length && list[j].event_type === 'leader_changed') j++
        const crossings = j - i
        if (crossings > 1) {
          const gateways = Array.from(new Set(list.slice(i, j).map((e) => e.gateway))).sort()
          items.push({ kind: 'leaderFlap', gateways, crossings, latest: ev })
        } else {
          items.push({ kind: 'single', event: ev })
        }
        i = j
      } else {
        items.push({ kind: 'single', event: ev })
        i++
      }
    }
    return items
  }, [accumulatedEvents])
  const [showPenaltyGuide, setShowPenaltyGuide] = useState(false)
  // SR / volume chart window: focus on the most recent N transactions (auto-follows
  // the live stream) or 'all' for the full-session view. Both charts share this so
  // their x-axes stay aligned.
  const [chartWindow, setChartWindow] = useState<number | 'all'>(100)
  const CHART_WINDOW_OPTIONS: (number | 'all')[] = [100, 500, 'all']

  const routingKeyNames = useMemo(
    () => Object.keys(routingKeysConfig).sort(),
    [routingKeysConfig]
  )

  const paymentMethodTypeOptions = useMemo(
    () => toUpperOptions(routingKeysConfig.payment_method?.values || []),
    [routingKeysConfig]
  )

  const currencyOptions = useMemo(
    () => uniqueUpperOptions(routingKeysConfig.currency?.values || []),
    [routingKeysConfig]
  )

  const cardBrandOptions = useMemo(
    () => uniqueUpperOptions(routingKeysConfig.card_network?.values || []),
    [routingKeysConfig]
  )

  const authTypeOptions = useMemo(
    () => uniqueUpperOptions(routingKeysConfig.authentication_type?.values || []),
    [routingKeysConfig]
  )

  const auditUrl = selectedAuditPaymentId
    ? buildAuditUrl(selectedAuditPaymentId)
    : null

  const auditDetail = useSWR<PaymentAuditResponse>(auditUrl, fetcher, {
    revalidateOnFocus: false,
  })

  const previewTraceUrl = selectedPreviewPaymentId
    ? buildPreviewTraceUrl(selectedPreviewPaymentId)
    : null

  const previewTraceDetail = useSWR<PaymentAuditResponse>(previewTraceUrl, fetcher, {
    revalidateOnFocus: false,
  })

  const { data: gsmOptionsData } = useSWR<{ rules: GsmOptionRow[] }>(
    '/gsm/options',
    fetcher,
    { revalidateOnFocus: false, dedupingInterval: 300_000 },
  )
  const gsmRules = gsmOptionsData?.rules ?? []

  useEffect(() => {
    if (routingConfigUnavailable || routingKeysLoading) return

    setForm(prev => {
      const next = { ...prev }
      let changed = false

      if (currencyOptions.length > 0 && !currencyOptions.includes(next.currency)) {
        next.currency = currencyOptions[0]
        changed = true
      }

      if (paymentMethodTypeOptions.length > 0 && !paymentMethodTypeOptions.includes(next.payment_method_type)) {
        next.payment_method_type = paymentMethodTypeOptions[0]
        changed = true
      }

      const methodsForType = toUpperOptions(
        routingKeysConfig[next.payment_method_type.toLowerCase()]?.values || []
      )
      if (methodsForType.length > 0 && !methodsForType.includes(next.payment_method)) {
        next.payment_method = methodsForType[0]
        changed = true
      }

      if (authTypeOptions.length > 0 && !authTypeOptions.includes(next.auth_type)) {
        next.auth_type = authTypeOptions[0]
        changed = true
      }

      if (cardBrandOptions.length > 0 && !cardBrandOptions.includes(next.card_brand)) {
        next.card_brand = cardBrandOptions[0]
        changed = true
      }

      return changed ? next : prev
    })

    setRuleParams(prev => {
      let changed = false
      const next = prev.map(param => {
        if (!param.key || !routingKeysConfig[param.key]) return param
        const keyConfig = routingKeysConfig[param.key]
        const mappedType = mapRoutingTypeToRuleParamType(keyConfig.type)
        const enumValues = keyConfig.values || []
        const nextValue = mappedType === 'enum_variant'
          ? (enumValues.includes(param.value) ? param.value : (enumValues[0] || ''))
          : param.value
        if (param.type !== mappedType || param.value !== nextValue) {
          changed = true
          return { ...param, type: mappedType, value: nextValue }
        }
        return param
      })
      return changed ? next : prev
    })
  }, [
    routingConfigUnavailable,
    routingKeysLoading,
    routingKeysConfig,
    currencyOptions,
    paymentMethodTypeOptions,
    authTypeOptions,
    cardBrandOptions,
  ])

  // SR_MULTI_OBJECTIVE constrains the form to a card-only cluster shape:
  // Currency = the selected `moCurrency`, Method Type = CARD, Payment Method = CREDIT, and Card
  // Brand defaults to Visa/Mastercard when the prior selection isn't one of them.
  useEffect(() => {
    if (form.ranking_algorithm !== 'SR_MULTI_OBJECTIVE') return
    setForm(prev => {
      if (prev.ranking_algorithm !== 'SR_MULTI_OBJECTIVE') return prev
      const next = { ...prev }
      let changed = false
      if (next.currency !== moCurrency) { next.currency = moCurrency; changed = true }
      const cardType = paymentMethodTypeOptions.find(p => p === 'CARD') || 'CARD'
      if (next.payment_method_type !== cardType) { next.payment_method_type = cardType; changed = true }
      if (!MULTI_OBJECTIVE_PAYMENT_METHODS.includes(next.payment_method as 'CREDIT')) {
        next.payment_method = 'CREDIT'
        changed = true
      }
      if (!MULTI_OBJECTIVE_CARD_BRANDS.includes(next.card_brand as 'VISA' | 'MASTERCARD')) {
        const fallback = cardBrandOptions.find(b => b === 'VISA') || cardBrandOptions.find(b => b === 'MASTERCARD') || cardBrandOptions[0] || 'VISA'
        next.card_brand = fallback
        changed = true
      }
      if (!CARD_PROGRAM_OPTIONS.includes(next.card_program as 'STANDARD' | 'PREMIUM')) {
        next.card_program = 'STANDARD'
        changed = true
      }
      return changed ? next : prev
    })
  }, [form.ranking_algorithm, paymentMethodTypeOptions, cardBrandOptions, moCurrency])

  // Cost mode: drive the eligible connectors from the default pair *unioned* with the ones we've
  // learned a cost model for (so an ingested connector like Adyen doesn't collapse the comparison
  // to a single processor — the default pair is always kept for a side-by-side). Ingested connectors
  // are scored on their real fitted costs; the default ones fall back to seed costs. SR-based mode
  // just uses the default pair. Either way we also union in any connectors the user added via the
  // "+" control. There is no manual text editor for this field, so owning it here clobbers nothing;
  // we stay out of the way of an in-flight run.
  useEffect(() => {
    if (isSimulating) return
    const merged =
      form.ranking_algorithm === 'SR_MULTI_OBJECTIVE'
        ? unionConnectors(DEFAULT_ELIGIBLE_GATEWAYS, ingestedConnectors, extraConnectors)
        : unionConnectors(DEFAULT_ELIGIBLE_GATEWAYS, extraConnectors)
    // Subtract any connectors the user explicitly removed so a dropped default/ingested one
    // stays out (this effect owns the field, so without the subtraction it would re-add them).
    const removed = new Set(removedConnectors)
    const target = merged.filter(gw => !removed.has(gw)).join(', ')
    setForm(prev => (prev.eligible_gateways === target ? prev : { ...prev, eligible_gateways: target }))
  }, [form.ranking_algorithm, ingestedConnectors, extraConnectors, removedConnectors, isSimulating])

  useEffect(() => {
    if (!selectedAuditPaymentId && !selectedPreviewPaymentId && !setupPrompt) return

    const previousOverflow = document.body.style.overflow
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setSelectedAuditPaymentId(null)
        setSelectedAuditEventId(null)
        setAuditInspectorTab('summary')
        setSelectedPreviewPaymentId(null)
        setSelectedPreviewEventId(null)
        setPreviewInspectorTab('summary')
        setSetupPrompt(null)
      }
    }

    document.body.style.overflow = 'hidden'
    window.addEventListener('keydown', onKeyDown)

    return () => {
      document.body.style.overflow = previousOverflow
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [selectedAuditPaymentId, selectedPreviewPaymentId, setupPrompt])

  function clearExplorerRunData() {
    const defaults = getDefaultExplorerState()
    setResult(defaults.result)
    setDebitResult(defaults.debitResult)
    setDebitPaymentId(defaults.debitPaymentId)
    setSingleRunPaymentId(defaults.singleRunPaymentId)
    setSingleRunOutcome(defaults.singleRunOutcome)
    setRuleResult(defaults.ruleResult)
    setVolumeDistribution(defaults.volumeDistribution)
    setVolumeEvaluationLog(defaults.volumeEvaluationLog)
    setVolumeProgress(defaults.volumeProgress)
    setSimulationResults(defaults.simulationResults)
    setSimulationStartedAtMs(null)
    setResponseOpen(defaults.responseOpen)
    setDebitResponseOpen(defaults.debitResponseOpen)
    setVolumeResponseOpen(defaults.volumeResponseOpen)
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
    setSelectedPreviewPaymentId(null)
    setSelectedPreviewEventId(null)
    setPreviewInspectorTab('summary')
    setPreviewTraceLabel('Rule Evaluation Decision')
    setResultDataUpdatedAtMs(null)
    setResumableRun(null)
    setError(null)
    setSetupPrompt(null)
    setLoading(false)
    simulationAbortRef.current = true
    setIsSimulating(false)
  }

  function markExplorerRunDataUpdated() {
    setResultDataUpdatedAtMs(Date.now())
  }

  function applyExplorerState(nextState: ExplorerPersistedState, scopeKey: string) {
    setActiveTab(nextState.activeTab)
    setStateScopeKey(nextState.scopeKey || scopeKey)
    setResultDataUpdatedAtMs(nextState.resultDataUpdatedAtMs)
    setForm(nextState.form)
    setSimulationConfig(nextState.simulationConfig)
    setGatewaySimConfigs(nextState.gatewaySimConfigs)
    setErrorInfo(nextState.errorInfo)
    setDebitForm(nextState.debitForm)
    setRuleParams(nextState.ruleParams)
    setFallbackConnectors(nextState.fallbackConnectors)
    setVolumePayments(nextState.volumePayments)
    setResult(nextState.result)
    setDebitResult(nextState.debitResult)
    setDebitPaymentId(nextState.debitPaymentId)
    setSingleRunPaymentId(nextState.singleRunPaymentId)
    setSingleRunOutcome(nextState.singleRunOutcome)
    setRuleResult(nextState.ruleResult)
    setVolumeDistribution(nextState.volumeDistribution)
    setVolumeEvaluationLog(nextState.volumeEvaluationLog)
    setVolumeProgress(nextState.volumeProgress)
    setSimulationResults(nextState.simulationResults)
    setSimulationStartedAtMs(null)
    setResponseOpen(nextState.responseOpen)
    setDebitResponseOpen(nextState.debitResponseOpen)
    setVolumeResponseOpen(nextState.volumeResponseOpen)
    setMultiObjScenario(nextState.multiObjScenario)
    setMoCurrency(nextState.moCurrency)
    setResumableRun(nextState.resumableRun)
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
    setSelectedPreviewPaymentId(null)
    setSelectedPreviewEventId(null)
    setPreviewInspectorTab('summary')
    setPreviewTraceLabel('Rule Evaluation Decision')
    setFilterOpen(false)
    setError(null)
    setSetupPrompt(null)
    setLoading(false)
    simulationAbortRef.current = true
    setIsSimulating(false)
  }

  useEffect(() => {
    if (stateScopeKey === currentScopeKey) return
    applyExplorerState(loadExplorerState(currentScopeKey), currentScopeKey)
  }, [currentScopeKey, stateScopeKey])

  useEffect(() => {
    if (!resultDataUpdatedAtMs) return

    const storageScopeKey = stateScopeKey || currentScopeKey
    const remainingMs = EXPLORER_RESULT_TTL_MS - (Date.now() - resultDataUpdatedAtMs)
    if (remainingMs <= 0) {
      clearExplorerRunData()
      removeExplorerState(storageScopeKey)
      return
    }

    const timer = window.setTimeout(() => {
      clearExplorerRunData()
      removeExplorerState(storageScopeKey)
    }, remainingMs)

    return () => window.clearTimeout(timer)
  }, [currentScopeKey, resultDataUpdatedAtMs, stateScopeKey])

  useEffect(() => {
    if (stateScopeKey !== currentScopeKey) return
    if (loading) return
    // Skip persisting while actively running — results are flushed once the run
    // completes. A *paused* run is the exception: we snapshot it so leaving the page
    // (e.g. to edit Multi-Objective config) and returning can resume from where it
    // stopped instead of restarting.
    if (isSimulating && !isPaused) return

    // While live-paused, snapshot the next-unrun index from the loop's ref; otherwise
    // carry the existing snapshot forward so it survives config edits until resumed.
    const resumableRunSnapshot = isPaused
      ? { total: parseInt(simulationConfig.totalPayments) || 0, nextIndex: runProgressRef.current }
      : resumableRun

    const nextState: ExplorerPersistedState = {
      scopeKey: currentScopeKey,
      resultDataUpdatedAtMs,
      activeTab,
      form,
      simulationConfig,
      gatewaySimConfigs,
      errorInfo,
      debitForm,
      ruleParams,
      fallbackConnectors,
      volumePayments,
      result,
      debitResult,
      debitPaymentId,
      singleRunPaymentId,
      singleRunOutcome,
      ruleResult,
      volumeDistribution,
      volumeEvaluationLog,
      volumeProgress,
      simulationResults,
      responseOpen,
      debitResponseOpen,
      volumeResponseOpen,
      smartRetryEnabled,
      multiObjScenario,
      moCurrency,
      resumableRun: resumableRunSnapshot,
    }

    saveExplorerState(currentScopeKey, nextState)
  }, [
    currentScopeKey,
    stateScopeKey,
    isSimulating,
    isPaused,
    resumableRun,
    loading,
    resultDataUpdatedAtMs,
    activeTab,
    form,
    simulationConfig,
    gatewaySimConfigs,
    errorInfo,
    debitForm,
    ruleParams,
    fallbackConnectors,
    volumePayments,
    result,
    debitResult,
    debitPaymentId,
    singleRunPaymentId,
    singleRunOutcome,
    ruleResult,
    volumeDistribution,
    volumeEvaluationLog,
    volumeProgress,
    simulationResults,
    responseOpen,
    debitResponseOpen,
    volumeResponseOpen,
    smartRetryEnabled,
    multiObjScenario,
    moCurrency,
  ])

  // Resume a paused run that survived a page unmount: continue the loop from the saved
  // index, appending to the completed rows rather than starting a fresh run.
  function resumeFromSnapshot() {
    if (!resumableRun) return
    runSimulation({ resumeFrom: resumableRun.nextIndex })
  }

  // Drop a resumable snapshot without resuming (start clean next time).
  function discardResumableRun() {
    setResumableRun(null)
    clearExplorerRunData()
  }

  function setDebitField<K extends keyof DebitRoutingFormState>(field: K, value: DebitRoutingFormState[K]) {
    setDebitForm(f => ({ ...f, [field]: value }))
  }

  function setErrorField(updates: Partial<ErrorInfoState>) {
    setErrorInfo(f => ({ ...f, ...updates }))
  }

  function getGwSimConfig(gw: string): GatewaySimConfig {
    return gatewaySimConfigsRef.current[gw] ?? defaultGwSimConfig(gw)
  }

  function getGwSuccessRate(gw: string): number {
    return getGwSimConfig(gw).successRate
  }

  function getGwFailureMode(gw: string): 'decline' | 'timeout' {
    return getGwSimConfig(gw).failureMode
  }

  function setGwSuccessRate(gw: string, rate: number) {
    setGatewaySimConfigs(c => ({ ...c, [gw]: { ...getGwSimConfig(gw), ...c[gw], successRate: rate } }))
  }

  // Clamp a success-rate value into 0–100 and snap to 2 decimals so the input/slider
  // can express decimal SRs (e.g. 98.45) without accumulating float noise.
  function clampSuccessRate(value: number): number {
    if (!Number.isFinite(value)) return 0
    return Math.round(Math.max(0, Math.min(100, value)))
  }

  function openAddConnector() {
    setNewConnectorName('')
    setAddConnectorError(null)
    setAddConnectorOpen(true)
  }

  // Validate the modal input and add it to the comparison. The seeding effect unions it into the
  // eligible-gateway set, so a new SR slider appears for it on the next render.
  function submitAddConnector() {
    const name = newConnectorName.trim().toLowerCase()
    if (!name) {
      setAddConnectorError('Enter a connector name.')
      return
    }
    if (!/^[a-z0-9_-]+$/.test(name)) {
      setAddConnectorError('Use only letters, numbers, hyphens or underscores.')
      return
    }
    if (eligibleGatewaysParsed.includes(name)) {
      setAddConnectorError(`${name.charAt(0).toUpperCase() + name.slice(1)} is already in the comparison.`)
      return
    }
    setExtraConnectors(prev => (prev.includes(name) ? prev : [...prev, name]))
    // Clear it from the removed set so re-adding a previously-dropped default/ingested
    // connector (e.g. stripe) actually brings it back.
    setRemovedConnectors(prev => prev.filter(c => c !== name))
    setAddConnectorOpen(false)
  }

  // Drop a connector from the comparison (the "×" on its slider). Works for any source: extras are
  // removed from that list, and defaults/ingested connectors go into removedConnectors so the
  // seeding effect subtracts them instead of re-adding them.
  function removeConnector(name: string) {
    const key = name.trim().toLowerCase()
    setExtraConnectors(prev => prev.filter(c => c !== key))
    setRemovedConnectors(prev => (prev.includes(key) ? prev : [...prev, key]))
  }

  // Finds a real error code for `connector` that produces the desired GSM decision + penalty.
  function resolveSimErrorInfo(connector: string, gsmDecision: 'retry' | 'do_default', penalized: boolean) {
    const penalizedMessages = new Set(['Issue with Integration', 'Issue with Configurations', 'Technical issue with PSP', 'Something went wrong'])
    const notPenalizedMessages = new Set(['Issue with Payment Method details'])
    const targetMessages = penalized ? penalizedMessages : notPenalizedMessages
    return gsmRules.find(r =>
      r.connector === connector &&
      (r.subFlow === 'Authorize' || r.flow === 'Authorize') &&
      r.decision === gsmDecision &&
      r.unifiedMessage != null &&
      targetMessages.has(r.unifiedMessage) &&
      !!r.errorCode && r.errorCode.toLowerCase() !== 'no error code'
    )
  }

  // Resolves the best-matching GSM rule for a connector+errorCode pair.
  // Mirrors the priority used in ErrorInfoFields: prefer subFlow=Authorize
  // (payment-auth path, newer rules) over flow=Authorize (general auth flow),
  // then fall back to any rule for that error code.
  function resolveGsmRule(connector: string, errorCode: string) {
    return (
      gsmRules.find(r => r.connector === connector && r.errorCode === errorCode && r.subFlow === 'Authorize') ??
      gsmRules.find(r => r.connector === connector && r.errorCode === errorCode && r.flow === 'Authorize') ??
      gsmRules.find(r => r.connector === connector && r.errorCode === errorCode)
    )
  }

  // Uses stored errorInfo (auto-populated from toggles or manually overridden by user).
  function buildSimErrorInfo(gateway: string) {
    if (!gateway) return undefined
    const info = getGwSimConfig(gateway).errorInfo
    if (!info.error_code) return undefined
    const rule = resolveGsmRule(gateway, info.error_code)
    return {
      connector: gateway,
      ...(rule?.flow && { flow: rule.flow }),
      ...(rule?.subFlow && { subFlow: rule.subFlow }),
      errorCode: info.error_code,
      ...(info.error_message && { errorMessage: info.error_message }),
      ...(info.issuer_error_code && { issuerErrorCode: info.issuer_error_code }),
      ...(info.card_network && { cardNetwork: info.card_network }),
    }
  }

  // Builds an error payload for a simulated failure, picking a real GSM error code whose
  // decision matches whether this failure should be retryable. A retryable failure carries
  // a soft-decline (GSM `retry`) code so the backend/audit agree it can be retried on an
  // alternate processor; otherwise a hard-decline (`do_default`) code. Falls back to the
  // gateway's configured sim error when no matching rule is loaded.
  function buildSimFailureErrorInfo(gateway: string, retryable: boolean) {
    if (!gateway) return undefined
    const resolved = resolveSimErrorInfo(gateway, retryable ? 'retry' : 'do_default', getGwSimConfig(gateway).penalized)
    if (resolved?.errorCode) {
      return {
        connector: gateway,
        ...(resolved.flow && { flow: resolved.flow }),
        ...(resolved.subFlow && { subFlow: resolved.subFlow }),
        errorCode: resolved.errorCode,
        ...(resolved.errorMessage && { errorMessage: resolved.errorMessage }),
      }
    }
    return buildSimErrorInfo(gateway)
  }

  function buildErrorInfo(gateway: string, info: ErrorInfoState = errorInfo) {
    if (!gateway) return undefined
    if (!info.error_code) return undefined
    const rule = resolveGsmRule(gateway, info.error_code)
    return {
      connector: gateway,
      ...(rule?.flow && { flow: rule.flow }),
      ...(rule?.subFlow && { subFlow: rule.subFlow }),
      errorCode: info.error_code,
      ...(info.error_message && { errorMessage: info.error_message }),
      ...(info.issuer_error_code && { issuerErrorCode: info.issuer_error_code }),
      ...(info.card_network && { cardNetwork: info.card_network }),
    }
  }

  function openSetupPrompt(tab: TabType, detail?: string) {
    setError(null)
    setSetupPrompt(setupPromptForTab(tab, detail))
  }

  function handleRunError(errorValue: unknown, tab: TabType, fallback = 'Request failed') {
    const message = errorValue instanceof Error ? errorValue.message : fallback
    if (isMissingRoutingSetupError(message)) {
      openSetupPrompt(tab, message)
      return
    }
    setError(message)
  }

  function buildDebitRoutingMetadata() {
    const networks = debitForm.co_badged_networks
      .split(',')
      .map(network => network.trim().toUpperCase())
      .filter(Boolean)

    return JSON.stringify({
      merchant_category_code: debitForm.merchant_category_code.trim(),
      acquirer_country: debitForm.acquirer_country.trim().toUpperCase(),
      co_badged_card_data: {
        co_badged_card_networks: networks,
        issuer_country: debitForm.issuer_country.trim().toUpperCase(),
        is_regulated: debitForm.is_regulated,
        regulated_name: debitForm.is_regulated && debitForm.regulated_name.trim()
          ? debitForm.regulated_name.trim()
          : null,
        card_type: normalizeDebitCardCategory(debitForm.card_type),
      },
    })
  }

  function addRuleParam() {
    if (routingKeyNames.length === 0) return
    const firstKey = routingKeyNames[0]
    const firstConfig = routingKeysConfig[firstKey]
    const mappedType = mapRoutingTypeToRuleParamType(firstConfig?.type)
    const firstValue = mappedType === 'enum_variant' ? (firstConfig?.values?.[0] || '') : ''
    setRuleParams([...ruleParams, { key: firstKey, type: mappedType, value: firstValue, metadataKey: '' }])
  }

  function removeRuleParam(index: number) {
    setRuleParams(ruleParams.filter((_, i) => i !== index))
  }

  function updateRuleParam(index: number, field: keyof RuleEvaluateParams, value: string) {
    setRuleParams(ruleParams.map((p, i) => i === index ? { ...p, [field]: value } : p))
  }

  function updateRuleParamMetadataKey(index: number, value: string) {
    setRuleParams(ruleParams.map((p, i) => i === index ? { ...p, metadataKey: value } : p))
  }

  function updateRuleParamKey(index: number, key: string) {
    const keyConfig = routingKeysConfig[key]
    const mappedType = mapRoutingTypeToRuleParamType(keyConfig?.type)
    const nextValue = mappedType === 'enum_variant' ? (keyConfig?.values?.[0] || '') : ''
    setRuleParams(ruleParams.map((p, i) => (
      i === index ? { ...p, key, type: mappedType, value: nextValue, metadataKey: '' } : p
    )))
  }

  function addFallbackConnector() {
    setFallbackConnectors([...fallbackConnectors, { gateway_name: '', gateway_id: '' }])
  }

  function removeFallbackConnector(index: number) {
    setFallbackConnectors(fallbackConnectors.filter((_, i) => i !== index))
  }

  function updateFallbackConnector(index: number, field: keyof GatewayConnector, value: string) {
    setFallbackConnectors(fallbackConnectors.map((c, i) => i === index ? { ...c, [field]: value } : c))
  }

  async function run() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')
    setLoading(true); setError(null); setSetupPrompt(null)
    setSingleRunPaymentId(null)
    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    const paymentId = `explorer_${Date.now()}`
    try {
      const res = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchantId: effectiveMerchantId,
        paymentInfo: {
          paymentId: paymentId,
          amount: parseFloat(form.amount) || 1000,
          currency: form.currency,
          paymentType: 'ORDER_PAYMENT',
          paymentMethodType: form.payment_method_type,
          paymentMethod: form.payment_method,
          authType: form.auth_type,
          cardBrand: form.card_brand,
          cardSwitchProvider: form.card_brand,
          cardType: form.payment_method,
          ...(form.ranking_algorithm === 'SR_MULTI_OBJECTIVE' && { cardProgram: form.card_program }),
        },
        eligibleGatewayList: gateways,
        rankingAlgorithm: 'SR_BASED_ROUTING',
        // Multi-objective (cost savings) is governed solely by the merchant's Autopilot
        // "Optimize for economic value" flag — we intentionally don't send
        // enableMultiObjective so the decider falls back to that single source of truth.
        eliminationEnabled: eliminationEnabled,
      })
      const scoreRes = await apiPost<UpdateScoreResponse>('/update-gateway-score', {
        merchantId: effectiveMerchantId,
        gateway: res.decided_gateway,
        gatewayReferenceId: null,
        status: singleRunOutcome,
        paymentId: paymentId,
        enforceDynamicRoutingFailure: null,
        ...(singleRunOutcome === 'FAILURE' && { errorInfo: buildErrorInfo(res.decided_gateway) }),
      })

      // Smart retry: if the failure is retryable and we have a fallback, attempt the next gateway
      if (
        smartRetryEnabled &&
        gsmScoringFilterEnabled &&
        singleRunOutcome === 'FAILURE' &&
        scoreRes.gsm_info?.decision === 'retry' &&
        res.fallback_gateways.length > 0
      ) {
        const retryGateway = res.fallback_gateways[0]
        await apiPost('/update-gateway-score', {
          merchantId: effectiveMerchantId,
          gateway: retryGateway,
          gatewayReferenceId: null,
          status: 'CHARGED',
          paymentId: paymentId,
          enforceDynamicRoutingFailure: null,
          isSmartRetry: true,
        })
        setResult({ ...res, decided_gateway: retryGateway })
        setSingleRunPaymentId(paymentId)
      } else {
        setResult(res)
        setSingleRunPaymentId(paymentId)
      }

      markExplorerRunDataUpdated()
    } catch (e: unknown) {
      handleRunError(e, 'single')
    } finally {
      setLoading(false)
    }
  }

  async function enableDebitRoutingForExplorer() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    setLoading(true)
    setError(null)
    setSetupPrompt(null)

    try {
      await debitRoutingFlag.setDebitRoutingEnabled(true)
    } catch (e: unknown) {
      handleRunError(e, 'debit', 'Failed to enable debit routing')
    } finally {
      setLoading(false)
    }
  }

  async function runDebitRouting() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    if (!debitRoutingFlag.isEnabled) return openSetupPrompt('debit', 'Debit routing is disabled.')

    const gateways = debitForm.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    if (gateways.length === 0) return setError('Add at least one eligible gateway')

    setLoading(true)
    setError(null)
    setSetupPrompt(null)
    setDebitResult(null)
    const paymentId = `debit_${Date.now()}`

    try {
      const res = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchantId: effectiveMerchantId,
        paymentInfo: {
          paymentId,
          amount: parseFloat(debitForm.amount) || 1000,
          currency: debitForm.currency,
          paymentType: 'ORDER_PAYMENT',
          paymentMethodType: 'CARD',
          paymentMethod: 'DEBIT',
          authType: debitForm.auth_type,
          metadata: buildDebitRoutingMetadata(),
        },
        eligibleGatewayList: gateways,
        rankingAlgorithm: 'NTW_BASED_ROUTING',
        eliminationEnabled: false,
      })

      setDebitResult(res)
      setDebitPaymentId(paymentId)
      markExplorerRunDataUpdated()
    } catch (e: unknown) {
      handleRunError(e, 'debit')
    } finally {
      setLoading(false)
    }
  }

  // Hard refresh: flush this merchant's SR scores in Redis so the next run starts from
  // fresh scores, then clear the local results/feed so the charts reset too. Scoped to the
  // current merchant only — other merchants' scores are untouched. Intentionally a small,
  // low-profile icon (see below) since it's a destructive escape hatch, not an everyday action.
  async function hardRefreshScores() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    if (isSimulating) return
    setIsHardRefreshing(true)
    setError(null)
    try {
      await apiPost<{ deleted_keys: number }>('/gateway-score/reset', {
        merchantId: effectiveMerchantId,
      })
      // Clear local state so the UI reflects the fresh-scores starting point.
      setSimulationResults([])
      setTxFilters({})
      routingEvents.refresh()
      setError(null)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to flush gateway scores')
    } finally {
      setIsHardRefreshing(false)
    }
  }

  async function runSimulation(opts: { resumeFrom?: number } = {}) {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')

    const total = parseInt(simulationConfig.totalPayments) || 0

    if (total <= 0) return setError('Total Payments must be greater than 0')

    // Resuming a paused run that outlived a page unmount: continue from the saved index
    // and keep the completed rows. A clamped/out-of-range index falls back to a fresh run.
    const resumeFrom = opts.resumeFrom && opts.resumeFrom > 0 && opts.resumeFrom < total
      ? opts.resumeFrom
      : 0
    const isResume = resumeFrom > 0

    setIsSimulating(true)
    setIsPaused(false)
    simulationPausedRef.current = false
    setResumableRun(null)
    setSimulationStartedAtMs(Date.now())
    setError(null)
    setSetupPrompt(null)
    if (!isResume) setSimulationResults([])
    simulationAbortRef.current = false

    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    // Seed from the completed rows on resume so new transactions append rather than replace.
    const results: SimulationResult[] = isResume ? [...simulationResultsRef.current] : []
    const MAX_CONSECUTIVE_ERRORS = 3
    let consecutiveErrors = 0
    let lastUIUpdate = 0
    // Refreshing the events feed on every 150ms UI tick fires overlapping fetches
    // whose out-of-order responses make rows flicker; ~1s keeps it fresh without
    // the storm (the SWR poll runs at 1s too).
    let lastEventsRefresh = 0

    // Deterministic outcome scheduler (error diffusion). i.i.d. coin flips
    // (Math.random() < rate) have ~±5pp swing over the backend's 125-txn window,
    // which can briefly push a laggard's score above the leader and steal traffic.
    // Error diffusion instead spaces successes/failures evenly so the *realized*
    // rate tracks the slider within ~1/window (≈ ±1pp) — so adyen@90 stays ~90 and
    // never overtakes stripe@96 — without touching backend scoring. Random initial
    // phase per gateway so failures aren't visibly periodic.
    const outcomeAccumulator: Record<string, number> = {}
    const drawSuccess = (gw: string): boolean => {
      const p = Math.max(0, Math.min(1, getGwSuccessRate(gw) / 100))
      if (outcomeAccumulator[gw] === undefined) outcomeAccumulator[gw] = Math.random()
      outcomeAccumulator[gw] += p
      if (outcomeAccumulator[gw] >= 1) {
        outcomeAccumulator[gw] -= 1
        return true
      }
      return false
    }

    const isMultiObjective = form.ranking_algorithm === 'SR_MULTI_OBJECTIVE'

    // Parallel-requests (TPS) lever: how many transactions we keep in flight at once (the worker
    // pool size below). Read once here so a mid-run slider change can't reshape an in-flight run.
    // 1 reproduces the original strictly-sequential loop.
    const concurrency = Math.max(1, Math.min(MAX_SIMULATION_TPS, Math.round(simulationConfig.tps) || 1))

    // One full transaction (decide → score → optional smart retry). Returns the row to
    // append; throws on a backend error so the batch can tally it. `drawSuccess` mutates
    // a shared accumulator, but each call is synchronous (no await inside), so concurrent
    // tasks still update it atomically — error diffusion stays intact.
    const runTxn = async (i: number): Promise<SimulationResult> => {
      const paymentId = `sim_${Date.now()}_${i}`

      // Under SR_MULTI_OBJECTIVE, vary the cluster and amount per payment so
      // the (mock) cost lookup returns distinct costs and the
      // multi-objective leg has meaningful choices to make. Form values still
      // seed everything else (currency, eligible_gateways, etc).
      // Read live so a scenario switch made while paused takes effect on resume.
      const scenarioSel = multiObjScenarioRef.current
      const variant = isMultiObjective
        ? (scenarioSel === 'ALL'
            ? MULTI_OBJECTIVE_CLUSTER_VARIANTS[i % MULTI_OBJECTIVE_CLUSTER_VARIANTS.length]
            : MULTI_OBJECTIVE_CLUSTER_VARIANTS[scenarioSel])
        : null
      const paymentMethodType = isMultiObjective ? 'CARD' : form.payment_method_type
      const paymentMethod = variant ? variant.paymentMethod : form.payment_method
      const cardBrand = variant ? variant.cardSwitchProvider : form.card_brand
      const cardProgram = variant ? variant.cardProgram : form.card_program
      const cardIssuerCountry = variant ? variant.cardIssuerCountry : undefined
      const amtLo = Math.min(amountRangeRef.current.min, amountRangeRef.current.max)
      const amtHi = Math.max(amountRangeRef.current.min, amountRangeRef.current.max)
      const amount = isMultiObjective
        ? Math.floor(amtLo + Math.random() * (amtHi - amtLo + 1)) // configurable amount range
        : (parseFloat(form.amount) || 1000)

      const decideRes = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchantId: effectiveMerchantId,
        paymentInfo: {
          paymentId: paymentId,
          amount,
          currency: formRef.current.currency,
          paymentType: 'ORDER_PAYMENT',
          paymentMethodType,
          paymentMethod,
          authType: form.auth_type,
          cardBrand,
          cardSwitchProvider: cardBrand,
          cardType: paymentMethod,
          cardProgram,
          ...(cardIssuerCountry && { cardIssuerCountry }),
          channel: moChannel,
        },
        eligibleGatewayList: gateways,
        rankingAlgorithm: 'SR_BASED_ROUTING',
        // Cost savings is driven by the merchant Autopilot flag, not this request.
        eliminationEnabled: eliminationEnabled,
      })

      const decidedGateway = decideRes.decided_gateway

      const isSuccess = drawSuccess(decidedGateway)
      const failureMode = getGwFailureMode(decidedGateway)
      const outcome: TransactionOutcome = isSuccess ? 'CHARGED' : (failureMode === 'timeout' ? 'PENDING_VBV' : 'FAILURE')

      // Model ~50% of failures as soft declines (GSM `retry`) that can be retried on an
      // alternate processor; the rest are hard declines. Only meaningful when retry is on
      // and the decision actually has a fallback to route to.
      const retryable =
        smartRetryEnabledRef.current &&
        outcome === 'FAILURE' &&
        decideRes.fallback_gateways.length > 0 &&
        Math.random() < SIM_RETRYABLE_FAILURE_SHARE

      await apiPost<UpdateScoreResponse>('/update-gateway-score', {
        merchantId: effectiveMerchantId,
        gateway: decidedGateway,
        gatewayReferenceId: null,
        status: outcome,
        paymentId: paymentId,
        enforceDynamicRoutingFailure: null,
        ...(outcome === 'FAILURE' && { errorInfo: buildSimFailureErrorInfo(decidedGateway, retryable) }),
      })

      let retryGateway: string | undefined
      let retryStatus: TransactionOutcome | undefined

      if (retryable) {
        retryGateway = decideRes.fallback_gateways[0]
        const retrySuccess = drawSuccess(retryGateway)
        const retryFailureMode = getGwFailureMode(retryGateway)
        retryStatus = retrySuccess ? 'CHARGED' : (retryFailureMode === 'timeout' ? 'PENDING_VBV' : 'FAILURE')
        await apiPost('/update-gateway-score', {
          merchantId: effectiveMerchantId,
          gateway: retryGateway,
          gatewayReferenceId: null,
          status: retryStatus,
          paymentId: paymentId,
          enforceDynamicRoutingFailure: null,
          isSmartRetry: true,
          ...(retryStatus === 'FAILURE' && { errorInfo: buildSimFailureErrorInfo(retryGateway, false) }),
        })
      }

      const mo = decideRes.multi_objective_info ?? null
      return {
        paymentId,
        decidedGateway,
        status: outcome,
        timestamp: new Date().toISOString(),
        routingApproach: decideRes.routing_approach ?? null,
        gatewayPriorityMap: decideRes.gateway_priority_map ?? null,
        retryGateway,
        retryStatus,
        costSavedBps: mo?.costSavedBps ?? null,
        costWon: mo?.outcome === 'COST_WON',
        authWon: mo?.outcome === 'AUTH_WON',
        headAuthRate: mo?.srHead?.authRate ?? null,
        chosenAuthRate: mo?.chosen?.authRate ?? null,
        margin: mo?.margin ?? null,
        evGapTop2: mo?.evGapTop2 ?? null,
        amount,
        currency: formRef.current.currency,
        cardNetwork: cardBrand,
        cardProgram,
        cardIssuerRegion: cardIssuerCountry,
        cardScenario: variant?.label,
      }
    }

    try {
      // Sliding-window worker pool. The old loop fired a batch of `concurrency` txns and awaited
      // Promise.all — a barrier that blocked on the slowest request in the batch, so the chart
      // advanced in bursts of `concurrency` points separated by the batch's tail latency (the
      // hiccups seen at high TPS). Instead keep up to `concurrency` requests in flight at all
      // times and stream each result the moment it lands: completions (and the chart) flow
      // smoothly, and one slow request no longer stalls the other in-flight ones.
      let dispatched = resumeFrom
      let lastError: unknown = null

      // Commit the accumulated rows + refresh the events feed, throttled so a fast loop can't
      // spam re-renders (force=true bypasses the throttle for pause/end-of-run flushes).
      const flushResults = (force: boolean) => {
        const now = Date.now()
        if (force || now - lastUIUpdate > 150) {
          setSimulationResults([...results])
          markExplorerRunDataUpdated()
          lastUIUpdate = now
        }
        if (force || now - lastEventsRefresh > 250) {
          routingEvents.refresh()
          lastEventsRefresh = now
        }
      }

      const worker = async (): Promise<void> => {
        while (true) {
          if (simulationAbortRef.current) return
          // Idle here while paused — without claiming new work — so the in-flight requests drain
          // and the resume index stays put; a Stop still breaks out. Flush once on the way in so a
          // page-leave/return resumes from exactly the committed rows.
          if (simulationPausedRef.current) {
            runProgressRef.current = results.length
            flushResults(true)
            while (simulationPausedRef.current && !simulationAbortRef.current) {
              await new Promise(resolve => setTimeout(resolve, 120))
            }
            continue
          }

          const i = dispatched
          if (i >= total) return
          dispatched++

          try {
            const row = await runTxn(i)
            results.push(row)
            consecutiveErrors = 0
          } catch (e) {
            lastError = e
            // Repeated failures with no success between them means the backend is down: bail and
            // signal the other workers. A single failure amid successes just resets the counter.
            consecutiveErrors++
            if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
              simulationAbortRef.current = true
              return
            }
            await new Promise(resolve => setTimeout(resolve, 300))
          }

          // Record the resume point as the committed count and flush on the shared throttle.
          runProgressRef.current = results.length
          flushResults(false)
        }
      }

      // Fan out `concurrency` workers that each pull the next txn index off `dispatched` until the
      // run is exhausted (single-threaded, so no two claim the same index).
      await Promise.all(Array.from({ length: concurrency }, () => worker()))

      // If a worker tripped the error threshold (as opposed to a user Stop), surface the failure.
      if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
        handleRunError(lastError, 'batch', `Simulation stopped after ${MAX_CONSECUTIVE_ERRORS} consecutive errors. Check that the server is running.`)
        return
      }
    } finally {
      setSimulationResults([...results])
      setIsSimulating(false)
      setIsPaused(false)
      simulationPausedRef.current = false
      // Final flush: events from the last txns can land just after the loop ends.
      routingEvents.refresh()
    }
  }

  function pauseSimulation() {
    simulationPausedRef.current = true
    setIsPaused(true)
  }

  function resumeSimulation() {
    simulationPausedRef.current = false
    setIsPaused(false)
  }

  async function runRuleEvaluation() {
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')
    setLoading(true)
    setError(null)
    setSetupPrompt(null)
    setRuleResult(null)
    setVolumeDistribution([])
    setVolumeEvaluationLog([])
    setVolumeProgress(0)
    const previewPaymentId = `rule_decision_${Date.now()}`

    try {
      const parameters: Record<string, { type: string; value: string | number | { key: string; value: string } }> = {}
      ruleParams.forEach(p => {
        if (p.key) {
          if (p.type === 'metadata_variant') {
            parameters[p.key] = {
              type: p.type,
              value: { key: p.metadataKey || p.key, value: p.value }
            }
          } else if (p.type === 'number') {
            parameters[p.key] = { type: p.type, value: parseFloat(p.value) || 0 }
          } else if (p.value !== '') {
            parameters[p.key] = { type: p.type, value: p.value }
          }
        }
      })

      const res = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
        created_by: effectiveMerchantId || 'test_user',
        payment_id: previewPaymentId,
        fallback_output: fallbackConnectors.filter(c => c.gateway_name),
        parameters,
      })

      setRuleResult(res)
      markExplorerRunDataUpdated()

      if (res.output.type === 'volume_split' && res.output.splits) {
        const totalPayments = parseInt(volumePayments) || 100
        const distribution = res.output.splits.map(item => ({
          name: item.connector.gateway_name,
          count: Math.round((item.split / 100) * totalPayments),
          percentage: item.split,
        }))
        setVolumeDistribution(distribution)
      }
    } catch (e: unknown) {
      handleRunError(e, 'rule')
    } finally {
      setLoading(false)
    }
  }

  async function runVolumeSplit() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    setLoading(true)
    setError(null)
    setSetupPrompt(null)
    setRuleResult(null)
    setVolumeDistribution([])
    setVolumeEvaluationLog([])
    setVolumeProgress(0)
    const totalPayments = parseInt(volumePayments) || 0

    if (totalPayments <= 0) {
      setLoading(false)
      return setError('Total Payments must be greater than 0')
    }

    try {
      const batchSize = 10
      const basePaymentId = `volume_decision_${Date.now()}`
      const logEntries: VolumePaymentEntry[] = []
      const counts = new Map<string, number>()
      let firstDecision: RuleEvaluateResponse | null = null
      const buildDistribution = (completedPayments: number) =>
        Array.from(counts.entries())
          .map(([name, count]) => ({
            name,
            count,
            percentage: Number(((count / Math.max(1, completedPayments)) * 100).toFixed(1)),
          }))
          .sort((left, right) => right.count - left.count)

      for (let start = 0; start < totalPayments; start += batchSize) {
        const chunkSize = Math.min(batchSize, totalPayments - start)
        const chunkResponses = await Promise.all(
          Array.from({ length: chunkSize }, async (_, offset) => {
            const index = start + offset
            const paymentId = `${basePaymentId}_${index}`
            const response = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
              created_by: effectiveMerchantId,
              payment_id: paymentId,
              fallback_output: fallbackConnectors.filter(c => c.gateway_name),
              parameters: {},
            })

            return { paymentId, response }
          }),
        )

        for (const { paymentId, response } of chunkResponses) {
          if (response.output.type !== 'volume_split') {
            throw new Error('Active routing algorithm is not a volume split rule.')
          }

          const connector = extractVolumeConnector(response)
          if (!connector) {
            throw new Error('Volume split evaluation did not return a connector.')
          }

          if (!firstDecision) {
            firstDecision = response
            setRuleResult(response)
          }

          counts.set(connector, (counts.get(connector) || 0) + 1)
          logEntries.push({ paymentId, connector })
        }

        setVolumeProgress(logEntries.length)
        setVolumeEvaluationLog([...logEntries])
        setVolumeDistribution(buildDistribution(logEntries.length))
        markExplorerRunDataUpdated()
      }
    } catch (e: unknown) {
      handleRunError(e, 'volume')
    } finally {
      setLoading(false)
    }
  }

  const scoreData = result?.gateway_priority_map
    ? Object.entries(result.gateway_priority_map)
      .sort(([, a], [, b]) => b - a)
      .map(([name, score]) => ({ name, score: Math.round(score * 10000) / 100 }))
    : []

  const totalSimulationPayments = parseInt(simulationConfig.totalPayments) || 0
  const completedSimulationCount = simulationResults.length
  const hasSimulationActivity = isSimulating || completedSimulationCount > 0

  const eligibleGatewaysParsed = useMemo(
    () => form.eligible_gateways.split(',').map(s => s.trim().toLowerCase()).filter(Boolean),
    [form.eligible_gateways],
  )

  const gatewayColorMap = useMemo(
    () => Object.fromEntries(eligibleGatewaysParsed.map((gw, i) => [gw, GW_COLOR_OVERRIDES[gw.toLowerCase()] ?? GW_PALETTE[i % GW_PALETTE.length]])),
    [eligibleGatewaysParsed],
  )

  // Auto-populate errorInfo for any gateway whose config has no error code yet,
  // once GSM rules have loaded from the API.
  useEffect(() => {
    if (gsmRules.length === 0 || eligibleGatewaysParsed.length === 0) return
    setGatewaySimConfigs(prev => {
      let changed = false
      const next = { ...prev }
      for (const gw of eligibleGatewaysParsed) {
        if (prev[gw]?.errorInfo?.error_code) continue
        const config = prev[gw] ?? defaultGwSimConfig(gw)
        const resolved = resolveSimErrorInfo(gw, config.gsmDecision, config.penalized)
        if (resolved) {
          next[gw] = { ...config, errorInfo: { error_code: resolved.errorCode, error_message: resolved.errorMessage, issuer_error_code: '', card_network: '' } }
          changed = true
        }
      }
      return changed ? next : prev
    })
  }, [gsmRules, eligibleGatewaysParsed])

  const gatewayStats = useMemo(() => deferredSimulationResults.reduce((acc, curr) => {
    if (!acc[curr.decidedGateway]) {
      acc[curr.decidedGateway] = { total: 0, success: 0, failure: 0 }
    }
    acc[curr.decidedGateway].total++
    if (curr.status === 'CHARGED') acc[curr.decidedGateway].success++
    else acc[curr.decidedGateway].failure++
    return acc
  }, {} as Record<string, { total: number; success: number; failure: number }>), [deferredSimulationResults])

  // Per-gateway engine SR-score trend. Each line plots the routing engine's
  // priority score for that gateway (gateway_priority_map), i.e. the same value
  // shown in the Transaction Log "SR Score" column — not the realized outcome
  // rate. This is the score the engine actually routes on, so the Top-PSP line
  // and the cost-eligible band below reflect the real Auth-vs-Cost decisions.
  // A normal SR decision emits a full map of every eligible gateway's score;
  // hedging decisions emit only an exploratory {decidedGateway: 1.0} entry, so
  // we skip those to avoid 100% spikes and carry forward each gateway's last
  // real score instead.
  const gatewaySparklines = useMemo(() => {
    const results = deferredSimulationResults
    const empty = { series: {} as Record<string, (number | null)[]>, evSeries: [] as (number | null)[], decidedSeries: [] as (string | null)[], paymentNums: [] as number[], yMin: 0, yMax: 100 }
    if (results.length < 2) return empty

    // Focus on the last `chartWindow` transactions (or the whole run).
    const windowStart = chartWindow === 'all' ? 0 : Math.max(0, results.length - chartWindow)
    const windowLen = results.length - windowStart
    // One point per transaction in the selected window — no downsampling. Target the window
    // size itself ("All" ⇒ every transaction in the run), so `bucketSize` stays 1 and odd
    // payment numbers are plotted too rather than being skipped by the old 80-point cap.
    const TARGET_POINTS = chartWindow === 'all' ? Math.max(1, windowLen) : chartWindow
    const bucketSize = Math.max(1, Math.ceil(windowLen / TARGET_POINTS))
    const gateways = Object.keys(gatewayStats)
    // Engine score (0–100) carried forward *per scenario* per gateway. With "All scenarios
    // (rotate)" each transaction reports only its own segment's score, so plotting the raw
    // per-txn value hops between the rotating buckets and the line wobbles with the rotation
    // period. Instead we keep the latest score for each scenario and plot the mean across the
    // scenarios seen so far — a smooth cross-segment SR. In single-scenario (or non-multi-
    // objective) runs only one bucket is ever populated, so the mean equals that scenario's
    // score and the trend is unchanged.
    const lastScoreByScenario: Record<string, Record<string, number>> = {}
    gateways.forEach(gw => { lastScoreByScenario[gw] = {} })
    const avgScore = (gw: string): number | null => {
      const buckets = lastScoreByScenario[gw]
      let sum = 0, n = 0
      for (const k in buckets) { sum += buckets[k]; n++ }
      return n > 0 ? sum / n : null
    }
    // Winning PSP's EV lead over the runner-up (`EV(#1) − EV(#2)`, a fraction of ticket),
    // carried forward per scenario exactly like the score: a transient null (pure-SR decision
    // or <2 cost-ranked PSPs) doesn't wipe the last good value, and rotate mode plots the mean
    // across scenarios so it stays smooth. Null until any scenario reports a real gap.
    const lastEvByScenario: Record<string, number> = {}
    const avgEv = (): number | null => {
      let sum = 0, n = 0
      for (const k in lastEvByScenario) { sum += lastEvByScenario[k]; n++ }
      return n > 0 ? sum / n : null
    }
    const series: Record<string, (number | null)[]> = {}
    gateways.forEach(gw => { series[gw] = [] })
    const evSeries: (number | null)[] = []
    // Gateway the engine actually *selected* at each point. Under cost-based / multi-objective
    // routing the winner is the highest-EV PSP, which is NOT necessarily the SR leader — so the
    // tooltip must star this, not `max(SR)`. Carried forward on non-hedging decisions (a hedge
    // routes to an exploratory PSP, not the real pick).
    const decidedSeries: (string | null)[] = []
    let lastDecided: string | null = null
    const paymentNums: number[] = []

    for (let i = 0; i < results.length; i++) {
      const r = results[i]
      const pm = r.gatewayPriorityMap
      // Bucket by the scenario this decision priced; non-rotating runs share one key.
      const scenarioKey = r.cardScenario ?? '__single__'
      // Refresh this scenario's score from the decision's priority map, skipping
      // hedging (single-entry exploratory map at 1.0) so the band stays meaningful.
      if (pm && !r.routingApproach?.includes('HEDGING')) {
        for (const gw of gateways) {
          const s = pm[gw]
          // Keep 2 decimals so decimal SRs (e.g. 98.45 vs 97.05) stay distinguishable on the trend.
          if (typeof s === 'number') lastScoreByScenario[gw][scenarioKey] = Math.round(s * 10000) / 100
        }
        // Store the raw fraction (formatted to % in the tooltip) so small gaps keep precision.
        if (typeof r.evGapTop2 === 'number') lastEvByScenario[scenarioKey] = r.evGapTop2
        // The EV winner (evGapTop2's #1) is exactly this decided gateway.
        if (r.decidedGateway) lastDecided = r.decidedGateway
      }
      // Transactions before the window only seed the carried-forward scores so the
      // left edge opens at the correct value; they aren't plotted.
      if (i < windowStart) continue
      const posInWindow = i - windowStart
      // Sample every gateway's mean engine score at the bucket boundary. A gateway
      // with no score yet records null (skipped by the chart) rather than a 0 dip.
      if ((posInWindow + 1) % bucketSize === 0 || i === results.length - 1) {
        paymentNums.push(i + 1)
        for (const gw of gateways) series[gw].push(avgScore(gw))
        evSeries.push(avgEv())
        decidedSeries.push(lastDecided)
      }
    }

    // Compute y-axis range here to avoid flatMap+spread in render
    let obsMin = 100, obsMax = 0
    for (const gw of gateways) {
      for (const v of series[gw]) {
        if (v != null && v > 0) { if (v < obsMin) obsMin = v; if (v > obsMax) obsMax = v }
      }
    }
    if (obsMin > obsMax) { obsMin = 0; obsMax = 100 }
    const yMin = Math.max(0, obsMin - 2)
    const yMax = Math.min(100, obsMax + 2)

    return { series, evSeries, decidedSeries, paymentNums, yMin, yMax }
  }, [deferredSimulationResults, gatewayStats, chartWindow])

  // Rolling routing-share trend, paired with the SR chart on the same x-axis.
  // For each bucket it reports the share of the last ≤WINDOW transactions that
  // went to each gateway, normalized to sum to 100% per point. Drawn as a 100%
  // stacked area so the split is always full height — you read the live traffic
  // mix directly and watch a connector's band grow or shrink as routing shifts.
  const gatewayVolumeTrend = useMemo(() => {
    const results = deferredSimulationResults
    const gateways = Object.keys(gatewayStats)
    const empty = { data: [] as Record<string, number>[], gateways, windowSize: 1 }
    if (results.length < 2) return empty

    const ROLLING_WINDOW = 50
    // Match the SR chart's sampling (one point per transaction in the window) so both x-axes
    // stay aligned.
    const windowStart = chartWindow === 'all' ? 0 : Math.max(0, results.length - chartWindow)
    const windowLen = results.length - windowStart
    const TARGET_POINTS = chartWindow === 'all' ? Math.max(1, windowLen) : chartWindow
    const bucketSize = Math.max(1, Math.ceil(windowLen / TARGET_POINTS))
    const recent: string[] = []  // decided gateway of the last ≤WINDOW transactions
    const data: Record<string, number>[] = []

    for (let i = 0; i < results.length; i++) {
      // Keep rolling the share window across all history so the left edge of the
      // visible window still has full ≤ROLLING_WINDOW lookback, but only emit
      // points inside the window.
      recent.push(results[i].decidedGateway)
      if (recent.length > ROLLING_WINDOW) recent.shift()
      if (i < windowStart) continue
      const posInWindow = i - windowStart
      if ((posInWindow + 1) % bucketSize === 0 || i === results.length - 1) {
        const counts: Record<string, number> = {}
        for (const gw of recent) counts[gw] = (counts[gw] ?? 0) + 1
        const total = recent.length || 1
        const row: Record<string, number> = { step: i + 1 }
        for (const gw of gateways) row[gw] = Math.round(((counts[gw] ?? 0) / total) * 1000) / 10
        data.push(row)
      }
    }
    return { data, gateways, windowSize: ROLLING_WINDOW }
  }, [deferredSimulationResults, gatewayStats, chartWindow])

  // Human-readable routing label, shared by the Transaction Log cell and its filter.
  const routingApproachLabel = (approach?: string | null): string =>
    approach?.includes('HEDGING')
      ? 'Hedging'
      : approach === 'SR_SELECTION_MULTI_OBJECTIVE'
        ? 'Cost Based'
        : approach === 'SR_SELECTION_V3_ROUTING'
          ? 'Auth Based'
          : approach ?? '—'

  // Distinct values that populate the categorical Transaction Log column filters.
  const txFilterOptions = useMemo(() => {
    const gateways = new Set<string>()
    const networks = new Set<string>()
    const programs = new Set<string>()
    const routings = new Set<string>()
    const outcomes = new Set<string>()
    const retryGateways = new Set<string>()
    const retryOutcomes = new Set<string>()
    for (const res of deferredSimulationResults) {
      gateways.add(res.decidedGateway)
      if (res.cardNetwork) networks.add(res.cardNetwork)
      if (res.cardProgram) programs.add(res.cardProgram)
      routings.add(routingApproachLabel(res.routingApproach))
      if (res.status) outcomes.add(res.status)
      if (res.retryGateway) retryGateways.add(res.retryGateway)
      if (res.retryStatus) retryOutcomes.add(res.retryStatus)
    }
    const sorted = (s: Set<string>) => Array.from(s).sort()
    return {
      gateways: sorted(gateways),
      networks: sorted(networks),
      programs: sorted(programs),
      routings: sorted(routings),
      outcomes: sorted(outcomes),
      retryGateways: sorted(retryGateways),
      retryOutcomes: sorted(retryOutcomes),
    }
  }, [deferredSimulationResults])

  // Transaction Log rows after applying the column filters, keeping each row's
  // original index so the "#" column stays stable regardless of filtering.
  const txFilteredRows = useMemo(() => {
    const f = txFilters
    const active = Object.values(f).some(Boolean)
    const rows = deferredSimulationResults.map((res, idx) => ({ res, idx }))
    if (!active) return rows
    const srText = (res: SimulationResult) => {
      const s = res.gatewayPriorityMap?.[res.decidedGateway]
      return typeof s === 'number' ? (s * 100).toFixed(1) : ''
    }
    return rows.filter(({ res }) => {
      if (f.gateway && res.decidedGateway !== f.gateway) return false
      if (f.network && (res.cardNetwork ?? '') !== f.network) return false
      if (f.program && (res.cardProgram ?? '') !== f.program) return false
      if (f.routing && routingApproachLabel(res.routingApproach) !== f.routing) return false
      if (f.outcome && res.status !== f.outcome) return false
      if (f.retryGateway && (res.retryGateway ?? '') !== f.retryGateway) return false
      if (f.retryOutcome && (res.retryStatus ?? '') !== f.retryOutcome) return false
      if (f.amount && !formatCurrencyValue(res.amount, res.currency).toLowerCase().includes(f.amount.toLowerCase())) return false
      if (f.sr && !srText(res).includes(f.sr)) return false
      if (f.evGap && !(res.evGapTop2 != null ? (res.evGapTop2 * 100).toFixed(2) : '').includes(f.evGap)) return false
      if (f.cost) {
        const hasSavings = !!res.costWon && res.costSavedBps != null && res.costSavedBps > 0 && res.status === 'CHARGED'
        if (f.cost === 'yes' && !hasSavings) return false
        if (f.cost === 'no' && hasSavings) return false
      }
      return true
    })
  }, [deferredSimulationResults, txFilters])

  // Column totals over the currently filtered rows, for the Transaction Log footer.
  // Only the numeric columns are summable: Amount (all rows) and Cost Savings (realized —
  // same condition as the per-row cell: a charged cost-override with positive savings).
  const txColumnTotals = useMemo(() => {
    let amount = 0
    let savings = 0
    // Average the EV margin-of-victory (% of ticket) over rows that actually ranked
    // on EV — a mean is more meaningful than a sum for a per-decision spread metric.
    let evGapPctSum = 0
    let evGapCount = 0
    for (const { res } of txFilteredRows) {
      amount += res.amount
      if (res.costWon && res.costSavedBps != null && res.costSavedBps > 0 && res.status === 'CHARGED') {
        savings += (res.costSavedBps / 10000) * res.amount
      }
      if (res.evGapTop2 != null) {
        evGapPctSum += res.evGapTop2 * 100
        evGapCount += 1
      }
    }
    const currency = txFilteredRows[0]?.res.currency || form.currency || 'USD'
    const evGapPctAvg = evGapCount > 0 ? evGapPctSum / evGapCount : null
    return { amount, savings, currency, count: txFilteredRows.length, evGapPctAvg }
  }, [txFilteredRows, form.currency])

  const txFiltersActive = Object.values(txFilters).some(Boolean)

  const hedgingHits = useMemo(
    () => deferredSimulationResults.filter(r => r.routingApproach?.includes('HEDGING')).length,
    [deferredSimulationResults],
  )


  const totalCostSaved = useMemo(() => {
    let value = 0
    let currency = ''
    for (const r of deferredSimulationResults) {
      if (r.costWon && r.costSavedBps != null && r.costSavedBps > 0 && r.status === 'CHARGED') {
        value += (r.costSavedBps / 10000) * r.amount
        currency = r.currency
      }
    }
    return { value, currency }
  }, [deferredSimulationResults])

  // Honest economics of cost Estimation. The gross fee saved (totalCostSaved) is only the
  // upside; an override also accepts a small auth-rate risk. We value that risk the way
  // the band does — the *expected* sale value given up, (headAuthRate − chosenAuthRate)
  // × amount × margin — and net it. This is counterfactual-free: it never books a single
  // failed override as a full lost sale (the SR head would have failed a share of those
  // too), it books only the auth-rate delta the override knowingly traded. The realized
  // line is a separate, observed sanity check: did overridden txns actually charge at a
  // similar rate to auth-kept txns?
  const costEconomics = useMemo(() => {
    let feeSaved = 0
    let authValueRisked = 0
    let overrideCharged = 0
    let overrideTotal = 0
    let authKeptCharged = 0
    let authKeptTotal = 0
    let currency = ''
    for (const r of deferredSimulationResults) {
      if (r.currency) currency = r.currency
      if (r.costWon) {
        overrideTotal++
        if (r.status === 'CHARGED') {
          overrideCharged++
          if (r.costSavedBps != null && r.costSavedBps > 0) {
            feeSaved += (r.costSavedBps / 10000) * r.amount
          }
        }
        // Expected auth value the override traded away (head vs chosen auth gap),
        // valued at the merchant's margin. Only meaningful when we captured both.
        if (r.headAuthRate != null && r.chosenAuthRate != null && r.margin != null) {
          const authGap = Math.max(0, r.headAuthRate - r.chosenAuthRate)
          authValueRisked += authGap * r.amount * r.margin
        }
      } else if (r.authWon) {
        authKeptTotal++
        if (r.status === 'CHARGED') authKeptCharged++
      }
    }
    const overrideSr = overrideTotal > 0 ? overrideCharged / overrideTotal : null
    const authKeptSr = authKeptTotal > 0 ? authKeptCharged / authKeptTotal : null
    return {
      currency,
      feeSaved,
      authValueRisked,
      netProfit: feeSaved - authValueRisked,
      overrideSr,
      authKeptSr,
      overrideTotal,
      authKeptTotal,
    }
  }, [deferredSimulationResults])

  // Multi-objective outcome counts: how often the auth objective vs the cost
  // objective won the routing decision across the run.
  const multiObjectiveStats = useMemo(() => {
    let costWon = 0
    let costSuccess = 0
    let costFailure = 0
    let srSuccess = 0
    let srFailure = 0
    let total = 0
    // Total Payment Volume across every decision (first-attempt amount, retries don't
    // re-charge the principal), plus the currency to format it in.
    let tpv = 0
    let currency = ''
    for (const r of deferredSimulationResults) {
      total++
      tpv += r.amount
      if (r.currency) currency = r.currency
      if (r.costWon) {
        // Cost override (cost beat the SR head).
        costWon++
        if (r.status === 'CHARGED') costSuccess++
        else costFailure++
      } else {
        // Everything else is SR-based — auth-won AND hedged decisions.
        if (r.status === 'CHARGED') srSuccess++
        else srFailure++
      }
    }
    // Total = SR-based + cost-based by construction, and matches the Gateway Summary total.
    const srBased = total - costWon
    return { srBased, srSuccess, srFailure, costWon, costSuccess, costFailure, total, tpv, currency }
  }, [deferredSimulationResults])

  // Auth-rate view of the run. Each row is one decision: `status` is the first-attempt
  // outcome and `retryStatus` is the smart-retry outcome (only set when a soft decline was
  // retried on an alternate PSP). FAAR credits only first-attempt charges; NAR credits the
  // final outcome (first attempt OR a successful retry), so NAR ≥ FAAR whenever retry helps.
  const authRateStats = useMemo(() => {
    let total = 0
    let firstAttemptSuccess = 0
    let finalSuccess = 0
    for (const r of deferredSimulationResults) {
      total++
      const firstOk = r.status === 'CHARGED'
      if (firstOk) firstAttemptSuccess++
      if (firstOk || r.retryStatus === 'CHARGED') finalSuccess++
    }
    return {
      total,
      firstAttemptSuccess,
      finalSuccess,
      faar: total > 0 ? firstAttemptSuccess / total : null,
      nar: total > 0 ? finalSuccess / total : null,
    }
  }, [deferredSimulationResults])

  const debitNetworkRows = debitResult?.debit_routing_output?.co_badged_card_networks_info || []
  const volumeColorIndex = useMemo(
    () => new Map(volumeDistribution.map((item, index) => [item.name, index] as const)),
    [volumeDistribution],
  )
  const sortedGatewayStats = useMemo(
    () => Object.entries(gatewayStats).sort((a, b) => b[1].total - a[1].total),
    [gatewayStats],
  )
  const sortedVolumeDistribution = useMemo(
    () => [...volumeDistribution].sort((a, b) => b.count - a.count),
    [volumeDistribution],
  )
  const volumeLeader = sortedVolumeDistribution[0]
  const volumeEvaluationCount = volumeEvaluationLog.length
  const volumeRunTarget = Number.parseInt(volumePayments, 10) || 0
  const volumeProgressPercentage =
    volumeRunTarget > 0 ? Math.min(100, Math.round((volumeProgress / volumeRunTarget) * 100)) : 0

  const auditSummary = useMemo(() => {
    const results = auditDetail.data?.results || []
    return results.find((row) => row.payment_id === selectedAuditPaymentId) || results[0] || null
  }, [auditDetail.data?.results, selectedAuditPaymentId])

  const selectedAuditEvent = useMemo(() => {
    const timeline = auditDetail.data?.timeline || []
    return timeline.find((event) => event.id === selectedAuditEventId) || timeline[0] || null
  }, [auditDetail.data?.timeline, selectedAuditEventId])

  useEffect(() => {
    if (selectedAuditEvent?.id) {
      setSelectedAuditEventId(selectedAuditEvent.id)
      return
    }
    const first = auditDetail.data?.timeline?.[0]
    if (first?.id) {
      setSelectedAuditEventId(first.id)
    }
  }, [auditDetail.data?.timeline, selectedAuditEvent?.id])

  const groupedAuditTimeline = useMemo(() => {
    const groups: Array<{ phase: string; events: PaymentAuditEvent[] }> = []
    for (const event of auditDetail.data?.timeline || []) {
      const phase = eventPhase(event)
      const current = groups[groups.length - 1]
      if (!current || current.phase !== phase) {
        groups.push({ phase, events: [event] })
      } else {
        current.events.push(event)
      }
    }
    return groups
  }, [auditDetail.data?.timeline])

  const auditInspectorModel = useMemo(() => buildInspectorModel(selectedAuditEvent), [selectedAuditEvent])

  const previewSummary = useMemo(() => {
    const results = previewTraceDetail.data?.results || []
    return results.find((row) => row.payment_id === selectedPreviewPaymentId) || results[0] || null
  }, [previewTraceDetail.data?.results, selectedPreviewPaymentId])

  const selectedPreviewEvent = useMemo(() => {
    const timeline = previewTraceDetail.data?.timeline || []
    return timeline.find((event) => event.id === selectedPreviewEventId) || timeline[0] || null
  }, [previewTraceDetail.data?.timeline, selectedPreviewEventId])

  useEffect(() => {
    if (selectedPreviewEvent?.id) {
      setSelectedPreviewEventId(selectedPreviewEvent.id)
      return
    }
    const first = previewTraceDetail.data?.timeline?.[0]
    if (first?.id) {
      setSelectedPreviewEventId(first.id)
    }
  }, [previewTraceDetail.data?.timeline, selectedPreviewEvent?.id])

  const groupedPreviewTimeline = useMemo(() => {
    const groups: Array<{ phase: string; events: PaymentAuditEvent[] }> = []
    for (const event of previewTraceDetail.data?.timeline || []) {
      const phase = eventPhase(event)
      const current = groups[groups.length - 1]
      if (!current || current.phase !== phase) {
        groups.push({ phase, events: [event] })
      } else {
        current.events.push(event)
      }
    }
    return groups
  }, [previewTraceDetail.data?.timeline])

  const previewInspectorModel = useMemo(() => buildInspectorModel(selectedPreviewEvent), [selectedPreviewEvent])


  useEffect(() => {
    const el = txLogRef.current
    if (!el) return
    el.scrollTop = el.scrollHeight
  }, [deferredSimulationResults.length])


  function openAuditModal(paymentId: string) {
    setSelectedPreviewPaymentId(null)
    setSelectedPreviewEventId(null)
    setPreviewInspectorTab('summary')
    setSelectedAuditPaymentId(paymentId)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
  }

  function closeAuditModal() {
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
  }

  function openPreviewModal(paymentId: string, label: string) {
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
    setPreviewTraceLabel(label)
    setSelectedPreviewPaymentId(paymentId)
    setSelectedPreviewEventId(null)
    setPreviewInspectorTab('summary')
  }

  function closePreviewModal() {
    setSelectedPreviewPaymentId(null)
    setSelectedPreviewEventId(null)
    setPreviewInspectorTab('summary')
  }

  function resetCurrentTabState() {
    const defaults = getDefaultExplorerState()

    // Reset form fields to the first available option from routing config so
    // required fields like currency are never sent as empty strings.
    function populatedForm(base: FormState): FormState {
      const next = { ...base }
      if (currencyOptions.length > 0 && !currencyOptions.includes(next.currency))
        next.currency = currencyOptions[0]
      if (paymentMethodTypeOptions.length > 0 && !paymentMethodTypeOptions.includes(next.payment_method_type))
        next.payment_method_type = paymentMethodTypeOptions[0]
      const methodsForType = toUpperOptions(routingKeysConfig[next.payment_method_type.toLowerCase()]?.values || [])
      if (methodsForType.length > 0 && !methodsForType.includes(next.payment_method))
        next.payment_method = methodsForType[0]
      if (authTypeOptions.length > 0 && !authTypeOptions.includes(next.auth_type))
        next.auth_type = authTypeOptions[0]
      if (cardBrandOptions.length > 0 && !cardBrandOptions.includes(next.card_brand))
        next.card_brand = cardBrandOptions[0]
      return next
    }

    if (activeTab === 'single') {
      setForm(populatedForm(defaults.form))
      setResult(defaults.result)
      setSingleRunPaymentId(defaults.singleRunPaymentId)
      setSingleRunOutcome(defaults.singleRunOutcome)
      setResponseOpen(defaults.responseOpen)
    } else if (activeTab === 'batch') {
      setForm({ ...populatedForm(defaults.form), currency: MULTI_OBJECTIVE_CURRENCY })
      setMoCurrency(MULTI_OBJECTIVE_CURRENCY)
      setSimulationConfig(defaults.simulationConfig)
      setMultiObjScenario(defaults.multiObjScenario)
      // Preserve the currently selected per-gateway success rate scores (e.g.
      // Stripe/Adyen) across a reset — only clear the other sim config fields.
      setGatewaySimConfigs(prev =>
        Object.fromEntries(
          Object.entries(prev).map(([gw, cfg]) => [
            gw,
            { ...DEFAULT_GW_SIM_CONFIG, successRate: cfg.successRate },
          ]),
        ),
      )
      setSimulationResults(defaults.simulationResults)
      setResumableRun(defaults.resumableRun)
      // Abort any in-flight run and drop the run-start timestamp so the
      // Autopilot Actions feed (scoped to simulationStartedAtMs) clears back
      // to its empty state instead of replaying the previous run's events.
      simulationAbortRef.current = true
      setSimulationStartedAtMs(null)
      setIsSimulating(false)
    } else if (activeTab === 'rule') {
      setRuleResetSignal(n => n + 1)
    } else if (activeTab === 'volume') {
      setVolumePayments(defaults.volumePayments)
      setRuleResult(defaults.ruleResult)
      setVolumeDistribution(defaults.volumeDistribution)
      setVolumeEvaluationLog(defaults.volumeEvaluationLog)
      setVolumeProgress(defaults.volumeProgress)
      setVolumeResponseOpen(defaults.volumeResponseOpen)
      setSelectedPreviewPaymentId(null)
      setSelectedPreviewEventId(null)
      setPreviewInspectorTab('summary')
      setPreviewTraceLabel('Volume Split Decision')
    } else if (activeTab === 'debit') {
      setDebitForm(defaults.debitForm)
      setDebitResult(defaults.debitResult)
      setDebitPaymentId(defaults.debitPaymentId)
      setDebitResponseOpen(defaults.debitResponseOpen)
      setSelectedAuditPaymentId(null)
      setSelectedAuditEventId(null)
      setAuditInspectorTab('summary')
    }

    setError(null)
    setSetupPrompt(null)
    setLoading(false)
    setFilterOpen(false)
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
  }

  const resetButtonLabel =
    activeTab === 'batch'
      ? 'Reset'
      : activeTab === 'rule'
        ? 'Reset Rule Based Routing'
        : activeTab === 'volume'
          ? 'Reset Volume Based Routing'
          : 'Reset Debit Routing'

  return (
    <div className="mx-auto max-w-[1500px] space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="mt-2 flex flex-wrap items-center gap-3">
            <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">Decision Simulator</h1>
          </div>
        </div>
        {/* Deliberately a small, muted icon (not a full button) — hard refresh flushes this
            merchant's SR scores and is a rare escape hatch, so it's kept low-profile. Clicking
            opens a confirmation popup before anything is flushed. */}
        <button
          type="button"
          onClick={() => setHardRefreshConfirmOpen(true)}
          disabled={isSimulating || !effectiveMerchantId || isHardRefreshing}
          title="Hard refresh — flush this merchant's SR scores from Redis so the next run starts fresh. Use sparingly."
          aria-label="Hard refresh gateway scores"
          className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600 disabled:pointer-events-none disabled:opacity-40 dark:text-slate-500 dark:hover:bg-white/5 dark:hover:text-slate-300"
        >
          <RefreshCw size={14} className={isHardRefreshing ? 'animate-spin' : ''} />
        </button>
      </div>

      <ConfirmDialog
        open={hardRefreshConfirmOpen}
        variant="danger"
        title="Hard refresh SR scores?"
        description="This flushes all of this merchant's accumulated SR scores from Redis. The next run starts from fresh scores — there's no undo."
        confirmLabel="Flush scores"
        cancelLabel="Cancel"
        onConfirm={() => { setHardRefreshConfirmOpen(false); void hardRefreshScores() }}
        onCancel={() => setHardRefreshConfirmOpen(false)}
      />

      {addConnectorOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div
            className="absolute inset-0 bg-black/40 backdrop-blur-[2px]"
            onClick={() => setAddConnectorOpen(false)}
          />
          <div className="relative w-full max-w-sm rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl outline-none dark:border-[#2a303a] dark:bg-[#0d1118]">
            <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Add a processor</h3>
            <p className="mt-2 text-sm leading-relaxed text-slate-500 dark:text-[#8a93a6]">
              Enter a connector name (e.g. <span className="font-medium">braintree</span>,{' '}
              <span className="font-medium">worldpay</span>). It joins the comparison with its own SR
              slider — scored on its ingested cost model if it has one, otherwise seed costs.
            </p>
            <input
              autoFocus
              type="text"
              value={newConnectorName}
              placeholder="Connector name"
              onChange={e => {
                setNewConnectorName(e.target.value)
                if (addConnectorError) setAddConnectorError(null)
              }}
              onKeyDown={e => {
                if (e.key === 'Enter') submitAddConnector()
                else if (e.key === 'Escape') setAddConnectorOpen(false)
              }}
              className="mt-4 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
            />
            {addConnectorError && <p className="mt-2 text-xs text-red-500">{addConnectorError}</p>}
            <div className="mt-6 flex justify-end gap-2">
              <Button variant="secondary" size="sm" onClick={() => setAddConnectorOpen(false)}>
                Cancel
              </Button>
              <Button variant="primary" size="sm" onClick={submitAddConnector}>
                Add processor
              </Button>
            </div>
          </div>
        </div>
      )}

      {activeTab === 'rule' && (
        <RuleEvaluationPanel
          merchantId={effectiveMerchantId}
          routingKeysConfig={routingKeysConfig}
          routingConfigUnavailable={routingConfigUnavailable}
          routingKeysLoading={routingKeysLoading}
          resetSignal={ruleResetSignal}
          onRunComplete={markExplorerRunDataUpdated}
          onOpenTrace={openPreviewModal}
        />
      )}

      {activeTab === 'batch' && (
        <div className="flex flex-wrap items-start gap-x-6 gap-y-4 rounded-2xl border border-slate-200 bg-white px-5 py-3 dark:border-[#222226] dark:bg-[#0b0b10]">
            {/* One SR slider per eligible connector. In cost mode the eligible set is driven by the
                merchant's ingested connectors (see the seeding effect), so these follow suit instead
                of a hardcoded stripe+adyen pair. */}
            {eligibleGatewaysParsed.map((key) => {
              const label = `${key.charAt(0).toUpperCase() + key.slice(1)} success rate`
              const color = gatewayColorMap[key] ?? GW_PALETTE[0]
              // Read from state (not the ref-backed getGwSuccessRate, which only
              // syncs in a post-render effect) so the controlled input reflects each
              // keystroke immediately instead of snapping back to a stale value.
              const rate = (gatewaySimConfigs[key] ?? defaultGwSimConfig(key)).successRate
              return (
                <div key={key} className="flex w-[190px] flex-col gap-1.5">
                  <div className="flex items-center gap-1.5">
                    <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: color }} />
                    <SurfaceLabel>{label}</SurfaceLabel>
                    {/* Any connector can be removed (default, ingested or user-added), but keep at
                        least two so the routing comparison stays meaningful, and don't mutate the
                        eligible set mid-run. */}
                    {eligibleGatewaysParsed.length > 2 && !isSimulating && (
                      <button
                        type="button"
                        onClick={() => removeConnector(key)}
                        className="ml-auto text-slate-400 transition-colors hover:text-red-500"
                        title={`Remove ${key} from the comparison`}
                        aria-label={`Remove ${key}`}
                      >
                        <X size={13} />
                      </button>
                    )}
                  </div>
                  <div className="relative">
                    <input
                      // Whole-number SR (e.g. 96%). Clamped to 0–100 and snapped to 1.
                      type="number"
                      min={0}
                      max={100}
                      step={1}
                      value={rate}
                      onChange={e => setGwSuccessRate(key, e.target.value === '' ? 0 : clampSuccessRate(parseFloat(e.target.value)))}
                      className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 pr-7 text-sm font-semibold text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
                    />
                    <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-sm text-slate-400">%</span>
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={100}
                    step={1}
                    value={rate}
                    onChange={e => setGwSuccessRate(key, clampSuccessRate(Number(e.target.value)))}
                    className="mt-1 h-1.5 w-full cursor-pointer appearance-none rounded-full outline-none
                      [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-white [&::-webkit-slider-thumb]:bg-[color:var(--thumb)] [&::-webkit-slider-thumb]:shadow [&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:hover:scale-110 [&::-webkit-slider-thumb]:active:scale-95
                      [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-4 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-2 [&::-moz-range-thumb]:border-white [&::-moz-range-thumb]:border-solid [&::-moz-range-thumb]:bg-[color:var(--thumb)] [&::-moz-range-thumb]:shadow"
                    style={{
                      '--thumb': color,
                      background: `linear-gradient(to right, ${color} ${rate}%, rgba(148,163,184,0.45) ${rate}%)`,
                    } as CSSProperties}
                  />
                </div>
              )
            })}

            {/* Add another processor to the comparison. Its cost is fitted if that connector has an
                ingested report, otherwise it falls back to seed costs — either way it gets its own
                SR slider so you can pit a third (or fourth) processor against the rest. Laid out as a
                labeled control so it aligns with the sliders/selects on the row. Behind the "More"
                toggle to keep the default bar uncluttered. */}
            {showMoreInputs && (
              <div className="flex w-[100px] flex-col gap-1.5">
                <SurfaceLabel>Processor</SurfaceLabel>
                <button
                  type="button"
                  onClick={openAddConnector}
                  title="Add another processor to the comparison"
                  className="flex items-center justify-center gap-1.5 rounded-lg border border-dashed border-slate-300 bg-slate-50 px-3 py-1.5 text-sm font-medium text-slate-500 transition-colors hover:border-brand-400 hover:text-brand-500 dark:border-[#33333a] dark:bg-[#0d0d13] dark:text-[#8a93a6] dark:hover:border-brand-500 dark:hover:text-brand-400"
                >
                  <Plus size={14} /> Add
                </button>
              </div>
            )}

            {form.ranking_algorithm === 'SR_MULTI_OBJECTIVE' && (
              <div className="flex w-[175px] flex-col gap-1.5">
                <SurfaceLabel>
                  <span title="Pin every multi-objective transaction to one card scenario so the SR Trend shows a single clean per-segment bucket. 'All scenarios' rotates through the 8 card types (interleaves dimensions on one chart). Editable while paused — the rest of the run continues in the new segment on resume.">Card scenario</span>
                </SurfaceLabel>
                <select
                  value={String(multiObjScenario)}
                  disabled={isSimulating && !isPaused}
                  onChange={e => setMultiObjScenario(e.target.value === 'ALL' ? 'ALL' : Number(e.target.value))}
                  className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm font-medium text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:opacity-50 disabled:cursor-not-allowed dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
                >
                  <option value="ALL">All scenarios (rotate)</option>
                  {MULTI_OBJECTIVE_CLUSTER_VARIANTS.map((v, idx) => (
                    <option key={v.label} value={idx}>{v.label}</option>
                  ))}
                </select>
              </div>
            )}

            {form.ranking_algorithm === 'SR_MULTI_OBJECTIVE' && (
              <div className="flex w-[130px] flex-col gap-1.5">
                <SurfaceLabel>
                  <span title="Settlement currency sent on each transaction. USD exercises the seed-cost fallback; EUR/AUD (and others) let a transaction match the in-house fitted cost models, which are keyed by currency.">Currency</span>
                </SurfaceLabel>
                <select
                  value={moCurrency}
                  disabled={isSimulating && !isPaused}
                  onChange={e => setMoCurrency(e.target.value)}
                  className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm font-medium text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:opacity-50 disabled:cursor-not-allowed dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
                >
                  {MULTI_OBJECTIVE_CURRENCIES.map(c => (
                    <option key={c} value={c}>{c}</option>
                  ))}
                </select>
              </div>
            )}

            {showMoreInputs && (() => {
              const tps = Math.max(1, Math.min(MAX_SIMULATION_TPS, simulationConfig.tps || 1))
              return (
                <div className="flex w-[62px] flex-col gap-1.5">
                  <SurfaceLabel>
                    <span title={`Transactions kept in flight at once (1–${MAX_SIMULATION_TPS}). Higher = more decide+feedback round-trips per second, so gateway scores move faster. Applies on the next run.`}>TPS</span>
                  </SurfaceLabel>
                  <input
                    type="number"
                    min={1}
                    max={MAX_SIMULATION_TPS}
                    step={1}
                    value={tps}
                    disabled={isSimulating}
                    onChange={e => {
                      const n = Math.round(Number(e.target.value))
                      const clamped = Number.isFinite(n) ? Math.max(1, Math.min(MAX_SIMULATION_TPS, n)) : 1
                      setSimulationConfig(c => ({ ...c, tps: clamped }))
                    }}
                    className="w-full rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-sm font-semibold text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:cursor-not-allowed disabled:opacity-50 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
                  />
                </div>
              )
            })()}

            {/* Retry is always shown (not behind "More"). The invisible spacer matches the
                SurfaceLabel height so the checkbox row lines up with the input row of the other
                controls. Editable mid-run (read via smartRetryEnabledRef). */}
            <div className="flex flex-col gap-1.5">
              <span className="block h-4" aria-hidden />
              <label
                className="flex h-[34px] items-center gap-2 cursor-pointer select-none"
                title="When on, ~50% of failed transactions are treated as soft declines (GSM retry) and retried on the next eligible processor (alternate PSP)."
              >
                <input
                  type="checkbox"
                  checked={smartRetryEnabled}
                  onChange={e => setSmartRetryEnabled(e.target.checked)}
                  className="cursor-pointer rounded border-slate-300 dark:border-slate-600"
                />
                <span className="text-xs font-medium text-slate-600 dark:text-slate-300">Retry Enabled</span>
              </label>
            </div>

            {SHOW_AMOUNT_RANGE_SLIDER && (() => {
              const BMIN = SIMULATION_AMOUNT_BOUND_MIN
              const BMAX = SIMULATION_AMOUNT_BOUND_MAX
              const lo = Math.max(BMIN, Math.min(simulationConfig.minAmount, simulationConfig.maxAmount))
              const hi = Math.min(BMAX, Math.max(simulationConfig.minAmount, simulationConfig.maxAmount))
              const pct = (v: number) => ((v - BMIN) / (BMAX - BMIN)) * 100
              const amtColor = '#10b981'
              // Two overlapped range inputs: track is transparent (the divs below draw it),
              // and only the thumbs receive pointer events so both handles stay draggable.
              const rangeCls = `pointer-events-none absolute inset-0 h-4 w-full appearance-none bg-transparent outline-none disabled:cursor-not-allowed
                [&::-webkit-slider-runnable-track]:h-4 [&::-webkit-slider-runnable-track]:bg-transparent [&::-moz-range-track]:h-4 [&::-moz-range-track]:bg-transparent
                [&::-webkit-slider-thumb]:pointer-events-auto [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:border-2 [&::-webkit-slider-thumb]:border-white [&::-webkit-slider-thumb]:bg-[color:var(--thumb)] [&::-webkit-slider-thumb]:shadow [&::-webkit-slider-thumb]:cursor-pointer
                [&::-moz-range-thumb]:pointer-events-auto [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-4 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-2 [&::-moz-range-thumb]:border-white [&::-moz-range-thumb]:border-solid [&::-moz-range-thumb]:bg-[color:var(--thumb)] [&::-moz-range-thumb]:shadow [&::-moz-range-thumb]:cursor-pointer`
              return (
                <div className="flex w-[220px] flex-col gap-1.5">
                  <div className="flex items-center justify-between gap-1.5">
                    <SurfaceLabel>
                      <span title="Per-transaction amount range (multi-objective sim). Each payment's amount is drawn uniformly from this range. Smaller amounts make the flat per-txn fee a bigger share of cost. Applies on the next run.">Amount range</span>
                    </SurfaceLabel>
                    <span className="text-xs font-semibold tabular-nums text-slate-600 dark:text-slate-300">${lo}–${hi}</span>
                  </div>
                  <div className="relative mt-2 h-4 w-full">
                    <div className="absolute top-1/2 h-1.5 w-full -translate-y-1/2 rounded-full bg-slate-200 dark:bg-[#23232b]" />
                    <div
                      className="absolute top-1/2 h-1.5 -translate-y-1/2 rounded-full"
                      style={{ left: `${pct(lo)}%`, right: `${100 - pct(hi)}%`, background: amtColor }}
                    />
                    <input
                      type="range" min={BMIN} max={BMAX} step={5} value={lo}
                      onChange={e => { const v = Math.min(Number(e.target.value), hi); setSimulationConfig(c => ({ ...c, minAmount: v })) }}
                      className={rangeCls} style={{ '--thumb': amtColor } as CSSProperties}
                    />
                    <input
                      type="range" min={BMIN} max={BMAX} step={5} value={hi}
                      onChange={e => { const v = Math.max(Number(e.target.value), lo); setSimulationConfig(c => ({ ...c, maxAmount: v })) }}
                      className={rangeCls} style={{ '--thumb': amtColor } as CSSProperties}
                    />
                  </div>
                </div>
              )
            })()}

            {/* Action cluster, pushed to the trailing edge. The invisible spacer matches the
                SurfaceLabel height (leading-4 = 16px) so the control row lines up with the
                SR inputs / Card scenario select rather than bottom-aligning under the sliders. */}
            <div className="flex flex-col gap-1.5 lg:ml-auto">
              <span className="block h-4" aria-hidden />
              <div className="flex flex-wrap items-center gap-3">
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setShowMoreInputs(v => !v)}
                  title="Show or hide the advanced controls (TPS, Add processor)"
                >
                  <SlidersHorizontal size={14} />
                  {showMoreInputs ? 'Less' : 'More'}
                </Button>

                <Button size="sm" variant="secondary" onClick={resetCurrentTabState}>
          <RefreshCw size={14} />
          {resetButtonLabel}
        </Button>

                {!isSimulating ? (
                  resumableRun ? (
                    // A run was paused and survived leaving the page (e.g. to edit
                    // Multi-Objective config). Resume continues from where it stopped;
                    // Start over discards the snapshot and runs fresh.
                    <>
                      <Button
                        onClick={resumeFromSnapshot}
                        disabled={!effectiveMerchantId || routingConfigUnavailable}
                        variant="primary"
                        title={`Continue the paused run from transaction ${resumableRun.nextIndex + 1} of ${resumableRun.total}`}
                      >
                        <Play size={14} className="fill-current" /> Resume run
                      </Button>
                      <Button onClick={discardResumableRun} variant="secondary">
                        <RefreshCw size={14} /> Start over
                      </Button>
                    </>
                  ) : (
                    <Button
                      onClick={() => runSimulation()}
                      disabled={!effectiveMerchantId || routingConfigUnavailable}
                      variant="primary"
                    >
                      <Play size={14} className="fill-current" /> Run simulation
                    </Button>
                  )
                ) : (
                  <>
                    {isPaused ? (
                      <Button onClick={resumeSimulation} variant="primary">
                        <Play size={14} className="fill-current" /> Resume
                      </Button>
                    ) : (
                      <Button onClick={pauseSimulation} variant="secondary">
                        <Pause size={14} /> Pause
                      </Button>
                    )}
                    <Button onClick={() => { simulationAbortRef.current = true }} variant="secondary">
                      <X size={14} /> Stop
                    </Button>
                  </>
                )}
              </div>
            </div>
          </div>
      )}

      <div className={activeTab === 'volume'
        ? 'grid grid-cols-1 gap-5 xl:grid-cols-[minmax(340px,420px)_minmax(0,1fr)]'
        : activeTab === 'batch'
          ? 'grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)] lg:items-stretch'
          : 'grid grid-cols-1 gap-6 lg:grid-cols-2'
      }
        style={activeTab === 'rule' ? { display: 'none' } : undefined}
      >
        <div className={`flex flex-col gap-6 min-w-0 ${activeTab === 'batch' ? 'lg:min-h-0' : 'self-start'}`}>
        {activeTab === 'batch' && (
                <Card className="flex-1">
                  <CardBody className="!pt-4 flex flex-1 flex-col">
                    {sortedGatewayStats.length > 0 ? (() => {
                      const sortedGateways = sortedGatewayStats
                      const hasAnySpark = sortedGateways.some(([gw]) => (gatewaySparklines.series[gw]?.length ?? 0) >= 2)
                      return (
                        <div className="flex flex-1 flex-col gap-4">
                          {/* per-gateway stat rows moved to the Gateway Selection Summary
                              card in the right column (enlarged, above Autopilot Actions). */}
                          {/* combined multi-line SR trend chart */}
                          {hasAnySpark && (() => {
                            const { series, evSeries, decidedSeries, paymentNums } = gatewaySparklines
                            // Routing is pure expected-value now — there is no auth band. The
                            // tooltip stars the *selected* PSP (the engine's actual pick, i.e. the
                            // highest-EV connector — not necessarily the SR leader), and no band
                            // line or shaded region is drawn.
                            const chartData = paymentNums.map((n, i) => {
                              const row: Record<string, number | string> = { step: n }
                              sortedGateways.forEach(([gw]) => {
                                const v = series[gw]?.[i]
                                if (v != null) row[gw] = v
                              })
                              // Selected (decided) gateway at this point — what the tooltip stars.
                              const sel = decidedSeries[i]
                              if (sel != null) row.selectedGw = sel
                              // Winner's EV lead over the runner-up at this point (fraction of ticket);
                              // null on pure-SR points is left off the row so the tooltip shows "—".
                              const ev = evSeries[i]
                              if (ev != null) row.evLead = ev
                              return row
                            })
                            // Zoom the Y-axis to the connector lines so a small gap (e.g. 93% vs
                            // 94%) is legible.
                            const yValues: number[] = []
                            chartData.forEach((row) => {
                              sortedGateways.forEach(([gw]) => { const v = row[gw]; if (typeof v === 'number') yValues.push(v) })
                            })
                            const dataMin = yValues.length ? Math.min(...yValues) : 0
                            const dataMax = yValues.length ? Math.max(...yValues) : 100
                            const spread = Math.max(dataMax - dataMin, 0)
                            // Never zoom tighter than this (pp). Early in a run only a few noisy
                            // points exist; without a floor the domain snaps to their ~0.3pp jitter
                            // and a tiny wiggle fills the whole chart. Holding a minimum window keeps
                            // sub-pp jitter small on-screen while the connector gap stays legible.
                            const MIN_SPAN = 6
                            const effectiveSpread = Math.max(spread, MIN_SPAN)
                            // Discrete zoom bands keyed off the *effective* spread so the gridline
                            // count stays roughly constant as the real gap grows.
                            const yStep = effectiveSpread <= 10 ? 2 : effectiveSpread <= 30 ? 5 : 10
                            // Center the (>= MIN_SPAN) window on the data, then snap out to whole ticks.
                            const center = (dataMin + dataMax) / 2
                            const half = Math.max(spread / 2, MIN_SPAN / 2)
                            const yMin = Math.max(0, Math.floor((center - half) / yStep) * yStep)
                            const yMaxTick = Math.min(100, Math.ceil((center + half) / yStep) * yStep)
                            // Domain reaches a hair past the top tick so a line at the max isn't
                            // clipped flat against the edge; ticks themselves stay capped at 100.
                            const yMax = yMaxTick + Math.min(yStep, 2)
                            const yTicks: number[] = []
                            for (let t = yMin; t <= yMaxTick; t += yStep) yTicks.push(t)
                            return (
                              <div className="w-full flex-1 min-h-[380px] flex flex-col">
                                <div className="mb-2 flex items-start justify-between gap-3">
                                  <div>
                                    <h4 className="text-sm font-medium text-slate-800 dark:text-white">Success Rate Trend</h4>
                                    {/* <p className="text-[13px] text-slate-400 dark:text-slate-500">Engine routing score per connector, with the cost-eligible band below the leader</p> */}
                                  </div>
                                  <div className="flex items-center gap-2 shrink-0">
                                    {/* <span className="text-[11px] text-slate-400 dark:text-slate-500">Window</span> */}
                                    <div className="inline-flex rounded-md border border-slate-200 dark:border-[#1f1f29] p-0.5">
                                      {CHART_WINDOW_OPTIONS.map((opt) => {
                                        const active = chartWindow === opt
                                        return (
                                          <button
                                            key={String(opt)}
                                            type="button"
                                            onClick={() => setChartWindow(opt)}
                                            className={`px-2.5 py-1 text-[11px] font-medium rounded transition-colors ${
                                              active
                                                ? 'bg-brand-500 text-white'
                                                : 'text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200'
                                            }`}
                                          >
                                            {opt === 'all' ? 'All' : `Last ${opt}`}
                                          </button>
                                        )
                                      })}
                                    </div>
                                  </div>
                                </div>
                                <div className="min-h-0 flex-1">
                                <ResponsiveContainer width="100%" height="100%">
                                  <ComposedChart data={chartData} margin={{ top: 16, right: 8, bottom: 8, left: 0 }}>
                                    <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                                    <XAxis
                                      dataKey="step"
                                      tick={{ fontSize: 11, fill: '#94a3b8' }}
                                      tickLine={false}
                                      axisLine={{ stroke: '#e2e8f0' }}
                                      minTickGap={28}
                                    />
                                    <YAxis
                                      domain={[yMin, yMax]}
                                      ticks={yTicks}
                                      allowDataOverflow
                                      tick={{ fontSize: 11, fill: '#94a3b8' }}
                                      tickLine={false}
                                      axisLine={{ stroke: '#e2e8f0' }}
                                      width={44}
                                      tickFormatter={(v: number) => `${v}%`}
                                    />
                                    <Tooltip
                                      content={(props) => {
                                        const { active, payload } = props as unknown as { active?: boolean; payload?: Array<{ payload?: Record<string, number | string> }> }
                                        if (!active || !payload || !payload.length) return null
                                        const row = payload[0].payload
                                        if (!row) return null
                                        // The starred PSP is the one the engine *selected* (highest EV),
                                        // which the EV-lead figure is measured over — not max(SR).
                                        const selectedGw = typeof row.selectedGw === 'string' ? row.selectedGw : null
                                        const evLead = typeof row.evLead === 'number' ? row.evLead : null
                                        return (
                                          <div style={{ ...CHART_TOOLTIP_STYLE, padding: '10px 12px', fontSize: 12, lineHeight: 1.5, minWidth: 190 }}>
                                            <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>Payment {row.step}</p>
                                            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                                              {sortedGateways.map(([gw]) => {
                                                const v = row[gw]
                                                if (typeof v !== 'number') return null
                                                const gwColor = gatewayColorMap[gw] ?? GW_PALETTE[0]
                                                const isSelected = selectedGw === gw
                                                return (
                                                  <div key={gw} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                                                    <span style={{ width: 8, height: 8, borderRadius: 999, backgroundColor: gwColor, flexShrink: 0 }} />
                                                    <span style={{ color: gwColor, fontWeight: 600 }}>{gw}</span>
                                                    <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>{v.toFixed(1)}%</span>
                                                    {selectedGw != null && (
                                                      <span style={{ fontSize: 10, fontWeight: 600, width: 56, textAlign: 'right', color: '#64748b' }}>
                                                        {isSelected ? '★ Selected' : ''}
                                                      </span>
                                                    )}
                                                  </div>
                                                )
                                              })}
                                            </div>
                                            {/* Selected PSP's expected-value lead over the runner-up
                                                (EV(#1) − EV(#2), % of ticket). "—" on pure-SR points
                                                or when fewer than two PSPs had cost data to rank on EV. */}
                                            <div style={{ marginTop: 6, paddingTop: 6, borderTop: '1px solid rgba(148,163,184,0.25)', display: 'flex', alignItems: 'center', gap: 6 }}>
                                              <span style={{ color: '#64748b' }}>EV lead{selectedGw != null ? ` · ${selectedGw}` : ''}</span>
                                              <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}>
                                                {evLead != null ? `+${(evLead * 100).toFixed(2)}% of ticket` : '—'}
                                              </span>
                                            </div>
                                          </div>
                                        )
                                      }}
                                    />
                                    {sortedGateways.map(([gw]) => (
                                      <Line
                                        key={gw}
                                        type="natural"
                                        dataKey={gw}
                                        name={gw}
                                        stroke={gatewayColorMap[gw] ?? GW_PALETTE[0]}
                                        strokeWidth={2.5}
                                        dot={false}
                                        activeDot={false}
                                        isAnimationActive={false}
                                        connectNulls
                                      />
                                    ))}
                                  </ComposedChart>
                                </ResponsiveContainer>
                                </div>
                                <div className="mt-3 flex flex-wrap items-center justify-center gap-x-5 gap-y-1.5 text-[11px] text-slate-500 dark:text-slate-400">
                                  {sortedGateways.map(([gw]) => (
                                    <span key={gw} className="inline-flex items-center gap-1.5">
                                      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: gatewayColorMap[gw] ?? GW_PALETTE[0] }} />
                                      {gw}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )
                          })()}
                          {/* routing-share trend — traffic split across connectors */}
                          {gatewayVolumeTrend.data.length >= 2 && (() => {
                            const { data, gateways } = gatewayVolumeTrend
                            const latest = data[data.length - 1] ?? {}
                            const colorFor = (gw: string, idx: number) => gatewayColorMap[gw] ?? GW_PALETTE[idx % GW_PALETTE.length]
                            const latestSplit = gateways
                              .map((gw, idx) => ({ gw, share: latest[gw] ?? 0, color: colorFor(gw, idx) }))
                              .sort((a, b) => b.share - a.share)
                            // Each connector is its own line at its real share (they sum to
                            // 100%, but are not stacked) so a hand-off shows as the two lines
                            // crossing. Paint the taller area first so the smaller one's line
                            // stays visible on top.
                            const drawOrder = gateways
                              .map((gw, idx) => ({ gw, idx, share: latest[gw] ?? 0 }))
                              .sort((a, b) => b.share - a.share)
                            return (
                              <div className="w-full border-t border-slate-100 pt-4 dark:border-[#1a1a22]">
                                <div className="mb-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
                                  <div>
                                    <h4 className="text-sm font-medium text-slate-800 dark:text-white">Routing Distribution</h4>
                                  </div>
                                  <div className="flex flex-wrap items-center gap-1.5">
                                    {latestSplit.map(({ gw, share, color }) => (
                                      <span
                                        key={gw}
                                        className="inline-flex items-center gap-1.5 rounded-full bg-slate-50 px-2 py-0.5 text-[11px] font-medium text-slate-600 ring-1 ring-inset ring-slate-200/70 dark:bg-[#12121a] dark:text-slate-300 dark:ring-[#22222c]"
                                      >
                                        <span className="h-2 w-2 rounded-full" style={{ backgroundColor: color }} />
                                        {gw}
                                        <span className="tabular-nums font-semibold text-slate-900 dark:text-white">{Math.round(share)}%</span>
                                      </span>
                                    ))}
                                  </div>
                                </div>
                                <div className="h-[200px] w-full">
                                  <ResponsiveContainer width="100%" height="100%">
                                    <ComposedChart data={data} margin={{ top: 8, right: 8, bottom: 8, left: 0 }}>
                                      <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                                      <XAxis
                                        dataKey="step"
                                        tick={{ fontSize: 11, fill: '#94a3b8' }}
                                        tickLine={false}
                                        axisLine={{ stroke: '#e2e8f0' }}
                                        minTickGap={28}
                                      />
                                      <YAxis
                                        domain={[0, 100]}
                                        ticks={[0, 25, 50, 75, 100]}
                                        tick={{ fontSize: 11, fill: '#94a3b8' }}
                                        tickLine={false}
                                        axisLine={{ stroke: '#e2e8f0' }}
                                        width={44}
                                        tickFormatter={(v: number) => `${v}%`}
                                      />
                                      <Tooltip
                                        cursor={{ stroke: '#cbd5e1', strokeWidth: 1 }}
                                        content={(props) => {
                                          const { active, payload } = props as unknown as { active?: boolean; payload?: Array<{ payload?: Record<string, number> }> }
                                          if (!active || !payload || !payload.length) return null
                                          const row = payload[0].payload
                                          if (!row) return null
                                          const rows = gateways
                                            .map((gw, idx) => ({ gw, share: row[gw] ?? 0, color: colorFor(gw, idx) }))
                                            .sort((a, b) => b.share - a.share)
                                          return (
                                            <div style={{ ...CHART_TOOLTIP_STYLE, padding: '10px 12px', fontSize: 12, lineHeight: 1.5, minWidth: 170 }}>
                                              <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>Payment {row.step}</p>
                                              <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                                                {rows.map(({ gw, share, color }) => (
                                                  <div key={gw} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                                                    <span style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: color, flexShrink: 0 }} />
                                                    <span style={{ color, fontWeight: 600 }}>{gw}</span>
                                                    <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>{share.toFixed(1)}%</span>
                                                  </div>
                                                ))}
                                              </div>
                                            </div>
                                          )
                                        }}
                                      />
                                      {drawOrder.map(({ gw, idx }) => (
                                        <Area
                                          key={gw}
                                          type="monotone"
                                          dataKey={gw}
                                          name={gw}
                                          stroke={colorFor(gw, idx)}
                                          fill={colorFor(gw, idx)}
                                          fillOpacity={0.18}
                                          strokeWidth={2.5}
                                          isAnimationActive={false}
                                          legendType="none"
                                          activeDot={{ r: 3.5, strokeWidth: 0 }}
                                        />
                                      ))}
                                    </ComposedChart>
                                  </ResponsiveContainer>
                                </div>
                              </div>
                            )
                          })()}
                        </div>
                      )
                    })() : (() => {
                      // Pre-run preview: project flat SR lines from the configured
                      // gateway success rates so the chart is visible on landing,
                      // before any simulated payment has been routed.
                      const previewGateways = eligibleGatewaysParsed
                      if (previewGateways.length === 0) return null
                      const previewScores = previewGateways.map(gw => ({ gw, sr: (gatewaySimConfigs[gw] ?? defaultGwSimConfig(gw)).successRate }))
                      const target = totalSimulationPayments || Number(SIMULATION_TOTAL_PAYMENTS)
                      // Empty plot area until a simulation runs — only the axis range
                      // is seeded so the chart frame renders without any band or lines.
                      const chartData = [{ step: 1 }, { step: target }]
                      return (
                        <div className="flex flex-1 flex-col gap-4">
                          <div className="space-y-2">
                            {previewScores.map(({ gw, sr }) => {
                              const gwColor = gatewayColorMap[gw] ?? GW_PALETTE[0]
                              return (
                                <div key={gw} className="space-y-1">
                                  <div className="flex items-center justify-end gap-1.5">
                                    <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: gwColor }} />
                                    <span className="text-xs font-semibold text-slate-700 dark:text-slate-200 truncate shrink-0 w-14 text-left">{gw}</span>
                                    <span className={`font-bold tabular-nums text-[11px] w-14 text-right ${sr >= 80 ? 'text-emerald-600 dark:text-emerald-400' : sr >= 50 ? 'text-amber-500' : 'text-red-500'}`}>{sr}% SR</span>
                                    <span className="text-slate-300 dark:text-slate-600 text-[11px]">·</span>
                                    <span className="text-slate-400 dark:text-slate-500 tabular-nums text-[11px] w-16 text-right">0% routed</span>
                                  </div>
                                </div>
                              )
                            })}
                          </div>
                          <div className="w-full flex-1 min-h-[300px]">
                            <ResponsiveContainer width="100%" height="100%">
                              <LineChart data={chartData} margin={{ top: 16, right: 8, bottom: 8, left: 0 }}>
                                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                                <XAxis
                                  dataKey="step"
                                  tick={{ fontSize: 11, fill: '#94a3b8' }}
                                  tickLine={false}
                                  axisLine={{ stroke: '#e2e8f0' }}
                                  minTickGap={28}
                                />
                                <YAxis
                                  domain={[0, 100]}
                                  tick={{ fontSize: 11, fill: '#94a3b8' }}
                                  tickLine={false}
                                  axisLine={{ stroke: '#e2e8f0' }}
                                  width={44}
                                  tickFormatter={(v: number) => `${v}%`}
                                />
                              </LineChart>
                            </ResponsiveContainer>
                          </div>
                          <p className="pt-1 text-center text-[11px] text-slate-400">
                            Projected from configured success rates · run a simulation to see live routing.
                          </p>
                        </div>
                      )
                    })()}
                    {Object.keys(gatewayStats).length === 1 && eligibleGatewaysParsed.length > 1 && (
                      <p className="text-[11px] text-slate-400 pt-1">
                        SR routing concentrates traffic on the highest-scoring gateway. Run with a "Gateway Down" scenario to see score drop and traffic shift to the next gateway.
                      </p>
                    )}
                  </CardBody>
                </Card>
        )}
        {activeTab !== 'batch' && (
        <Card className="!rounded-2xl">
          <CardHeader className="!px-5 !py-4">
            <div>
              <SurfaceLabel>
                {activeTab === 'rule' ? 'Rule Evaluation' :
                  activeTab === 'volume' ? 'Volume Split' :
                    activeTab === 'debit' ? 'Network Routing' :
                      'Simulation'}
              </SurfaceLabel>
              <h2 className="mt-1.5 font-medium text-slate-800 dark:text-white">
                {activeTab === 'rule' ? 'Rule Evaluation Parameters' :
                  activeTab === 'volume' ? 'Volume Split Configuration' :
                    activeTab === 'debit' ? 'Debit Routing Parameters' :
                      'Auth-Rate Based Routing Parameters'}
              </h2>
            </div>
          </CardHeader>
          <CardBody className="space-y-3 !px-5 !py-4">
            {!effectiveMerchantId && (
              <p className="text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
                Set a merchant ID in the top bar first.
              </p>
            )}
            {activeTab !== 'volume' && activeTab !== 'debit' && routingKeysLoading && (
              <p className="text-xs text-slate-600 bg-slate-50 border border-slate-200 rounded px-3 py-2">
                Loading routing config from backend...
              </p>
            )}
            {activeTab !== 'volume' && activeTab !== 'debit' && routingConfigUnavailable && (
              <ErrorMessage error="Routing config unavailable from /config/routing-keys. Parameter forms are disabled." />
            )}

            {activeTab === 'rule' ? (
              <>
                {routingKeysLoading && (
                  <p className="text-sm text-slate-500">Loading routing keys from backend...</p>
                )}
                {routingConfigUnavailable && (
                  <ErrorMessage error="Routing keys are unavailable from backend (/config/routing-keys). Rule Evaluation is disabled." />
                )}

                {/* Parameters */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <p className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 dark:text-[#4e5870]">Parameters</p>
                  </div>
                  <div className="space-y-1.5">
                    {ruleParams.map((param, idx) => (
                      <div key={idx} className="space-y-1.5">
                        <div className="group flex items-center gap-0 rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] overflow-hidden transition-shadow hover:shadow-sm">
                          <select
                            value={param.key}
                            onChange={e => updateRuleParamKey(idx, e.target.value)}
                            disabled={routingConfigUnavailable || routingKeysLoading}
                            className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm font-medium text-slate-700 dark:text-[#c8d0de] focus:outline-none cursor-pointer appearance-none"
                          >
                            {routingKeyNames.length === 0 ? (
                              <option value="">No keys available</option>
                            ) : (
                              routingKeyNames.map(name => <option key={name} value={name}>{name}</option>)
                            )}
                          </select>
                          <span className="shrink-0 border-x border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-2.5 py-2.5 text-[11px] font-bold text-slate-300 dark:text-[#3a4258] select-none">=</span>
                          {param.type === 'enum_variant' ? (
                            <select
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none cursor-pointer appearance-none"
                            >
                              {(routingKeysConfig[param.key]?.values || []).map(v => (
                                <option key={v} value={v}>{v}</option>
                              ))}
                            </select>
                          ) : param.type === 'number' ? (
                            <input
                              type="number"
                              placeholder="Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none"
                            />
                          ) : param.type !== 'metadata_variant' ? (
                            <input
                              placeholder="Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none"
                            />
                          ) : (
                            <span className="flex-1 px-3 py-2.5 text-sm text-slate-400 dark:text-[#3a4258] italic">see below</span>
                          )}
                          <button
                            onClick={() => removeRuleParam(idx)}
                            className="shrink-0 px-2.5 py-2.5 text-slate-300 dark:text-[#2a3040] hover:text-red-400 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                          >
                            <Trash2 size={13} />
                          </button>
                        </div>
                        {param.type === 'metadata_variant' && (
                          <div className="ml-3 flex gap-1.5">
                            <input
                              placeholder="Metadata key"
                              value={param.metadataKey || ''}
                              onChange={e => updateRuleParamMetadataKey(idx, e.target.value)}
                              className="flex-1 rounded-lg border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] px-3 py-2 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                            <input
                              placeholder="Metadata value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 rounded-lg border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] px-3 py-2 text-sm text-slate-600 dark:text-[#a8b4c8] focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addRuleParam}
                    disabled={routingConfigUnavailable || routingKeysLoading || routingKeyNames.length === 0}
                    className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-brand-500 hover:bg-brand-50 dark:hover:bg-brand-500/10 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
                  >
                    <Plus size={12} /> Add Parameter
                  </button>
                </div>

                {/* Fallback Gateways */}
                <div className="space-y-2">
                  <p className="text-[11px] font-semibold uppercase tracking-wider text-slate-400 dark:text-[#4e5870]">Fallback Gateways</p>
                  <div className="space-y-1.5">
                    {fallbackConnectors.map((connector, idx) => (
                      <div key={idx} className="group flex items-center gap-0 rounded-xl border border-slate-200 dark:border-[#1e2330] bg-white dark:bg-[#0c0f17] overflow-hidden transition-shadow hover:shadow-sm">
                        <span className="shrink-0 flex items-center justify-center w-8 self-stretch bg-slate-50 dark:bg-[#10131c] border-r border-slate-100 dark:border-[#1e2330] text-[10px] font-bold text-slate-300 dark:text-[#3a4258] select-none">
                          {idx + 1}
                        </span>
                        <input
                          placeholder="gateway name"
                          value={connector.gateway_name}
                          onChange={e => updateFallbackConnector(idx, 'gateway_name', e.target.value)}
                          className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm font-medium text-slate-700 dark:text-[#c8d0de] focus:outline-none"
                        />
                        <span className="shrink-0 border-x border-slate-100 dark:border-[#1e2330] bg-slate-50 dark:bg-[#10131c] px-2 py-2.5 text-[11px] font-bold text-slate-300 dark:text-[#3a4258] select-none">/</span>
                        <input
                          placeholder="gateway id (optional)"
                          value={connector.gateway_id || ''}
                          onChange={e => updateFallbackConnector(idx, 'gateway_id', e.target.value)}
                          className="flex-1 min-w-0 bg-transparent px-3 py-2.5 text-sm text-slate-500 dark:text-[#8090a8] focus:outline-none"
                        />
                        <button
                          onClick={() => removeFallbackConnector(idx)}
                          className="shrink-0 px-2.5 py-2.5 text-slate-300 dark:text-[#2a3040] hover:text-red-400 dark:hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addFallbackConnector}
                    className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-brand-500 hover:bg-brand-50 dark:hover:bg-brand-500/10 transition-colors"
                  >
                    <Plus size={12} /> Add Gateway
                  </button>
                </div>
              </>
            ) : activeTab === 'debit' ? (
              <div className="space-y-4">
                {debitRoutingFlag.isLoading ? (
                  <p className="flex items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-600 dark:border-[#222226] dark:bg-[#10131a] dark:text-[#aab5c8]">
                    <Spinner size={14} />
                    Loading debit routing flag...
                  </p>
                ) : debitRoutingFlag.isEnabled ? (
                  <p className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300">
                    Debit routing is enabled. This tab will call /decide-gateway with NTW_BASED_ROUTING.
                  </p>
                ) : (
                  <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <span>Debit routing is disabled.</span>
                      <Button size="sm" variant="secondary" onClick={enableDebitRoutingForExplorer} disabled={!effectiveMerchantId || loading}>
                        Enable Debit Routing
                      </Button>
                    </div>
                  </div>
                )}

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Amount</label>
                    <input
                      value={debitForm.amount}
                      onChange={e => setDebitField('amount', e.target.value)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Currency</label>
                    <input
                      value={debitForm.currency}
                      onChange={e => setDebitField('currency', e.target.value.toUpperCase())}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Auth Type</label>
                    <select
                      value={debitForm.auth_type}
                      onChange={e => setDebitField('auth_type', e.target.value)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    >
                      <option value="THREE_DS">THREE_DS</option>
                      <option value="NO_THREE_DS">NO_THREE_DS</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Card Type</label>
                    <select
                      value={debitForm.card_type}
                      onChange={e => setDebitField('card_type', e.target.value as DebitRoutingFormState['card_type'])}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    >
                      <option value="debit">Debit</option>
                      <option value="credit">Credit</option>
                    </select>
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-600 mb-1">Eligible Gateways (comma-separated)</label>
                  <input
                    value={debitForm.eligible_gateways}
                    onChange={e => setDebitField('eligible_gateways', e.target.value)}
                    placeholder="stripe, adyen"
                    className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Merchant Category Code</label>
                    <input
                      value={debitForm.merchant_category_code}
                      onChange={e => setDebitField('merchant_category_code', e.target.value)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Acquirer Country</label>
                    <input
                      value={debitForm.acquirer_country}
                      onChange={e => setDebitField('acquirer_country', e.target.value.toUpperCase())}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-600 mb-1">Co-badged Networks (comma-separated)</label>
                  <input
                    value={debitForm.co_badged_networks}
                    onChange={e => setDebitField('co_badged_networks', e.target.value)}
                    placeholder="VISA, NYCE, PULSE, STAR"
                    className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  />
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Issuer Country</label>
                    <input
                      value={debitForm.issuer_country}
                      onChange={e => setDebitField('issuer_country', e.target.value.toUpperCase())}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                  <div className="flex items-center gap-2 pt-6">
                    <input
                      id="debit-is-regulated"
                      type="checkbox"
                      checked={debitForm.is_regulated}
                      onChange={e => setDebitField('is_regulated', e.target.checked)}
                      className="h-4 w-4 rounded border-slate-300"
                    />
                    <label htmlFor="debit-is-regulated" className="text-sm text-slate-600 dark:text-[#aab5c8]">
                      Regulated debit card
                    </label>
                  </div>
                </div>

                {debitForm.is_regulated && (
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Regulated Name</label>
                    <input
                      value={debitForm.regulated_name}
                      onChange={e => setDebitField('regulated_name', e.target.value)}
                      placeholder="GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)"
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                    />
                  </div>
                )}

                <p className="text-xs text-slate-500">
                  The request sends debit details inside paymentInfo.metadata because the backend debit router parses co-badged card data from metadata.
                </p>
              </div>
            ) : activeTab === 'volume' ? (
              <div className="space-y-4">
                <div>
                  <label className="mb-2 block text-xs font-semibold uppercase tracking-[0.14em] text-slate-500 dark:text-[#7f8ca3]">
                    Evaluation count
                  </label>
                  <div className="flex gap-2">
                    <div className="flex flex-1 items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 dark:border-[#283241] dark:bg-[#101722]">
                      <input
                        type="number"
                        min="1"
                        inputMode="numeric"
                        value={volumePayments}
                        onChange={e => setVolumePayments(e.target.value)}
                        className="min-w-0 flex-1 bg-transparent text-xl font-semibold text-slate-950 outline-none dark:text-white"
                      />
                      <span className="shrink-0 rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-500 dark:bg-[#1d2633] dark:text-[#91a0b8]">
                        runs
                      </span>
                    </div>
                    <Button
                      onClick={runVolumeSplit}
                      disabled={loading || !effectiveMerchantId}
                      className="shrink-0 dark:bg-sky-500 dark:text-white dark:hover:bg-sky-400"
                    >
                      {loading ? <><Spinner size={14} /> Running…</> : <><PieChartIcon size={14} /> Run</>}
                    </Button>
                  </div>
                  <p className="mt-2 text-xs leading-5 text-slate-500 dark:text-[#8f9bb0]">
                    Samples the active volume split strategy through <code>/routing/evaluate</code> and records each decision trace.
                  </p>
                </div>

                <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-[#283241] dark:bg-[#0b111a]">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center gap-2">
                      <span className="font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#77849a]">Target</span>
                      <span className="font-semibold text-slate-900 dark:text-white">{volumeRunTarget || '--'}</span>
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#77849a]">Completed</span>
                      <span className="font-semibold text-slate-900 dark:text-white">
                        {loading ? volumeProgress : volumeEvaluationCount || '--'}
                      </span>
                    </div>
                  </div>
                  <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-200 dark:bg-[#1d2633]">
                    <div
                      className={`h-full rounded-full transition-[width] ${loading ? 'bg-sky-500' : 'bg-slate-400 dark:bg-[#3a4a5c]'}`}
                      style={{
                        width: `${loading
                          ? volumeProgressPercentage
                          : (volumeEvaluationCount && volumeRunTarget ? Math.min(100, Math.round((volumeEvaluationCount / volumeRunTarget) * 100)) : 0)}%`
                      }}
                    />
                  </div>
                  {loading && (
                    <p className="mt-1 text-right text-[10px] font-semibold text-sky-600 dark:text-sky-300">
                      {volumeProgressPercentage}%
                    </p>
                  )}
                </div>
              </div>
            ) : (
              <>
                {activeTab === 'single' && (
                  <>
                    <div>
                      <label className="block text-xs font-medium text-slate-600 mb-1">Transaction Outcome</label>
                      <select
                        value={singleRunOutcome}
                        onChange={e => setSingleRunOutcome(e.target.value as TransactionOutcome)}
                        className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                      >
                        <option value="CHARGED">Success (CHARGED)</option>
                        <option value="FAILURE">Failure (FAILURE)</option>
                      </select>
                      <p className="mt-1 text-xs text-slate-500">
                        After deciding the gateway, single test will post feedback with this outcome so the payment appears in Decision Audit.
                      </p>
                    </div>
                    {singleRunOutcome === 'FAILURE' && <ErrorInfoFields info={errorInfo} onChange={setErrorField} rules={gsmRules} />}
                  </>
                )}
              </>
            )}
          </CardBody>
          <div className="border-t border-slate-200 dark:border-[#2a303a] px-5 py-4 space-y-3">
            <ErrorMessage error={error} />
            {activeTab === 'rule' ? (
              <Button onClick={runRuleEvaluation} disabled={loading || routingConfigUnavailable} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Evaluating…</> : <><Play size={14} /> Evaluate Rules</>}
              </Button>
            ) : activeTab === 'debit' ? (
              <Button
                onClick={runDebitRouting}
                disabled={loading || !effectiveMerchantId || debitRoutingFlag.isLoading || !debitRoutingFlag.isEnabled}
                className="w-full justify-center"
              >
                {loading ? <><Spinner size={14} /> Running Debit Routing…</> : <><Network size={14} /> Run Debit Routing</>}
              </Button>
            ) : activeTab === 'volume' ? null : (
              <>
                {FEATURE_FLAGS.SMART_RETRY_IN_SIMULATION && (
                  <label className={`flex items-center gap-2 select-none ${gsmScoringFilterEnabled ? 'cursor-pointer' : 'cursor-not-allowed opacity-50'}`}>
                    <input
                      type="checkbox"
                      checked={smartRetryEnabled}
                      onChange={e => setSmartRetryEnabled(e.target.checked)}
                      disabled={!gsmScoringFilterEnabled}
                      className="rounded border-slate-300 dark:border-slate-600 disabled:cursor-not-allowed"
                    />
                    <span className="text-xs text-slate-600 dark:text-slate-400">
                      Smart retry — on GSM <code className="text-[11px]">retry</code> decision, attempt next fallback gateway
                      {!gsmScoringFilterEnabled && <span className="ml-1 text-amber-500">(enable GSM scoring filter first)</span>}
                    </span>
                  </label>
                )}
                <Button onClick={run} disabled={loading || !effectiveMerchantId || routingConfigUnavailable} className="w-full justify-center">
                  {loading ? <><Spinner size={14} /> Running…</> : <><Play size={14} /> Run Single Transaction</>}
                </Button>
              </>
            )}
          </div>
        </Card>
        )}
        </div>

        <div className={`min-w-0 flex flex-col gap-4 ${activeTab === 'batch' ? 'lg:min-h-0' : ''}`}>
          {activeTab === 'batch' && (
            <div className="flex flex-col justify-center gap-5 rounded-2xl border border-slate-200 bg-white px-5 py-4 dark:border-[#222226] dark:bg-[#0b0b10]">
              {(() => {
                const ccy = costEconomics.currency || form.currency || 'USD'
                return (
                  <>
                  {/* Row 1 — auth-rate & realized cost */}
                    <div className="grid grid-cols-3 gap-4 border-t border-slate-100 pt-4 dark:border-[#1c1c24]">
                      <div className="flex flex-col gap-1.5" title="First Attempt Auth Rate — share of decisions charged on the first attempt (first-attempt charges ÷ total decisions).">
                        <StatLabel label="First-attempt auth rate" abbr="FAAR" />
                        <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {authRateStats.faar != null ? `${(authRateStats.faar * 100).toFixed(2)}%` : '—'}
                        </p>
                      </div>
                      <div className="flex flex-col gap-1.5" title="Net Auth Rate — share of decisions charged in the end, counting a successful smart retry (final charges ÷ total decisions). NAR ≥ FAAR when retries recover failures.">
                        <StatLabel label="Net auth rate" abbr="NAR" />
                        <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {authRateStats.nar != null ? `${(authRateStats.nar * 100).toFixed(2)}%` : '—'}
                        </p>
                      </div>
                      <div className="flex flex-col gap-1.5" title="Realized fee saved on charged cost-override decisions: Σ (costSavedBps ÷ 10000) × amount.">
                          <StatLabel label="Realized cost savings" />
                          <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {formatCurrencyValue(totalCostSaved.value, totalCostSaved.currency || ccy)}
                        </p>
                      </div>
                    </div>
                    {/* Row 2 — decision counts */}
                    <div className="grid grid-cols-3 gap-4">
                      <div className="flex min-w-0 flex-col gap-1.5">
                        <StatLabel label="Total decisions" />
                        <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {multiObjectiveStats.total.toLocaleString()}
                        </p>
                      </div>
                      <div className="flex min-w-0 flex-col gap-1.5">
                        <StatLabel label="SR-based decisions" />
                        <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {multiObjectiveStats.srBased.toLocaleString()}
                          <span className="ml-1.5 text-xs font-medium tabular-nums" title="Charged / Failed">
                            <span className="text-slate-400">(</span>
                            <span className="text-emerald-600 dark:text-emerald-400">{multiObjectiveStats.srSuccess.toLocaleString()}</span>
                            <span className="text-slate-400"> / </span>
                            <span className="text-red-500 dark:text-red-400">{multiObjectiveStats.srFailure.toLocaleString()}</span>
                            <span className="text-slate-400">)</span>
                          </span>
                        </p>
                      </div>
                      <div className="flex min-w-0 flex-col gap-1.5">
                        <StatLabel label="Cost-based decisions" />
                        <p className="py-1.5 text-lg font-semibold leading-snug tabular-nums text-sky-600 dark:text-sky-400">
                          {multiObjectiveStats.costWon.toLocaleString()}
                          <span className="ml-1.5 text-xs font-medium tabular-nums" title="Charged / Failed">
                            <span className="text-slate-400">(</span>
                            <span className="text-emerald-600 dark:text-emerald-400">{multiObjectiveStats.costSuccess.toLocaleString()}</span>
                            <span className="text-slate-400"> / </span>
                            <span className="text-red-500 dark:text-red-400">{multiObjectiveStats.costFailure.toLocaleString()}</span>
                            <span className="text-slate-400">)</span>
                          </span>
                        </p>
                      </div>
                    </div>
                  </>
                )
              })()}
            </div>
          )}
          {activeTab === 'debit' ? (
            debitResult ? (
              <>
                <Card>
                  <CardHeader>
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <h3 className="text-sm font-medium text-slate-800 dark:text-white">Debit Routing Result</h3>
                        <p className="mt-1 text-xs text-slate-500 dark:text-[#9ca7ba]">
                          Real response from <code>/decide-gateway</code> using <code>NTW_BASED_ROUTING</code>.
                        </p>
                      </div>
                      {debitPaymentId ? (
                        <Button size="sm" variant="secondary" onClick={() => openAuditModal(debitPaymentId)}>
                          View audit
                        </Button>
                      ) : null}
                    </div>
                  </CardHeader>
                  <CardBody className="space-y-4">
                    <div className="grid grid-cols-2 gap-3">
                      <div className="rounded-lg bg-slate-50 p-3 dark:bg-[#111114]">
                        <p className="text-xs text-slate-500">routing_approach</p>
                        <p className="mt-1 font-semibold text-slate-900 dark:text-white">{debitResult.routing_approach}</p>
                      </div>
                      <div className="rounded-lg bg-slate-50 p-3 dark:bg-[#111114]">
                        <p className="text-xs text-slate-500">request payment_id</p>
                        <p className="mt-1 font-mono text-xs text-slate-900 dark:text-white">{debitPaymentId}</p>
                      </div>
                    </div>

                    {debitResult.debit_routing_output ? (
                      <>
                        <div className="grid grid-cols-3 gap-3">
                          <div className="rounded-lg border border-slate-200 p-3 dark:border-[#222226]">
                            <p className="text-xs text-slate-500">Issuer country</p>
                            <p className="mt-1 text-lg font-semibold text-slate-900 dark:text-white">{debitResult.debit_routing_output.issuer_country}</p>
                          </div>
                          <div className="rounded-lg border border-slate-200 p-3 dark:border-[#222226]">
                            <p className="text-xs text-slate-500">Regulated</p>
                            <p className="mt-1 text-lg font-semibold text-slate-900 dark:text-white">{debitResult.debit_routing_output.is_regulated ? 'Yes' : 'No'}</p>
                          </div>
                          <div className="rounded-lg border border-slate-200 p-3 dark:border-[#222226]">
                            <p className="text-xs text-slate-500">Card type</p>
                            <p className="mt-1 text-lg font-semibold text-slate-900 dark:text-white">{debitResult.debit_routing_output.card_type}</p>
                          </div>
                        </div>

                        <Card>
                          <CardHeader>
                            <h3 className="text-sm font-medium text-slate-800 dark:text-white">Ranked Debit Networks</h3>
                          </CardHeader>
                          <CardBody className="p-0">
                            <table className="w-full text-sm">
                              <thead className="bg-slate-50 text-xs text-slate-500 dark:bg-[#111114]">
                                <tr>
                                  <th className="px-4 py-2 text-left">Rank</th>
                                  <th className="px-4 py-2 text-left">Network</th>
                                  <th className="px-4 py-2 text-right">Saving %</th>
                                </tr>
                              </thead>
                              <tbody className="divide-y divide-slate-100 dark:divide-[#222226]">
                                {debitNetworkRows.map((row, idx) => (
                                  <tr key={`${row.network}-${idx}`} className="hover:bg-slate-50 dark:hover:bg-[#111114]">
                                    <td className="px-4 py-2 font-mono text-xs text-slate-500">#{idx + 1}</td>
                                    <td className="px-4 py-2 font-medium text-slate-900 dark:text-white">{row.network}</td>
                                    <td className="px-4 py-2 text-right text-slate-700 dark:text-[#d8e1ef]">{row.saving_percentage.toFixed(2)}%</td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          </CardBody>
                        </Card>
                      </>
                    ) : (
                      <ErrorMessage error="Debit routing output was not returned. Check the raw response for backend details." />
                    )}

                    <div className="border-t border-slate-200 pt-3 dark:border-[#222226]">
                      <button
                        type="button"
                        onClick={() => setDebitResponseOpen(!debitResponseOpen)}
                        className="flex items-center gap-1 text-xs font-medium text-slate-500 hover:text-slate-700"
                      >
                        {debitResponseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                        Raw response
                      </button>
                      {debitResponseOpen && (
                        <pre className="mt-3 max-h-96 overflow-auto rounded-lg border border-slate-200/80 bg-slate-50/90 p-4 font-mono text-xs leading-6 text-slate-800 shadow-[inset_0_1px_0_rgba(255,255,255,0.75),0_16px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
                          {JSON.stringify(debitResult, null, 2)}
                        </pre>
                      )}
                    </div>
                  </CardBody>
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-12 text-center">
                  <Network size={32} className="mx-auto mb-3 text-slate-300" />
                  <p className="text-sm text-slate-500">Enable debit routing, keep the default debit metadata, and click "Run Debit Routing" to inspect ranked networks.</p>
                </CardBody>
              </Card>
            )
          ) : activeTab === 'volume' ? (
            volumeDistribution.length > 0 ? (
              <div className="space-y-5">
                <section className="rounded-2xl border border-slate-200 bg-white shadow-[0_18px_60px_-46px_rgba(15,23,42,0.2)] dark:border-[#283241] dark:bg-[#101722]">
                  <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 px-5 py-4 dark:border-[#263141]">
                    <div className="min-w-0">
                      <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Distribution analysis</h3>
                      <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1">
                        <span className="text-xs text-slate-500 dark:text-[#8f9bb0]">
                          <span className="font-semibold text-slate-700 dark:text-[#c5d0e0]">{volumeEvaluationCount}</span> evaluations
                        </span>
                        {volumeLeader && (
                          <>
                            <span className="text-slate-300 dark:text-[#3a4455]">·</span>
                            <span className="text-xs text-slate-500 dark:text-[#8f9bb0]">
                              leader <span className="font-semibold text-slate-700 dark:text-[#c5d0e0]">{volumeLeader.name}</span> at{' '}
                              <span className="font-semibold text-slate-700 dark:text-[#c5d0e0]">{volumeLeader.percentage}%</span>
                            </span>
                          </>
                        )}
                      </div>
                    </div>
                    {ruleResult?.payment_id ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => openPreviewModal(ruleResult.payment_id!, 'Volume Split Decision')}
                      >
                        View first trace
                      </Button>
                    ) : null}
                  </div>

                  <div className="space-y-4 px-5 py-4">
                    <div>
                      <div className="flex items-center justify-between text-xs text-slate-500 dark:text-[#8f9bb0]">
                        <span>Observed percentage split</span>
                        <span>{sortedVolumeDistribution.length} gateways</span>
                      </div>
                      <div className="mt-2 flex h-2.5 overflow-hidden rounded-full bg-slate-100 dark:bg-[#1d2633]">
                        {sortedVolumeDistribution.map((item) => (
                          <div
                            key={item.name}
                            className="h-full transition-all duration-300"
                            style={{
                              width: `${item.percentage}%`,
                              backgroundColor: COLORS[(volumeColorIndex.get(item.name) ?? 0) % COLORS.length],
                            }}
                            title={`${item.name}: ${item.percentage}%`}
                          />
                        ))}
                      </div>
                    </div>

                    <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
                      {sortedVolumeDistribution.map((item) => {
                        const color = COLORS[(volumeColorIndex.get(item.name) ?? 0) % COLORS.length]
                        return (
                          <div key={item.name} className="rounded-xl border border-slate-200 bg-slate-50/60 px-4 py-3 dark:border-[#263141] dark:bg-[#0c121c]">
                            <div className="flex items-center justify-between gap-2">
                              <div className="flex min-w-0 items-center gap-2">
                                <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: color }} />
                                <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">{item.name}</p>
                              </div>
                              <p className="shrink-0 text-sm font-semibold text-slate-900 dark:text-white">{item.percentage}%</p>
                            </div>
                            <p className="mt-0.5 pl-[18px] text-xs text-slate-500 dark:text-[#8290a5]">{item.count} payments</p>
                            <div className="mt-2.5 h-1 overflow-hidden rounded-full bg-slate-200 dark:bg-[#1d2633]">
                              <div className="h-full rounded-full" style={{ width: `${item.percentage}%`, backgroundColor: color }} />
                            </div>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                </section>

                <section className="rounded-2xl border border-slate-200 bg-white dark:border-[#283241] dark:bg-[#101722]">
                    <div className="border-b border-slate-200 px-5 py-4 dark:border-[#263141]">
                      <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Evaluation sequence</h3>
                      <p className="mt-1 text-xs text-slate-500 dark:text-[#8f9bb0]">Click any row to inspect the captured decision trace.</p>
                    </div>
                    <div className="max-h-[360px] overflow-auto">
                      <table className="w-full text-sm">
                        <thead className="sticky top-0 bg-slate-50 text-xs text-slate-500 dark:bg-[#0b111a] dark:text-[#8290a5]">
                          <tr>
                            <th className="w-16 px-4 py-2 text-left">#</th>
                            <th className="px-4 py-2 text-left">payment_id</th>
                            <th className="px-4 py-2 text-left">gateway</th>
                            <th className="w-24 px-4 py-2 text-right">trace</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-100 dark:divide-[#263141]">
                          {volumeEvaluationLog.slice(-200).map((entry, idx) => {
                            const absIdx = Math.max(0, volumeEvaluationLog.length - 200) + idx
                            return (
                            <tr
                              key={entry.paymentId}
                              className="cursor-pointer transition hover:bg-slate-50 dark:hover:bg-[#151d2a]"
                              onClick={() => openPreviewModal(entry.paymentId, 'Volume Split Decision')}
                            >
                              <td className="px-4 py-2 font-mono text-xs text-slate-500">{absIdx + 1}</td>
                              <td className="max-w-[260px] truncate px-4 py-2 font-mono text-xs text-slate-600 dark:text-[#aab5c8]">{entry.paymentId}</td>
                              <td className="px-4 py-2">
                                <div className="flex items-center gap-2">
                                  <span
                                    className="h-2 w-2 rounded-full"
                                    style={{ backgroundColor: COLORS[(volumeColorIndex.get(entry.connector) ?? 0) % COLORS.length] }}
                                  />
                                  <span className="font-medium text-slate-900 dark:text-white">{entry.connector}</span>
                                </div>
                              </td>
                              <td className="px-4 py-2 text-right">
                                <button
                                  type="button"
                                  className="text-xs font-semibold text-brand-600 hover:text-brand-700 dark:text-sky-300"
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    openPreviewModal(entry.paymentId, 'Volume Split Decision')
                                  }}
                                >
                                  Open
                                </button>
                              </td>
                            </tr>
                          )})}
                        </tbody>
                      </table>
                    </div>
                  </section>

                <section className="rounded-2xl border border-slate-200 bg-white dark:border-[#283241] dark:bg-[#101722]">
                  <button
                    onClick={() => setVolumeResponseOpen(o => !o)}
                    className="flex w-full items-center justify-between px-5 py-4 text-sm font-semibold text-slate-900 dark:text-white"
                  >
                    <span className="flex items-center gap-2">
                      <Code size={14} />
                      API response
                    </span>
                    {volumeResponseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                  </button>
                  {volumeResponseOpen && ruleResult && (
                    <pre className="max-h-96 overflow-auto border-t border-slate-200 bg-slate-50 p-4 text-xs text-slate-700 dark:border-[#263141] dark:bg-[#070b12] dark:text-[#b7c2d6]">
                      {JSON.stringify(ruleResult, null, 2)}
                    </pre>
                  )}
                </section>
              </div>
            ) : (
              <section className="flex min-h-[360px] items-center justify-center rounded-2xl border border-dashed border-slate-200 bg-slate-50/70 px-8 py-12 text-center dark:border-[#2a303a] dark:bg-[#101722]/70">
                <div className="max-w-sm">
                  <PieChartIcon size={30} className="mx-auto text-slate-300 dark:text-[#536075]" />
                  <h3 className="mt-4 text-sm font-semibold text-slate-900 dark:text-white">No volume results yet</h3>
                  <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-[#9aa6bb]">
                    Set the run size and run the volume split check to view distribution and traces.
                  </p>
                </div>
              </section>
            )
          ) : activeTab === 'rule' ? (
            ruleResult ? (
              <>
                <Card>
                  <CardBody>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">Status</p>
                        <p className="text-2xl font-bold text-slate-900">{ruleResult.status}</p>
                        <p className="text-xs text-slate-500 mt-1">output_type: {ruleResult.output.type}</p>
                      </div>
                      {ruleResult.payment_id ? (
                        <Button
                          size="sm"
                          variant="secondary"
                          onClick={() => openPreviewModal(ruleResult.payment_id!, 'Rule Evaluation Decision')}
                        >
                          View decision trace
                        </Button>
                      ) : null}
                    </div>

                    {ruleResult.output.type === 'single' && ruleResult.output.connector && (
                      <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                        <p className="text-xs text-slate-400 mb-1">Selected gateway_name</p>
                        <p className="text-lg font-semibold">{ruleResult.output.connector.gateway_name}</p>
                        {ruleResult.output.connector.gateway_id && (
                          <p className="text-xs text-slate-500">gateway_id: {ruleResult.output.connector.gateway_id}</p>
                        )}
                      </div>
                    )}

                    {ruleResult.output.type === 'priority' && ruleResult.output.connectors && (
                      <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                        <p className="text-xs text-slate-400 mb-2">Priority gateway_name list</p>
                        <div className="space-y-1">
                          {ruleResult.output.connectors.map((gw, idx) => (
                            <div key={idx} className="flex items-center gap-2 text-sm">
                              <span className="w-5 h-5 rounded-full bg-brand-500 text-white text-xs flex items-center justify-center">{idx + 1}</span>
                              <span className="font-medium">{gw.gateway_name}</span>
                              {gw.gateway_id && <span className="text-xs text-slate-500">({gw.gateway_id})</span>}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {ruleResult.output.type === 'volume_split' && (
                      <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                        <p className="text-xs text-slate-400 mb-2">Volume Split Result</p>
                        <p className="text-sm text-slate-600">See Volume Split tab for detailed visualization.</p>
                      </div>
                    )}
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-slate-800"
                    >
                      <span className="flex items-center gap-2">
                        <Code size={14} />
                        API Response
                      </span>
                      {responseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                    </button>
                  </CardHeader>
                  {responseOpen && (
                    <CardBody className="p-0">
                      <pre className="text-xs text-slate-600 bg-slate-50 dark:bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
                        {JSON.stringify(ruleResult, null, 2)}
                      </pre>
                    </CardBody>
                  )}
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <Play size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Configure rule parameters and click "Evaluate Rules" to test routing.</p>
                </CardBody>
              </Card>
            )
          ) : activeTab === 'batch' ? (
            <>
              {/* Run summary — per-gateway stats lifted above the feed and enlarged.
                  The run progress / hedged / saved line stays on the chart card to
                  avoid duplicating it here. */}
              {hasSimulationActivity && (
                <Card className="lg:shrink-0">
                  <CardHeader className="flex flex-row items-start justify-between gap-3">
                    <div>
                      <h3 className="text-[14px] font-semibold text-slate-500 dark:text-slate-400">Gateway Selection Summary</h3>
                      {/* <p className="text-[13px] text-slate-400 dark:text-slate-500 mt-0.5">Overall success rate &amp; routed share across the run</p> */}
                    </div>
                    <span className="text-[13px] tabular-nums flex items-center gap-1.5 shrink-0">
                      <span className="text-slate-400">{completedSimulationCount} / {totalSimulationPayments || 0}</span>
                      {completedSimulationCount > 0 && hedgingHits > 0 && (
                        <>
                          <span className="text-slate-300 dark:text-slate-600">·</span>
                          <span className="font-medium text-brand-600 dark:text-sky-400">
                            {Math.round((hedgingHits / completedSimulationCount) * 100)}% hedged
                          </span>
                        </>
                      )}
                    </span>
                  </CardHeader>
                  {/* <CardBody className="space-y-3">
                    {sortedGatewayStats.map(([gateway, stats]) => {
                      const share = totalRoutedPayments > 0 ? Math.round((stats.total / totalRoutedPayments) * 100) : 0
                      const srPct = stats.total > 0 ? Math.round((stats.success / stats.total) * 100) : 0
                      const gwColor = gatewayColorMap[gateway] ?? GW_PALETTE[0]
                      return (
                        <div key={gateway} className="flex items-center gap-2.5">
                          <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: gwColor }} />
                          <span className="text-sm font-semibold text-slate-800 dark:text-slate-100 w-20 truncate">{gateway}</span>
                          <span className={`text-lg font-bold tabular-nums leading-none ${srPct >= 80 ? 'text-emerald-600 dark:text-emerald-400' : srPct >= 50 ? 'text-amber-500' : 'text-red-500'}`}>{srPct}%</span>
                          <span className="text-[11px] text-slate-400 dark:text-slate-500">overall SR</span>
                          <span className="ml-auto flex items-center gap-2 text-xs tabular-nums text-slate-500 dark:text-slate-400">
                            <span>{share}% of traffic</span>
                            <span className="text-slate-300 dark:text-slate-600">·</span>
                            <span>
                              <span className="text-emerald-600 dark:text-emerald-400">{stats.success}</span>
                              <span className="text-slate-300 dark:text-slate-600 mx-0.5">/</span>
                              <span className="text-red-400">{stats.failure}</span>
                            </span>
                          </span>
                        </div>
                      )
                    })}
                  </CardBody> */}
                </Card>
              )}
              {/* Autopilot Actions — fills the column now the transaction log moved full-width below. */}
              <Card className="lg:flex-1 lg:min-h-0 lg:flex lg:flex-col">
                <CardHeader className="flex flex-row items-center justify-between gap-3 lg:shrink-0">
                  <span className="flex items-center gap-2.5">
                    <h3 className="text-sm font-medium text-slate-800 dark:text-white">Autopilot Actions</h3>
                  </span>
                  {accumulatedEvents.length > 0 && (
                    <span className="text-xs text-slate-400 tabular-nums">{accumulatedEvents.length} actions</span>
                  )}
                </CardHeader>
                <CardBody className="p-0 lg:flex-1 lg:min-h-0 lg:flex lg:flex-col">
                  {simulationStartedAtMs == null ? (
                    <div className="py-12 text-center">
                      <p className="text-sm text-slate-500 dark:text-slate-400">No events yet</p>
                      <p className="mt-1 text-xs text-slate-400 dark:text-slate-500">
                        Start a simulation — leader changes and auth-band crossings appear here as gateway scores shift.
                      </p>
                    </div>
                  ) : routingEvents.isUnavailable ? (
                    <div className="py-12 text-center">
                      <p className="text-sm text-slate-500 dark:text-slate-400">Events unavailable</p>
                      <p className="mt-1 text-xs text-slate-400 dark:text-slate-500">
                        The analytics pipeline (Kafka → ClickHouse) is offline for this environment.
                      </p>
                    </div>
                  ) : (
                    <div className="max-h-[240px] overflow-y-auto lg:max-h-[360px] lg:flex-1 lg:min-h-0 divide-y divide-slate-100 dark:divide-[#1a1a22]">
                      {collapsedRoutingEvents.map((item) => {
                        if (item.kind === 'leaderFlap') {
                          const isFresh = highlightedEventIds.has(item.latest.id)
                          return (
                            <div
                              key={item.latest.id}
                              className={`flex items-start gap-2.5 px-4 py-2.5 transition-colors duration-1000 ${isFresh ? 'bg-emerald-200 dark:bg-emerald-500/25' : 'bg-transparent'}`}
                            >
                              <div className="mt-0.5 shrink-0">
                                <ArrowRightLeft size={16} className="text-sky-500" />
                              </div>
                              <div className="min-w-0 flex-1">
                                <p className="text-[15px] text-slate-700 dark:text-slate-200">
                                  {item.latest.gateway}
                                  {item.latest.score != null ? ` (${(item.latest.score * 100).toFixed(1)}%)` : ''} now leads on success rate
                                  {item.latest.previous_gateway
                                    ? ` over ${item.latest.previous_gateway}${item.latest.previous_score != null ? ` (${(item.latest.previous_score * 100).toFixed(1)}%)` : ''}`
                                    : ''}
                                  <span className="ml-1.5 rounded-full bg-sky-100 px-1.5 py-0.5 text-[11px] font-medium text-sky-700 dark:bg-sky-900/30 dark:text-sky-300">×{item.crossings}</span>
                                </p>
                                <p className="mt-0.5 text-[13px] text-slate-600 dark:text-slate-400">
                                  now routing to the best PSP · {formatSimEventTime(item.latest.bucket_ms)}
                                </p>
                              </div>
                            </div>
                          )
                        }
                        if (item.kind === 'flap') {
                          const isFresh = highlightedEventIds.has(item.latest.id)
                          return (
                            <div
                              key={item.latest.id}
                              className={`flex items-start gap-2.5 px-4 py-2.5 transition-colors duration-1000 ${isFresh ? 'bg-emerald-200 dark:bg-emerald-500/25' : 'bg-transparent'}`}
                            >
                              <div className="mt-0.5 shrink-0">
                                <RefreshCw size={16} className="text-amber-500" />
                              </div>
                              <div className="min-w-0 flex-1">
                                <p className="text-[15px] text-slate-700 dark:text-slate-200">
                                  {item.gateway} fluctuating at the cost-savings cutoff
                                  <span className="ml-1.5 rounded-full bg-amber-100 px-1.5 py-0.5 text-[11px] font-medium text-amber-700 dark:bg-amber-900/30 dark:text-amber-300">×{item.crossings}</span>
                                </p>
                                <p className="mt-0.5 text-[13px] text-slate-600 dark:text-slate-400">
                                  now {item.inBand ? 'routed to save cost' : 'using top performer only'} · {formatSimEventTime(item.latest.bucket_ms)}
                                </p>
                              </div>
                            </div>
                          )
                        }
                        const event = item.event
                        const meta = SIM_EVENT_META[event.event_type]
                        const Icon = meta?.icon ?? ArrowRightLeft
                        const isFresh = highlightedEventIds.has(event.id)
                        return (
                          <div
                            key={event.id}
                            className={`flex items-start gap-2.5 px-4 py-2.5 transition-colors duration-1000 ${isFresh ? 'bg-emerald-200 dark:bg-emerald-500/25' : 'bg-transparent'}`}
                          >
                            <div className="mt-0.5 shrink-0">
                              <Icon size={16} className={meta?.iconClass ?? 'text-slate-400'} />
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="text-[15px] text-slate-700 dark:text-slate-200">{describeRoutingEvent(event)}</p>
                              <p className="mt-0.5 text-[13px] text-slate-600 dark:text-slate-400">{formatSimEventTime(event.bucket_ms)}</p>
                            </div>
                          </div>
                        )
                      })}
                      {/* Default first event — the start of tracking. It's the oldest
                          entry, so it sits at the bottom of the newest-first feed. */}
                      <div className="flex items-start gap-2.5 px-4 py-2.5 bg-gradient-to-r from-sky-50 via-transparent to-violet-50 dark:from-sky-500/10 dark:to-violet-500/10">
                        <div className="mt-0.5 shrink-0">
                          <Flag size={16} className="text-violet-500" />
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="text-[15px] text-slate-700 dark:text-slate-200">
                            Started tracking{' '}
                            <span className="font-semibold text-emerald-600 dark:text-emerald-400">SR</span>{' '}and{' '}
                            <span className="font-semibold text-violet-600 dark:text-violet-400">Cost</span>{' '}for{' '}
                            {eligibleGatewaysParsed.map((gw, i) => (
                              <span key={gw}>
                                <span className="font-semibold capitalize" style={{ color: gatewayColorMap[gw] ?? GW_PALETTE[0] }}>{gw}</span>
                                {i < eligibleGatewaysParsed.length - 1 ? ', ' : ''}
                              </span>
                            ))}{' '}at CARD, CARD SCHEME, CARD_TYPE, CARD_PROGRAM, AMOUNT combinations
                          </p>
                          {simulationStartedAtMs != null && (
                            <p className="mt-0.5 text-[13px] text-slate-600 dark:text-slate-400">{formatSimEventTime(simulationStartedAtMs)}</p>
                          )}
                        </div>
                      </div>
                    </div>
                  )}
                </CardBody>
              </Card>
              {!hasSimulationActivity && (
              <div className="space-y-3">
                <button
                  type="button"
                  onClick={() => setShowPenaltyGuide(v => !v)}
                  className="flex items-center gap-2 rounded-lg border border-slate-200 dark:border-[#222226] bg-white dark:bg-[#0c0f17] px-3 py-2 text-xs font-medium text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-[#131720] transition-colors w-full"
                >
                  <span className={`transition-transform ${showPenaltyGuide ? 'rotate-90' : ''}`}>▶</span>
                  Penalty Classification Guide
                </button>
                {showPenaltyGuide && (
                  <PenaltyClassificationGuide
                    merchantId={effectiveMerchantId}
                    gsmRules={gsmRules}
                    decideParams={{
                      amount: form.amount,
                      currency: form.currency,
                      paymentMethodType: form.payment_method_type,
                      paymentMethod: form.payment_method,
                      authType: form.auth_type,
                      cardBrand: form.card_brand,
                      rankingAlgorithm: 'SR_BASED_ROUTING',
                      eligibleGateways: form.eligible_gateways,
                    }}
                  />
                )}
              </div>
            )}
            </>
          ) : (
            result ? (
              <>
                <Card>
                  <CardBody>
                    <div className="flex items-start justify-between mb-3">
                      <div>
                        <p className="text-xs text-slate-500 uppercase tracking-wide mb-1">Decided Gateway</p>
                        <p className="text-3xl font-bold text-slate-900">{result.decided_gateway}</p>
                      </div>
                      <div className="text-right space-y-2">
                        <div>
                          <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${approachColor(result.routing_approach)}`}>
                            {result.routing_approach}
                          </span>
                        </div>
                        {singleRunPaymentId ? (
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() => openAuditModal(singleRunPaymentId)}
                          >
                            View audit
                          </Button>
                        ) : null}
                        {result.is_scheduled_outage && <Badge variant="red">Scheduled Outage</Badge>}
                        {singleRunPaymentId ? (
                          <Badge variant={singleRunOutcome === 'CHARGED' ? 'green' : 'red'}>
                            {singleRunOutcome}
                          </Badge>
                        ) : null}
                        {result.latency != null && (
                          <p className="text-xs text-slate-400">{result.latency}ms</p>
                        )}
                      </div>
                    </div>
                    {singleRunPaymentId ? (
                      <div className="mb-3 rounded-[18px] border border-slate-200 bg-slate-50/80 px-4 py-3 dark:border-[#1c1c23] dark:bg-[#0b0b10]">
                        <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                          Payment ID
                        </p>
                        <p className="mt-2 font-mono text-sm text-slate-900 dark:text-white">{singleRunPaymentId}</p>
                        <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                          Feedback recorded as {singleRunOutcome}. Open audit to inspect the full decide and update flow.
                        </p>
                      </div>
                    ) : null}
                    {result.routing_dimension && (
                      <div className="flex gap-4 text-sm text-slate-600 border-t border-slate-200 dark:border-[#1c1c24] pt-3">
                        <div>
                          <span className="text-xs text-slate-400">Dimension</span>
                          <p className="font-medium">{result.routing_dimension}</p>
                        </div>
                        {result.routing_dimension_level && (
                          <div>
                            <span className="text-xs text-slate-400">Level</span>
                            <p className="font-medium">{result.routing_dimension_level}</p>
                          </div>
                        )}
                        <div>
                          <span className="text-xs text-slate-400">Reset</span>
                          <p className="font-medium">{result.reset_approach}</p>
                        </div>
                      </div>
                    )}
                  </CardBody>
                </Card>

                {result.multi_objective_info && (
                  <MultiObjectiveDecisionPanel info={result.multi_objective_info} />
                )}

                {scoreData.length > 0 && (
                  <Card>
                    <CardHeader>
                      <div className="flex items-center justify-between">
                        <h3 className="text-sm font-medium text-slate-800">Gateway Scores</h3>
                        <Button size="sm" variant="ghost" onClick={run} className="text-xs">
                          <RefreshCw size={12} /> Refresh
                        </Button>
                      </div>
                    </CardHeader>
                    <CardBody>
                      <ResponsiveContainer width="100%" height={scoreData.length * 40 + 20}>
                        <BarChart data={scoreData} layout="vertical" margin={{ left: 10, right: 30 }}>
                          <XAxis type="number" domain={[0, 100]} tickFormatter={v => `${v}%`} tick={{ fontSize: 11, fill: '#66667a' }} axisLine={{ stroke: '#1c1c24' }} tickLine={false} />
                          <YAxis type="category" dataKey="name" tick={{ fontSize: 12, fill: '#8e8ea0' }} width={60} axisLine={false} tickLine={false} />
                          <Tooltip
                            formatter={v => `${v}%`}
                            contentStyle={CHART_TOOLTIP_STYLE}
                            labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                            itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                          />
                          <Bar dataKey="score" radius={[0, 4, 4, 0]}>
                            {scoreData.map((entry, i) => (
                              <Cell
                                key={i}
                                fill={
                                  entry.name === result.decided_gateway
                                    ? '#0069ED'
                                    : entry.score < 30 ? '#ef4444'
                                      : entry.score < 60 ? '#f59e0b'
                                        : '#10b981'
                                }
                              />
                            ))}
                          </Bar>
                        </BarChart>
                      </ResponsiveContainer>
                    </CardBody>
                  </Card>
                )}

                {result.filter_wise_gateways && (
                  <Card>
                    <CardHeader>
                      <button
                        onClick={() => setFilterOpen(o => !o)}
                        className="flex items-center justify-between w-full text-sm font-medium text-slate-800"
                      >
                        Filter Chain
                        {filterOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                      </button>
                    </CardHeader>
                    {filterOpen && (
                      <CardBody className="space-y-2">
                        {Object.entries(result.filter_wise_gateways).map(([filter, gateways]) => (
                          <div key={filter} className="flex items-start gap-3">
                            <span className="text-xs font-mono bg-slate-100 dark:bg-[#111118] text-slate-600 rounded-md px-2 py-0.5 mt-0.5 shrink-0 border border-slate-200 dark:border-[#1c1c24]">{filter}</span>
                            <div className="flex flex-wrap gap-1">
                              {Array.isArray(gateways)
                                ? gateways.map(gw => (
                                  <span key={gw} className="text-xs bg-blue-500/10 text-blue-400 ring-1 ring-inset ring-blue-500/20 rounded-md px-2 py-0.5">{gw}</span>
                                ))
                                : <span className="text-xs text-slate-400">—</span>
                              }
                            </div>
                          </div>
                        ))}
                      </CardBody>
                    )}
                  </Card>
                )}

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-slate-800"
                    >
                      <span className="flex items-center gap-2">
                        <Code size={14} />
                        API Response
                      </span>
                      {responseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                    </button>
                  </CardHeader>
                  {responseOpen && (
                    <CardBody className="p-0">
                      <pre className="text-xs text-slate-600 bg-slate-50 dark:bg-[#0a0a0f] p-4 overflow-auto max-h-96 font-mono">
                        {JSON.stringify(result, null, 2)}
                      </pre>
                    </CardBody>
                  )}
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <Play size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Fill in the parameters and click "Run Single Transaction" to decide a gateway, post feedback, and inspect the audit trail.</p>
                </CardBody>
              </Card>
            )
          )}
        </div>
      </div>

      {/* Transaction Log — full-width table at the page bottom. */}
      {activeTab === 'batch' && hasSimulationActivity && (
        <Card className="mt-6">
          <CardHeader className="flex flex-row items-center justify-between gap-3">
            <h3 className="text-sm font-medium text-slate-800 dark:text-white">Transaction Log</h3>
            {deferredSimulationResults.length > 0 && (
              <span className="flex items-center gap-2 text-xs text-slate-400 tabular-nums">
                {txFiltersActive && (
                  <>
                    <button
                      type="button"
                      onClick={() => setTxFilters({})}
                      className="rounded px-1.5 py-0.5 text-[11px] font-medium text-brand-600 hover:bg-brand-50 dark:text-brand-400 dark:hover:bg-brand-900/20"
                    >
                      Clear filters
                    </button>
                    <span className="text-slate-300 dark:text-slate-600">·</span>
                  </>
                )}
                <span>
                  {txFiltersActive
                    ? `${txFilteredRows.length} / ${deferredSimulationResults.length}`
                    : deferredSimulationResults.length}{' '}
                  transactions
                </span>
              </span>
            )}
          </CardHeader>
          <CardBody className="p-0">
            {deferredSimulationResults.length > 0 ? (
              <div ref={txLogRef} className="max-h-[560px] overflow-y-auto overflow-x-auto">
                <table className="w-full text-sm">
                  <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-[11px] text-slate-400 dark:text-slate-500 sticky top-0 border-b border-slate-100 dark:border-[#1c1c24]">
                    <tr>
                      <th className="text-left px-3 py-2 w-8">#</th>
                      <th className="text-left px-3 py-2">Amount</th>
                      <th className="text-left px-3 py-2 whitespace-nowrap w-20">Network</th>
                      <th className="text-left px-3 py-2 whitespace-nowrap ">Program</th>
                      <th className="text-left px-3 py-2 whitespace-nowrap">Gateway</th>
                      <th className="text-right px-3 py-2 whitespace-nowrap w-20">SR Score</th>
                      <th className="text-right px-3 py-2 whitespace-nowrap w-20" title="Expected-value gap between the top-two EV-ranked PSPs (% of ticket) — the decision's margin of victory. Small values mean it was a close call.">EV Δ (top 2)</th>
                      <th className="text-left px-3 py-2">Routing</th>
                      <th className="text-left px-3 py-2">Outcome</th>
                      <th className="text-right px-3 py-2 whitespace-nowrap">Cost Savings</th>
                      {smartRetryEnabled && <th className="text-left px-3 py-2 whitespace-nowrap">Retry Gateway</th>}
                      {smartRetryEnabled && <th className="text-left px-3 py-2">Retry Outcome</th>}
                    </tr>
                    {(() => {
                      const inputCls = 'w-full rounded border border-slate-200 bg-white px-1.5 py-1 text-[11px] font-normal text-slate-700 placeholder:text-slate-300 focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-200 dark:placeholder:text-slate-600'
                      const setF = (key: string, value: string) => setTxFilters(prev => ({ ...prev, [key]: value }))
                      const sel = (key: string, opts: string[], placeholder: string) => (
                        <select value={txFilters[key] ?? ''} onChange={e => setF(key, e.target.value)} className={inputCls}>
                          <option value="">{placeholder}</option>
                          {opts.map(o => <option key={o} value={o}>{o}</option>)}
                        </select>
                      )
                      return (
                        <tr className="bg-white dark:bg-[#0a0a0f] border-b border-slate-100 dark:border-[#1c1c24]">
                          <th className="px-2 py-1.5" />
                          <th className="px-2 py-1.5">
                            <input value={txFilters.amount ?? ''} onChange={e => setF('amount', e.target.value)} placeholder="search" className={inputCls} />
                          </th>
                          <th className="px-2 py-1.5">{sel('network', txFilterOptions.networks, 'All')}</th>
                          <th className="px-2 py-1.5">{sel('program', txFilterOptions.programs, 'All')}</th>
                          <th className="px-2 py-1.5">{sel('gateway', txFilterOptions.gateways, 'All')}</th>
                          <th className="px-2 py-1.5">
                            <input value={txFilters.sr ?? ''} onChange={e => setF('sr', e.target.value)} placeholder="e.g. 90" className={inputCls} />
                          </th>
                          <th className="px-2 py-1.5">
                            <input value={txFilters.evGap ?? ''} onChange={e => setF('evGap', e.target.value)} placeholder="e.g. 0.12" className={inputCls} />
                          </th>
                          <th className="px-2 py-1.5">{sel('routing', txFilterOptions.routings, 'All')}</th>
                          <th className="px-2 py-1.5">{sel('outcome', txFilterOptions.outcomes, 'All')}</th>
                          <th className="px-2 py-1.5">
                            <select value={txFilters.cost ?? ''} onChange={e => setF('cost', e.target.value)} className={inputCls}>
                              <option value="">All</option>
                              <option value="yes">Has savings</option>
                              <option value="no">None</option>
                            </select>
                          </th>
                          {smartRetryEnabled && <th className="px-2 py-1.5">{sel('retryGateway', txFilterOptions.retryGateways, 'All')}</th>}
                          {smartRetryEnabled && <th className="px-2 py-1.5">{sel('retryOutcome', txFilterOptions.retryOutcomes, 'All')}</th>}
                        </tr>
                      )
                    })()}
                  </thead>
                  <tbody className="divide-y divide-slate-100 dark:divide-[#1a1a22]">
                    {txFilteredRows.map(({ res, idx }) => (
                      <tr
                        key={res.paymentId}
                        className="group cursor-pointer hover:bg-slate-50 dark:hover:bg-[#0d0d14] transition-colors"
                        onClick={() => openAuditModal(res.paymentId)}
                      >
                        <td className="px-3 py-2 text-[11px] text-slate-400 tabular-nums">{idx + 1}</td>
                        <td className="px-3 py-2 whitespace-nowrap">
                          <span className="block font-mono text-xs text-slate-700 dark:text-slate-300 tabular-nums group-hover:text-brand-600 dark:group-hover:text-brand-400 transition-colors">
                            {formatCurrencyValue(res.amount, res.currency)}
                          </span>
                        </td>
                        <td className="px-3 py-2 text-xs text-slate-600 dark:text-slate-300 whitespace-nowrap">{res.cardNetwork ?? '—'}</td>
                        <td className="px-3 py-2 text-xs text-slate-600 dark:text-slate-300 whitespace-nowrap">{res.cardProgram ?? '—'}</td>
                        <td className="px-3 py-2 text-xs font-medium text-slate-600 dark:text-slate-300 whitespace-nowrap">{res.decidedGateway}</td>
                        <td className="px-3 py-2 text-right whitespace-nowrap">
                          {(() => {
                            const srScore = res.gatewayPriorityMap?.[res.decidedGateway]
                            return typeof srScore === 'number' ? (
                              <span className="font-mono text-xs text-slate-600 dark:text-slate-300 tabular-nums">
                                {(srScore * 100).toFixed(1)}%
                              </span>
                            ) : (
                              <span className="text-[11px] text-slate-400">—</span>
                            )
                          })()}
                        </td>
                        <td className="px-3 py-2 text-right whitespace-nowrap">
                          {res.evGapTop2 != null ? (
                            <span className="font-mono text-xs text-slate-600 dark:text-slate-300 tabular-nums">
                              {(res.evGapTop2 * 100).toFixed(2)}%
                            </span>
                          ) : (
                            <span className="text-[11px] text-slate-400">—</span>
                          )}
                        </td>
                        <td className="px-3 py-2">
                          {res.routingApproach?.includes('HEDGING') ? (
                            <span className="inline-flex items-center rounded-full bg-amber-50 px-2 py-0.5 text-[11px] font-medium text-amber-700 ring-1 ring-inset ring-amber-200 dark:bg-amber-900/20 dark:text-amber-300 dark:ring-amber-800">Hedging</span>
                          ) : res.routingApproach === 'SR_SELECTION_MULTI_OBJECTIVE' ? (
                            <span className="inline-flex items-center rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 ring-1 ring-inset ring-emerald-200 dark:bg-emerald-900/20 dark:text-emerald-300 dark:ring-emerald-800">Cost Based</span>
                          ) : res.routingApproach === 'SR_SELECTION_V3_ROUTING' ? (
                            <span className="inline-flex items-center rounded-full bg-brand-50 px-2 py-0.5 text-[11px] font-medium text-brand-700 ring-1 ring-inset ring-brand-200 dark:bg-brand-900/20 dark:text-brand-300 dark:ring-brand-800">Auth Based</span>
                          ) : (
                            <span className="text-[11px] text-slate-400">{res.routingApproach ?? '—'}</span>
                          )}
                        </td>
                        <td className="px-3 py-2">
                          <span className={`text-xs font-semibold ${res.status === 'CHARGED' ? 'text-emerald-600 dark:text-emerald-400' : res.status === 'PENDING_VBV' ? 'text-amber-600 dark:text-amber-400' : 'text-red-500 dark:text-red-400'}`}>{res.status}</span>
                        </td>
                        <td className="px-3 py-2 text-right whitespace-nowrap">
                          {res.costWon && res.costSavedBps != null && res.costSavedBps > 0 && res.status === 'CHARGED' ? (
                            <span className="font-mono text-xs text-emerald-700 dark:text-emerald-400 tabular-nums">
                              {formatSavingsCurrency(res.costSavedBps, res.amount, res.currency)}
                            </span>
                          ) : null}
                        </td>
                        {smartRetryEnabled && (
                          <td className="px-3 py-2 text-xs text-slate-500 dark:text-slate-400 whitespace-nowrap">
                            {res.retryGateway ?? '—'}
                          </td>
                        )}
                        {smartRetryEnabled && (
                          <td className="px-3 py-2">
                            {res.retryStatus ? (
                              <Badge variant={res.retryStatus === 'CHARGED' ? 'green' : res.retryStatus === 'PENDING_VBV' ? 'orange' : 'red'}>
                                {res.retryStatus}
                              </Badge>
                            ) : <span className="text-xs text-slate-400">—</span>}
                          </td>
                        )}
                      </tr>
                    ))}
                    {txFilteredRows.length === 0 && (
                      <tr>
                        <td colSpan={smartRetryEnabled ? 12 : 10} className="px-3 py-8 text-center text-sm text-slate-400 dark:text-slate-500">
                          No transactions match the current filters.
                        </td>
                      </tr>
                    )}
                  </tbody>
                  {txFilteredRows.length > 0 && (
                    <tfoot className="sticky bottom-0 z-10 bg-slate-50 dark:bg-[#0a0a0f] border-t border-slate-200 dark:border-[#1c1c24]">
                      <tr className="text-xs font-semibold text-slate-700 dark:text-slate-200">
                        <td className="px-3 py-2 text-slate-400">Σ</td>
                        <td className="px-3 py-2 whitespace-nowrap tabular-nums">{formatCurrencyValue(txColumnTotals.amount, txColumnTotals.currency)}</td>
                        <td className="px-3 py-2" />
                        <td className="px-3 py-2" />
                        <td className="px-3 py-2 whitespace-nowrap font-normal text-slate-400">{txColumnTotals.count.toLocaleString()} rows</td>
                        <td className="px-3 py-2" />
                        <td className="px-3 py-2 text-right whitespace-nowrap tabular-nums font-normal text-slate-400" title="Average EV margin of victory across these rows">
                          {txColumnTotals.evGapPctAvg != null ? `${txColumnTotals.evGapPctAvg.toFixed(2)}% avg` : ''}
                        </td>
                        <td className="px-3 py-2" />
                        <td className="px-3 py-2" />
                        <td className="px-3 py-2 text-right whitespace-nowrap tabular-nums text-emerald-700 dark:text-emerald-400">{formatCurrencyValue(txColumnTotals.savings, txColumnTotals.currency)}</td>
                        {smartRetryEnabled && <td className="px-3 py-2" />}
                        {smartRetryEnabled && <td className="px-3 py-2" />}
                      </tr>
                    </tfoot>
                  )}
                </table>
              </div>
            ) : (
              <div className="flex items-center gap-3 px-4 py-6 text-sm text-slate-500">
                <Spinner size={16} />
                Waiting for the first simulated payment result…
              </div>
            )}
          </CardBody>
        </Card>
      )}

      {setupPrompt && (
        <div className="fixed bottom-0 left-0 right-0 top-[76px] z-[140] flex items-center justify-center p-4">
          <button
            type="button"
            aria-label="Close setup prompt"
            className="absolute inset-0 bg-slate-950/70 backdrop-blur-sm"
            onClick={() => setSetupPrompt(null)}
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="decision-explorer-setup-title"
            className="relative w-full max-w-[440px] overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-2xl dark:border-[#283241] dark:bg-[#101722]"
          >
            <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-sky-400/50 to-transparent" />
            <div className="p-6">
              <div className="flex items-start justify-between gap-4">
                <div className="flex items-start gap-4">
                  <div className="rounded-2xl border border-sky-500/20 bg-sky-500/10 p-3 text-sky-500 dark:text-sky-300">
                    <Settings size={22} />
                  </div>
                  <div>
                    <SurfaceLabel>Setup required</SurfaceLabel>
                    <h2 id="decision-explorer-setup-title" className="mt-2 text-2xl font-semibold tracking-tight text-slate-950 dark:text-white">
                      {setupPrompt.title}
                    </h2>
                  </div>
                </div>
                <button
                  type="button"
                  aria-label="Close setup prompt"
                  className="rounded-full p-2 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-[#1a2230] dark:hover:text-white"
                  onClick={() => setSetupPrompt(null)}
                >
                  <X size={16} />
                </button>
              </div>

              <p className="mt-4 text-sm leading-6 text-slate-500 dark:text-[#9aa6bb]">
                {setupPrompt.body}
              </p>

              {setupPrompt.detail ? (
                <div className="mt-4 rounded-xl border border-slate-200 bg-slate-50 px-3 py-3 font-mono text-xs leading-5 text-slate-500 dark:border-[#283241] dark:bg-[#0b111a] dark:text-[#9aa6bb]">
                  {setupPrompt.detail}
                </div>
              ) : null}

              <div className="mt-6 flex flex-wrap justify-end gap-2">
                <Button variant="secondary" onClick={() => setSetupPrompt(null)}>
                  Dismiss
                </Button>
                {setupPrompt.configurePath && (
                  <Button onClick={() => { setSetupPrompt(null); navigate(setupPrompt.configurePath!) }}>
                    Configure
                  </Button>
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {selectedAuditPaymentId && (
        <div className="fixed bottom-0 left-64 right-0 top-[76px] z-[130] p-8">
          <button
            type="button"
            aria-label="Close payment audit"
            className="absolute inset-0 bg-slate-950/70 backdrop-blur-sm"
            onClick={closeAuditModal}
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="decision-explorer-audit-title"
            className="relative mx-auto flex h-full w-full max-w-7xl flex-col overflow-hidden rounded-[30px] border border-slate-200 bg-white shadow-2xl dark:border-[#1c1c23] dark:bg-[#09090d]"
          >
            <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 bg-slate-50/90 px-6 py-5 dark:border-[#1c1c23] dark:bg-[#0b0b10]">
              <div className="min-w-0">
                <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-slate-500 dark:text-[#8a8a93]">
                  Simulation Audit
                </p>
                <h2
                  id="decision-explorer-audit-title"
                  className="mt-2 truncate text-2xl font-semibold text-slate-900 dark:text-white"
                >
                  {selectedAuditPaymentId}
                </h2>
                <p className="mt-2 max-w-3xl text-sm text-slate-500 dark:text-[#8a8a93]">
                  Inspect the exact decision trail for this simulated payment, including request payloads, API responses, score context, and the final transaction outcome.
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {auditSummary?.latest_gateway ? <Badge variant="green">{auditSummary.latest_gateway}</Badge> : null}
                {auditSummary?.latest_status ? (
                  <Badge variant={summaryBadgeVariant(auditSummary.latest_status)}>
                    {humanizeAuditValue(auditSummary.latest_status)}
                  </Badge>
                ) : null}
                {auditSummary?.event_count ? <Badge variant="gray">{auditSummary.event_count} events</Badge> : null}
                <Button size="sm" variant="secondary" onClick={() => auditDetail.mutate()}>
                  <RefreshCw size={12} />
                  Refresh
                </Button>
                <Button size="sm" variant="ghost" onClick={closeAuditModal}>
                  <X size={14} />
                  Close
                </Button>
              </div>
            </div>

            <div className="grid min-h-0 flex-1 gap-0 xl:grid-cols-[340px_minmax(0,1fr)]">
              <div className="flex min-h-0 flex-col border-b border-slate-200 bg-slate-50/70 xl:border-b-0 xl:border-r dark:border-[#1c1c23] dark:bg-[#08080b]">
                <div className="border-b border-slate-200 px-6 py-4 dark:border-[#1c1c23]">
                  <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Audit Timeline</h3>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    Choose a step to inspect its request, response, and scoring context.
                  </p>
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
                  {auditDetail.isLoading && !auditDetail.data ? (
                    <div className="flex items-center gap-2 px-2 text-sm text-slate-500 dark:text-[#8a8a93]">
                      <Spinner size={16} />
                      Loading payment audit…
                    </div>
                  ) : auditDetail.error ? (
                    <ErrorMessage error={auditDetail.error.message} />
                  ) : groupedAuditTimeline.length ? (
                    <div className="space-y-4">
                      {groupedAuditTimeline.map((group) => (
                        <section key={group.phase} className="space-y-2">
                          <div className="px-2">
                            <Badge variant={phaseBadgeVariant(group.phase)}>{group.phase}</Badge>
                          </div>
                          <div className="space-y-2">
                            {group.events.map((event) => (
                              <button
                                key={event.id}
                                type="button"
                                onClick={() => {
                                  setSelectedAuditEventId(event.id)
                                  setAuditInspectorTab('summary')
                                }}
                                className={`w-full rounded-[22px] border px-4 py-3 text-left transition ${
                                  selectedAuditEvent?.id === event.id
                                    ? 'border-brand-500/50 bg-brand-500/8'
                                    : 'border-slate-200 bg-white hover:border-slate-300 dark:border-[#1d1d23] dark:bg-[#0c0c10] dark:hover:border-[#2a2a31]'
                                }`}
                              >
                                <div className="flex items-start justify-between gap-3">
                                  <div className="min-w-0">
                                    <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                                      {stageLabel(event)}
                                    </p>
                                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                                      {formatDateTime(event.created_at_ms)}
                                    </p>
                                  </div>
                                  <Badge variant={badgeVariantForEvent(event)}>
                                    {humanizeAuditValue(event.status) || eventTypeLabel(event.flow_type)}
                                  </Badge>
                                </div>
                                <div className="mt-3 flex flex-wrap gap-2">
                                  <Badge variant="gray">{routeLabel(event.route)}</Badge>
                                  {event.gateway ? <Badge variant="green">{event.gateway}</Badge> : null}
                                  {event.request_id ? <Badge variant="blue">Request</Badge> : null}
                                </div>
                              </button>
                            ))}
                          </div>
                        </section>
                      ))}
                    </div>
                  ) : (
                    <EmptyAuditState
                      title="No audit trail captured yet"
                      body="Run a simulated payment and gateway update first, then reopen the row once the audit payload is available."
                    />
                  )}
                </div>
              </div>

              <div className="flex min-h-0 flex-col">
                <div className="border-b border-slate-200 px-6 py-4 dark:border-[#1c1c23]">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
                        {selectedAuditEvent ? stageLabel(selectedAuditEvent) : 'Audit Inspector'}
                      </h3>
                      <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                        {selectedAuditEvent
                          ? `${routeLabel(selectedAuditEvent.route)} · ${formatDateTime(selectedAuditEvent.created_at_ms)}`
                          : 'Select an event from the left to inspect payloads.'}
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {selectedAuditEvent?.gateway ? <Badge variant="green">{selectedAuditEvent.gateway}</Badge> : null}
                      {selectedAuditEvent?.status ? (
                        <Badge variant={badgeVariantForEvent(selectedAuditEvent)}>
                          {humanizeAuditValue(selectedAuditEvent.status)}
                        </Badge>
                      ) : null}
                    </div>
                  </div>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {(['summary', 'input', 'response', 'raw'] as AuditInspectorTab[]).map((tab) => (
                      <button
                        key={tab}
                        type="button"
                        onClick={() => setAuditInspectorTab(tab)}
                        className={`rounded-full px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] transition ${sectionButtonClass(auditInspectorTab === tab)}`}
                      >
                        {tab === 'raw' ? 'Raw JSON' : humanizeAuditValue(tab)}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
                  {auditDetail.isLoading && !auditDetail.data ? (
                    <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
                      <Spinner size={16} />
                      Loading inspector…
                    </div>
                  ) : auditInspectorModel ? (
                    <div className="space-y-5">
                      {auditInspectorTab === 'summary' ? (
                        <>
                          <InspectorKeyValueGrid rows={auditInspectorModel.summaryRows} />
                          {auditInspectorModel.selectionReason ? (
                            <div className="rounded-[22px] border border-slate-200 bg-slate-50/80 px-5 py-4 dark:border-[#1d1d23] dark:bg-[#0b0b10]">
                              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                                Selection Reason
                              </p>
                              <p className="mt-3 text-sm leading-6 text-slate-700 dark:text-slate-200">
                                {stringifyValue(auditInspectorModel.selectionReason)}
                              </p>
                            </div>
                          ) : null}
                          <InspectorJsonPanel
                            title="Score Context"
                            value={auditInspectorModel.scoreContext}
                            emptyMessage="No scoring context was captured for this event."
                          />
                          {auditInspectorModel.signalRecord ? (
                            <InspectorJsonPanel
                              title="Additional Signals"
                              value={auditInspectorModel.signalRecord}
                              emptyMessage="No additional signals were captured for this event."
                            />
                          ) : null}
                        </>
                      ) : null}

                      {auditInspectorTab === 'input' ? (
                        <InspectorJsonPanel
                          title="Request Payload"
                          value={auditInspectorModel.requestPayload}
                          emptyMessage="This step did not persist a request payload."
                        />
                      ) : null}

                      {auditInspectorTab === 'response' ? (
                        <InspectorJsonPanel
                          title="Response Payload"
                          value={auditInspectorModel.responsePayload}
                          emptyMessage="This step did not persist a response payload."
                        />
                      ) : null}

                      {auditInspectorTab === 'raw' ? (
                        <InspectorJsonPanel
                          title="Raw Event JSON"
                          value={auditInspectorModel.rawEvent}
                          emptyMessage="No raw event payload is available."
                        />
                      ) : null}
                    </div>
                  ) : (
                    <EmptyAuditState
                      title="Select a timeline step"
                      body="Choose one of the audit events on the left to inspect its request, response, and score context."
                    />
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {selectedPreviewPaymentId && (
        <div className="fixed bottom-0 left-64 right-0 top-[76px] z-[130] p-8">
          <button
            type="button"
            aria-label="Close decision trace"
            className="absolute inset-0 bg-slate-950/70 backdrop-blur-sm"
            onClick={closePreviewModal}
          />
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby="decision-explorer-preview-title"
            className="relative mx-auto flex h-full w-full max-w-7xl flex-col overflow-hidden rounded-[30px] border border-slate-200 bg-white shadow-2xl dark:border-[#1c1c23] dark:bg-[#09090d]"
          >
            <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 bg-slate-50/90 px-6 py-5 dark:border-[#1c1c23] dark:bg-[#0b0b10]">
              <div className="min-w-0">
                <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-slate-500 dark:text-[#8a8a93]">
                  Decision Trace
                </p>
                <h2
                  id="decision-explorer-preview-title"
                  className="mt-2 truncate text-2xl font-semibold text-slate-900 dark:text-white"
                >
                  {selectedPreviewPaymentId}
                </h2>
                <p className="mt-2 max-w-3xl text-sm text-slate-500 dark:text-[#8a8a93]">
                  {previewTraceLabel}. This trace was captured from <code className="font-mono text-xs">/routing/evaluate</code> and is kept separate from auth-rate transaction outcomes.
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {previewSummary?.latest_gateway ? <Badge variant="green">{previewSummary.latest_gateway}</Badge> : null}
                {previewSummary?.latest_status ? (
                  <Badge variant={summaryBadgeVariant(previewSummary.latest_status)}>
                    {humanizeAuditValue(previewSummary.latest_status)}
                  </Badge>
                ) : null}
                {previewSummary?.event_count ? <Badge variant="gray">{previewSummary.event_count} events</Badge> : null}
                <Button size="sm" variant="secondary" onClick={() => previewTraceDetail.mutate()}>
                  <RefreshCw size={12} />
                  Refresh
                </Button>
                <Button size="sm" variant="ghost" onClick={closePreviewModal}>
                  <X size={14} />
                  Close
                </Button>
              </div>
            </div>

            <div className="grid min-h-0 flex-1 gap-0 xl:grid-cols-[340px_minmax(0,1fr)]">
              <div className="flex min-h-0 flex-col border-b border-slate-200 bg-slate-50/70 xl:border-b-0 xl:border-r dark:border-[#1c1c23] dark:bg-[#08080b]">
                <div className="border-b border-slate-200 px-6 py-4 dark:border-[#1c1c23]">
                  <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Decision Timeline</h3>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    Choose a decision step to inspect its request, response, and routing output.
                  </p>
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
                  {previewTraceDetail.isLoading && !previewTraceDetail.data ? (
                    <div className="flex items-center gap-2 px-2 text-sm text-slate-500 dark:text-[#8a8a93]">
                      <Spinner size={16} />
                      Loading decision trace…
                    </div>
                  ) : previewTraceDetail.error && isTraceIndexingError(previewTraceDetail.error) && selectedPreviewPaymentId ? (
                    <PendingAuditState
                      title="Decision trace still indexing"
                      body="The routing decision succeeded, but the trace row is still being processed. The modal will update once the analytics event is available."
                    />
                  ) : previewTraceDetail.error ? (
                    <ErrorMessage error={previewTraceDetail.error.message} />
                  ) : groupedPreviewTimeline.length ? (
                    <div className="space-y-4">
                      {groupedPreviewTimeline.map((group) => (
                        <section key={group.phase} className="space-y-2">
                          <div className="px-2">
                            <Badge variant={phaseBadgeVariant(group.phase)}>{group.phase}</Badge>
                          </div>
                          <div className="space-y-2">
                            {group.events.map((event) => (
                              <button
                                key={event.id}
                                type="button"
                                onClick={() => {
                                  setSelectedPreviewEventId(event.id)
                                  setPreviewInspectorTab('summary')
                                }}
                                className={`w-full rounded-[22px] border px-4 py-3 text-left transition ${
                                  selectedPreviewEvent?.id === event.id
                                    ? 'border-brand-500/50 bg-brand-500/8'
                                    : 'border-slate-200 bg-white hover:border-slate-300 dark:border-[#1d1d23] dark:bg-[#0c0c10] dark:hover:border-[#2a2a31]'
                                }`}
                              >
                                <div className="flex items-start justify-between gap-3">
                                  <div className="min-w-0">
                                    <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                                      {stageLabel(event)}
                                    </p>
                                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                                      {formatDateTime(event.created_at_ms)}
                                    </p>
                                  </div>
                                  <Badge variant={badgeVariantForEvent(event)}>
                                    {humanizeAuditValue(event.status) || eventTypeLabel(event.flow_type)}
                                  </Badge>
                                </div>
                                <div className="mt-3 flex flex-wrap gap-2">
                                  <Badge variant="gray">{routeLabel(event.route)}</Badge>
                                  {event.gateway ? <Badge variant="green">{event.gateway}</Badge> : null}
                                </div>
                              </button>
                            ))}
                          </div>
                        </section>
                      ))}
                    </div>
                  ) : selectedPreviewPaymentId ? (
                    <PendingAuditState
                      title={
                        previewSummary
                          ? 'Decision summary available'
                          : 'Decision trace still arriving'
                      }
                      body={
                        previewSummary
                          ? 'The decision summary is available. The step-by-step timeline will appear as soon as the latest events are ready.'
                          : 'This decision was just logged. Waiting for the decision trace details to become available.'
                      }
                    />
                  ) : (
                    <EmptyAuditState
                      title="No decision trace captured yet"
                      body="Run Rule-Based or Volume Split evaluation first, then open the decision trace once the request has been logged."
                    />
                  )}
                </div>
              </div>

              <div className="flex min-h-0 flex-col">
                <div className="border-b border-slate-200 px-6 py-4 dark:border-[#1c1c23]">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
                        {selectedPreviewEvent ? stageLabel(selectedPreviewEvent) : 'Decision Inspector'}
                      </h3>
                      <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                        {selectedPreviewEvent
                          ? `${routeLabel(selectedPreviewEvent.route)} · ${formatDateTime(selectedPreviewEvent.created_at_ms)}`
                          : 'Select an event from the left to inspect the decision payload.'}
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {selectedPreviewEvent?.gateway ? <Badge variant="green">{selectedPreviewEvent.gateway}</Badge> : null}
                      {selectedPreviewEvent?.status ? (
                        <Badge variant={badgeVariantForEvent(selectedPreviewEvent)}>
                          {humanizeAuditValue(selectedPreviewEvent.status)}
                        </Badge>
                      ) : null}
                    </div>
                  </div>
                  <div className="mt-4 flex flex-wrap gap-2">
                    {(['summary', 'input', 'response', 'raw'] as AuditInspectorTab[]).map((tab) => (
                      <button
                        key={tab}
                        type="button"
                        onClick={() => setPreviewInspectorTab(tab)}
                        className={`rounded-full px-4 py-2 text-xs font-semibold uppercase tracking-[0.16em] transition ${sectionButtonClass(previewInspectorTab === tab)}`}
                      >
                        {tab === 'raw' ? 'Raw JSON' : humanizeAuditValue(tab)}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
                  {previewTraceDetail.isLoading && !previewTraceDetail.data ? (
                    <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
                      <Spinner size={16} />
                      Loading decision inspector…
                    </div>
                  ) : previewInspectorModel ? (
                    <div className="space-y-5">
                      {previewInspectorTab === 'summary' ? (
                        <>
                          <InspectorKeyValueGrid rows={previewInspectorModel.summaryRows} />
                          <InspectorJsonPanel
                            title="Decision Signals"
                            value={previewInspectorModel.signalRecord}
                            emptyMessage="No extra decision metadata was captured for this evaluation."
                          />
                        </>
                      ) : null}

                      {previewInspectorTab === 'input' ? (
                        <InspectorJsonPanel
                          title="Request Payload"
                          value={previewInspectorModel.requestPayload}
                          emptyMessage="No request payload was captured for this decision."
                        />
                      ) : null}

                      {previewInspectorTab === 'response' ? (
                        <InspectorJsonPanel
                          title="Response Payload"
                          value={previewInspectorModel.responsePayload}
                          emptyMessage="No response payload was captured for this decision."
                        />
                      ) : null}

                      {previewInspectorTab === 'raw' ? (
                        <InspectorJsonPanel
                          title="Raw Event JSON"
                          value={previewInspectorModel.rawEvent}
                          emptyMessage="No raw event payload is available for this decision."
                        />
                      ) : null}
                    </div>
                  ) : selectedPreviewPaymentId && !(previewTraceDetail.data?.timeline?.length || 0) ? (
                    <PendingAuditState
                      title={
                        previewSummary
                          ? 'Waiting for detailed decision step'
                          : 'Waiting for decision step'
                      }
                      body={
                        previewSummary
                          ? 'The decision record is available. Request and response payloads will appear as soon as the first timeline event is ready.'
                          : 'Request and response payloads will appear as soon as the first decision event is ready.'
                      }
                    />
                  ) : (
                    <EmptyAuditState
                      title="Select a decision step"
                      body="Choose one of the decision events on the left to inspect its request and response payload."
                    />
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function MultiObjectiveDecisionPanel({ info }: { info: MultiObjectiveInfo }) {
  const isCostWin = info.outcome === 'COST_WON'
  const tone = isCostWin
    ? 'border-cyan-200 bg-cyan-50/60 dark:border-cyan-900 dark:bg-cyan-950/30'
    : 'border-slate-200 bg-slate-50/60 dark:border-[#1c1c24] dark:bg-[#0b0b10]'
  const pillTone = isCostWin
    ? 'bg-cyan-100 text-cyan-800 dark:bg-cyan-900/40 dark:text-cyan-200'
    : 'bg-slate-200 text-slate-700 dark:bg-[#1f1f29] dark:text-slate-200'
  return (
    <Card>
      <CardBody>
        <div className={`rounded-2xl border px-4 py-3 ${tone}`}>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <span className="text-[11px] uppercase tracking-[0.16em] text-slate-500 dark:text-slate-400">
                Multi-Objective Decision
              </span>
              <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${pillTone}`}>
                {isCostWin ? 'Cost won' : 'Auth won'}
              </span>
            </div>
          </div>

          <p className="mt-3 text-sm text-slate-700 dark:text-slate-200 leading-relaxed">
            {info.reason}
          </p>

          {isCostWin && info.costSavedBps != null && (
            <div className="mt-3 inline-flex items-center gap-2 rounded-full bg-emerald-100 px-3 py-1 text-xs font-semibold text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">
              <span>Cost saved</span>
              <span className="font-mono">{info.costSavedBps.toFixed(2)} bps</span>
            </div>
          )}

          {(info.srHead || info.chosen) && (
            <div className="mt-4 grid gap-3 sm:grid-cols-2">
              {info.srHead && (
                <MultiObjectivePspCard
                  label={isCostWin ? 'SR head (would have picked)' : 'SR head (kept)'}
                  summary={info.srHead}
                />
              )}
              {info.chosen && (
                <MultiObjectivePspCard
                  label={isCostWin ? 'Chosen by EV' : 'Final pick'}
                  summary={info.chosen}
                  emphasis={isCostWin}
                />
              )}
            </div>
          )}

          <p className="mt-3 text-[11px] text-slate-500 dark:text-slate-400">
            {info.qualifiedCount} PSP{info.qualifiedCount === 1 ? '' : 's'} ranked on EV.
          </p>
        </div>
      </CardBody>
    </Card>
  )
}

function MultiObjectivePspCard({
  label,
  summary,
  emphasis = false,
}: {
  label: string
  summary: { psp: string; authRate: number; costBps: number | null }
  emphasis?: boolean
}) {
  const borderTone = emphasis
    ? 'border-cyan-300 bg-white dark:border-cyan-700 dark:bg-[#0d141a]'
    : 'border-slate-200 bg-white dark:border-[#1c1c24] dark:bg-[#0d0d13]'
  return (
    <div className={`rounded-xl border px-3 py-2 ${borderTone}`}>
      <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-slate-500 dark:text-slate-400">
        {label}
      </p>
      <p className="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-white">
        {summary.psp}
      </p>
      <div className="mt-1.5 flex gap-3 text-xs text-slate-600 dark:text-slate-300">
        <span>
          <span className="text-slate-400">auth</span>{' '}
          <span className="font-mono">{(summary.authRate * 100).toFixed(2)}%</span>
        </span>
        <span>
          <span className="text-slate-400">cost</span>{' '}
          <span className="font-mono">
            {summary.costBps != null ? `${summary.costBps.toFixed(2)} bps` : '—'}
          </span>
        </span>
      </div>
    </div>
  )
}
