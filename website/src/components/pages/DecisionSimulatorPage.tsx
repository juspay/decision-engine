import { useDeferredValue, useEffect, useMemo, useRef, useState } from 'react'
import { RuleEvaluationPanel } from './RuleEvaluationPanel'
import { ErrorInfoFields, ErrorInfoState, GsmOptionRow, DEFAULT_ERROR_INFO } from './ErrorInfoFields'
import { PenaltyClassificationGuide } from './PenaltyClassificationGuide'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import { BarChart, Bar, LineChart, Line, ComposedChart, Area, CartesianGrid, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, ReferenceLine, ReferenceArea } from 'recharts'
import { Tooltip as UiTooltip } from '../ui/Tooltip'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import { CHART_TOOLTIP_ITEM_STYLE, CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import { DecideGatewayResponse, GatewayConnector, MultiObjectiveInfo, PaymentAuditEvent, PaymentAuditResponse, RoutingEventType, UpdateScoreResponse } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'
import { describeRoutingEvent, useRoutingEvents } from '../../hooks/useRoutingEvents'
import { FEATURE_FLAGS } from '../../lib/featureFlags'
import { Play, RefreshCw, ChevronDown, ChevronUp, Code, Plus, Trash2, PieChart as PieChartIcon, X, Network, Settings, ArrowRightLeft, Target, TrendingDown } from 'lucide-react'

// UI-local algorithm tokens for the simulation dropdown. Maps to the backend
// /decide-gateway request as follows:
//   'SR_BASED_ROUTING'   → { rankingAlgorithm: 'SR_BASED_ROUTING' }
//   'SR_MULTI_OBJECTIVE' → { rankingAlgorithm: 'SR_BASED_ROUTING', enableMultiObjective: true }
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
const MULTI_OBJECTIVE_PAYMENT_METHODS = ['CREDIT'] as const
const CARD_PROGRAM_OPTIONS = ['STANDARD', 'PREMIUM'] as const
const MULTI_OBJECTIVE_CARD_BRANDS = ['VISA', 'MASTERCARD'] as const

const MULTI_OBJECTIVE_CLUSTER_VARIANTS: Array<{
  paymentMethod: 'CREDIT'
  cardSwitchProvider: 'VISA' | 'MASTERCARD'
  cardProgram: 'STANDARD' | 'PREMIUM'
}> = [
  { paymentMethod: 'CREDIT', cardSwitchProvider: 'VISA',       cardProgram: 'STANDARD' },
  { paymentMethod: 'CREDIT', cardSwitchProvider: 'VISA',       cardProgram: 'PREMIUM'  },
  { paymentMethod: 'CREDIT', cardSwitchProvider: 'MASTERCARD', cardProgram: 'STANDARD' },
  { paymentMethod: 'CREDIT', cardSwitchProvider: 'MASTERCARD', cardProgram: 'PREMIUM'  },
]

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
  tolerancePp?: number | null
  amount: number
  currency: string
}

function formatCurrencyValue(value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency,
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

// Auth-rate simulation runs a fixed batch — no user input; it starts on Run
// and stops at this many transactions.
const SIMULATION_TOTAL_PAYMENTS = '5000'

// Static annotation shown beside the simulation controls (read-only note).
const EXPERIMENT_NOTE = 'Stripe degraded · Testing cost-based fallback to Adyen'

// Default auth-rate tolerance band below the best gateway's SR within which gateways are
// SR-equivalent and become eligible for cost-based routing. Stored as a fraction
// (0.2 = 20 percentage points), matching the "Cost Optimisation Override Configuration"
// default of 0.5 pp on the SR Routing page. The live per-decision value
// (multi_objective_info.tolerancePp) overrides this when present.
const DEFAULT_COST_ROUTING_TOLERANCE = 0.005

const DEFAULT_SIMULATION_CONFIG: SimulationConfig = {
  totalPayments: SIMULATION_TOTAL_PAYMENTS,
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
}

function cloneRuleParams(params: RuleEvaluateParams[]) {
  return params.map((param) => ({ ...param }))
}

function cloneConnectors(connectors: GatewayConnector[]) {
  return connectors.map((connector) => ({ ...connector }))
}

function normalizeDebitCardType(value: unknown): DebitRoutingFormState['card_type'] {
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
        card_type: normalizeDebitCardType(parsed.debitForm?.card_type),
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

  const [simulationConfig, setSimulationConfig] = useState<SimulationConfig>(initialState.simulationConfig)
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
  const [isSimulating, setIsSimulating] = useState(false)
  const [smartRetryEnabled, setSmartRetryEnabled] = useState(initialState.smartRetryEnabled)
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
  const routingEvents = useRoutingEvents('1h')
  const sessionRoutingEvents = useMemo(() => {
    if (simulationStartedAtMs == null) return []
    const sinceMs = simulationStartedAtMs - EVENTS_RUN_START_MARGIN_MS
    return routingEvents.events.filter((event) => event.bucket_ms >= sinceMs)
  }, [routingEvents.events, simulationStartedAtMs])
  const [showPenaltyGuide, setShowPenaltyGuide] = useState(false)

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
  // Currency = USD, Method Type = CARD, Payment Method = CREDIT, and Card Brand
  // defaults to Visa/Mastercard when the prior selection isn't one of them.
  useEffect(() => {
    if (form.ranking_algorithm !== 'SR_MULTI_OBJECTIVE') return
    setForm(prev => {
      if (prev.ranking_algorithm !== 'SR_MULTI_OBJECTIVE') return prev
      const next = { ...prev }
      let changed = false
      if (next.currency !== MULTI_OBJECTIVE_CURRENCY) { next.currency = MULTI_OBJECTIVE_CURRENCY; changed = true }
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
  }, [form.ranking_algorithm, paymentMethodTypeOptions, cardBrandOptions])

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
    // Skip persisting mid-run — results are flushed once the run completes.
    if (isSimulating || loading) return

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
    }

    saveExplorerState(currentScopeKey, nextState)
  }, [
    currentScopeKey,
    stateScopeKey,
    isSimulating,
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
  ])

  function setDebitField<K extends keyof DebitRoutingFormState>(field: K, value: DebitRoutingFormState[K]) {
    setDebitForm(f => ({ ...f, [field]: value }))
  }

  function setErrorField(updates: Partial<ErrorInfoState>) {
    setErrorInfo(f => ({ ...f, ...updates }))
  }

  function getGwSimConfig(gw: string): GatewaySimConfig {
    return gatewaySimConfigsRef.current[gw] ?? { ...DEFAULT_GW_SIM_CONFIG }
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
        card_type: normalizeDebitCardType(debitForm.card_type),
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
        enableMultiObjective: form.ranking_algorithm === 'SR_MULTI_OBJECTIVE',
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

  async function runSimulation() {
    if (!effectiveMerchantId) return setError('Sign in with a merchant-linked account to continue')
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')

    const total = parseInt(simulationConfig.totalPayments) || 0

    if (total <= 0) return setError('Total Payments must be greater than 0')

    setIsSimulating(true)
    setSimulationStartedAtMs(Date.now())
    setError(null)
    setSetupPrompt(null)
    setSimulationResults([])
    simulationAbortRef.current = false

    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    const results: SimulationResult[] = []
    const MAX_CONSECUTIVE_ERRORS = 3
    let consecutiveErrors = 0
    let lastUIUpdate = 0

    const isMultiObjective = form.ranking_algorithm === 'SR_MULTI_OBJECTIVE'

    try {
      for (let i = 0; i < total; i++) {
        if (simulationAbortRef.current) break
        const paymentId = `sim_${Date.now()}_${i}`

        // Under SR_MULTI_OBJECTIVE, vary the cluster and amount per payment so
        // the (mock) Hypersense cost lookup returns distinct costs and the
        // multi-objective leg has meaningful choices to make. Form values still
        // seed everything else (currency, eligible_gateways, etc).
        const variant = isMultiObjective ? MULTI_OBJECTIVE_CLUSTER_VARIANTS[i % MULTI_OBJECTIVE_CLUSTER_VARIANTS.length] : null
        const paymentMethodType = isMultiObjective ? 'CARD' : form.payment_method_type
        const paymentMethod = variant ? variant.paymentMethod : form.payment_method
        const cardBrand = variant ? variant.cardSwitchProvider : form.card_brand
        const cardProgram = variant ? variant.cardProgram : form.card_program
        const amount = isMultiObjective
          ? Math.floor(10 + Math.random() * 991)
          : (parseFloat(form.amount) || 1000)

        try {
          const decideRes = await apiPost<DecideGatewayResponse>('/decide-gateway', {
            merchantId: effectiveMerchantId,
            paymentInfo: {
              paymentId: paymentId,
              amount,
              currency: form.currency,
              paymentType: 'ORDER_PAYMENT',
              paymentMethodType,
              paymentMethod,
              authType: form.auth_type,
              cardBrand,
              cardSwitchProvider: cardBrand,
              cardType: paymentMethod,
              cardProgram,
            },
            eligibleGatewayList: gateways,
            rankingAlgorithm: 'SR_BASED_ROUTING',
            enableMultiObjective: isMultiObjective,
            eliminationEnabled: eliminationEnabled,
          })

          const decidedGateway = decideRes.decided_gateway
          const gwRate = getGwSuccessRate(decidedGateway)
          const isSuccess = Math.random() * 100 < gwRate
          const failureMode = getGwFailureMode(decidedGateway)
          const outcome: TransactionOutcome = isSuccess ? 'CHARGED' : (failureMode === 'timeout' ? 'PENDING_VBV' : 'FAILURE')

          const scoreRes = await apiPost<UpdateScoreResponse>('/update-gateway-score', {
            merchantId: effectiveMerchantId,
            gateway: decidedGateway,
            gatewayReferenceId: null,
            status: outcome,
            paymentId: paymentId,
            enforceDynamicRoutingFailure: null,
            ...(outcome === 'FAILURE' && { errorInfo: buildSimErrorInfo(decidedGateway) }),
          })

          let retryGateway: string | undefined
          let retryStatus: TransactionOutcome | undefined

          if (
            smartRetryEnabled &&
            gsmScoringFilterEnabled &&
            outcome === 'FAILURE' &&
            scoreRes.gsm_info?.decision === 'retry' &&
            decideRes.fallback_gateways.length > 0
          ) {
            retryGateway = decideRes.fallback_gateways[0]
            const retryGwRate = getGwSuccessRate(retryGateway)
            const retrySuccess = Math.random() * 100 < retryGwRate
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
              ...(retryStatus === 'FAILURE' && { errorInfo: buildSimErrorInfo(retryGateway) }),
            })
          }

          const mo = decideRes.multi_objective_info ?? null
          results.push({
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
            tolerancePp: mo?.tolerancePp ?? null,
            amount,
            currency: form.currency,
          })

          consecutiveErrors = 0

          const now = Date.now()
          if (now - lastUIUpdate > 150 || i === total - 1) {
            setSimulationResults([...results])
            markExplorerRunDataUpdated()
            lastUIUpdate = now
          }
        } catch (e: unknown) {
          consecutiveErrors++
          if (consecutiveErrors >= MAX_CONSECUTIVE_ERRORS) {
            handleRunError(e, 'batch', `Simulation stopped after ${MAX_CONSECUTIVE_ERRORS} consecutive backend errors. Check that the server is running.`)
            return
          }
          await new Promise(resolve => setTimeout(resolve, 1000))
          continue
        }
      }
    } finally {
      setSimulationResults([...results])
      setIsSimulating(false)
    }
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
      .map(([name, score]) => ({ name, score: Math.round(score * 1000) / 10 }))
    : []

  const totalSimulationPayments = parseInt(simulationConfig.totalPayments) || 0
  const completedSimulationCount = simulationResults.length
  const simulationProgressPercentage =
    totalSimulationPayments > 0
      ? Math.round((completedSimulationCount / totalSimulationPayments) * 100)
      : 0
  const hasSimulationActivity = isSimulating || completedSimulationCount > 0

  const eligibleGatewaysParsed = useMemo(
    () => form.eligible_gateways.split(',').map(s => s.trim().toLowerCase()).filter(Boolean),
    [form.eligible_gateways],
  )

  const gatewayColorMap = useMemo(
    () => Object.fromEntries(eligibleGatewaysParsed.map((gw, i) => [gw, GW_PALETTE[i % GW_PALETTE.length]])),
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
        const config = prev[gw] ?? { ...DEFAULT_GW_SIM_CONFIG }
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

  // Latest SR score the routing engine assigned to each gateway (gatewayPriorityMap),
  // i.e. the "real" success rate plotted in the trend chart — not the observed success/total ratio.
  const gatewaySrScores = useMemo(() => {
    const scores: Record<string, number> = {}
    for (const r of deferredSimulationResults) {
      if (r.routingApproach?.includes('HEDGING') || !r.gatewayPriorityMap) continue
      for (const [gw, score] of Object.entries(r.gatewayPriorityMap)) {
        if (score !== undefined && score !== null) scores[gw] = Math.round((score as number) * 100)
      }
    }
    return scores
  }, [deferredSimulationResults])

  // Live cost-optimisation tolerance band (pp) — the most recent value the router reported
  // for this run, falling back to the configured default when no decision carried one.
  const costRoutingTolerancePp = useMemo(() => {
    for (let i = deferredSimulationResults.length - 1; i >= 0; i--) {
      const t = deferredSimulationResults[i].tolerancePp
      if (t != null) return t
    }
    return DEFAULT_COST_ROUTING_TOLERANCE
  }, [deferredSimulationResults])

  // Single forward pass: carry the last seen non-hedging score for each gateway forward,
  // sampling every `step` results. O(n) vs the previous O(n × MAX_PTS) backward search.
  const gatewaySparklines = useMemo(() => {
    const results = deferredSimulationResults
    const empty = { series: {} as Record<string, number[]>, paymentNums: [] as number[], yMin: 0, yMax: 100 }
    if (results.length < 2) return empty

    const MAX_PTS = 200
    const step = Math.max(1, Math.floor(results.length / MAX_PTS))
    const gateways = Object.keys(gatewayStats)
    const lastScore: Record<string, number> = {}
    const series: Record<string, number[]> = {}
    gateways.forEach(gw => { series[gw] = [] })
    const paymentNums: number[] = []

    for (let i = 0; i < results.length; i++) {
      const r = results[i]
      if (!r.routingApproach?.includes('HEDGING') && r.gatewayPriorityMap) {
        for (const gw of gateways) {
          const score = r.gatewayPriorityMap[gw]
          if (score !== undefined && score !== null) lastScore[gw] = Math.round(score * 100)
        }
      }
      if (i % step === 0 || i === results.length - 1) {
        paymentNums.push(i + 1)
        for (const gw of gateways) series[gw].push(lastScore[gw] ?? 0)
      }
    }

    // Compute y-axis range here to avoid flatMap+spread in render
    let obsMin = 100, obsMax = 0
    for (const gw of gateways) {
      for (const v of series[gw]) {
        if (v > 0) { if (v < obsMin) obsMin = v; if (v > obsMax) obsMax = v }
      }
    }
    if (obsMin > obsMax) { obsMin = 0; obsMax = 100 }
    const yMin = Math.max(0, obsMin - 2)
    const yMax = Math.min(100, obsMax + 2)

    return { series, paymentNums, yMin, yMax }
  }, [deferredSimulationResults, gatewayStats])

  const hedgingHits = useMemo(
    () => deferredSimulationResults.filter(r => r.routingApproach?.includes('HEDGING')).length,
    [deferredSimulationResults],
  )

  const smartRetryStats = useMemo(() => {
    const triggered = deferredSimulationResults.filter(r => r.retryGateway !== undefined).length
    const recovered = deferredSimulationResults.filter(r => r.retryStatus === 'CHARGED').length
    return { triggered, recovered }
  }, [deferredSimulationResults])

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

  const debitNetworkRows = debitResult?.debit_routing_output?.co_badged_card_networks_info || []
  const volumeColorIndex = useMemo(
    () => new Map(volumeDistribution.map((item, index) => [item.name, index] as const)),
    [volumeDistribution],
  )
  const sortedGatewayStats = useMemo(
    () => Object.entries(gatewayStats).sort((a, b) => b[1].total - a[1].total),
    [gatewayStats],
  )
  const totalRoutedPayments = useMemo(
    () => Object.values(gatewayStats).reduce((s, g) => s + g.total, 0),
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
      setSimulationConfig(defaults.simulationConfig)
      setGatewaySimConfigs({})
      setSimulationResults(defaults.simulationResults)
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
      ? 'Reset Multi Objective Routing'
      : activeTab === 'rule'
        ? 'Reset Rule Based Routing'
        : activeTab === 'volume'
          ? 'Reset Volume Based Routing'
          : 'Reset Debit Routing'

  return (
    <div className="mx-auto max-w-[1500px] space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <SurfaceLabel>Simulation console</SurfaceLabel>
          <div className="mt-2 flex flex-wrap items-center gap-3">
            <h1 className="text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">Decision Explorer</h1>
            <Badge variant="blue">{effectiveMerchantId || 'No merchant'}</Badge>
          </div>
        </div>
        <Button size="sm" variant="secondary" onClick={resetCurrentTabState}>
          <RefreshCw size={14} />
          {resetButtonLabel}
        </Button>
      </div>

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
        <div className="flex flex-wrap items-end gap-6 rounded-2xl border border-slate-200 bg-white px-5 py-4 dark:border-[#222226] dark:bg-[#0b0b10]">
          {[
            { key: 'stripe', label: 'Stripe success rate' },
            { key: 'adyen', label: 'Adyen success rate' },
          ].map(({ key, label }) => {
            const color = gatewayColorMap[key] ?? GW_PALETTE[0]
            const rate = getGwSuccessRate(key)
            return (
              <div key={key} className="flex w-[190px] flex-col gap-1.5">
                <div className="flex items-center gap-1.5">
                  <span className="h-2 w-2 shrink-0 rounded-full" style={{ background: color }} />
                  <SurfaceLabel>{label}</SurfaceLabel>
                </div>
                <input
                  type="text"
                  value={`${rate}%`}
                  onChange={e => setGwSuccessRate(key, Math.max(0, Math.min(100, parseInt(e.target.value.replace(/\D/g, ''), 10) || 0)))}
                  className="w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm font-semibold text-slate-800 focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0d0d13] dark:text-slate-100"
                />
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={rate}
                  onChange={e => setGwSuccessRate(key, Number(e.target.value))}
                  className="h-1 w-full cursor-pointer"
                  style={{ accentColor: color }}
                />
              </div>
            )
          })}

          <div className="flex min-w-[240px] flex-1 flex-col gap-1.5">
            <SurfaceLabel>Experiment variables</SurfaceLabel>
            <p className="py-1.5 text-sm leading-snug text-slate-700 dark:text-slate-200">
              {EXPERIMENT_NOTE}
            </p>
          </div>

          <Button
            onClick={isSimulating ? () => { simulationAbortRef.current = true } : runSimulation}
            disabled={!effectiveMerchantId || routingConfigUnavailable}
            variant={isSimulating ? 'secondary' : 'primary'}
            className="ml-auto self-end"
          >
            {isSimulating ? <><X size={14} /> Stop</> : <><Play size={14} className="fill-current" /> Run simulation</>}
          </Button>
        </div>
      )}

      <div className={activeTab === 'volume'
        ? 'grid grid-cols-1 gap-5 xl:grid-cols-[minmax(340px,420px)_minmax(0,1fr)]'
        : activeTab === 'batch'
          ? 'grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)]'
          : 'grid grid-cols-1 gap-6 lg:grid-cols-2'
      }
        style={activeTab === 'rule' ? { display: 'none' } : undefined}
      >
        <div className={`flex flex-col gap-6 min-w-0 ${activeTab === 'batch' ? '' : 'self-start'}`}>
        {activeTab === 'batch' && (
                <Card className="flex-1">
                  <CardHeader className="!py-3">
                    <div className="flex items-center justify-between gap-3">
                      <h3 className="text-sm font-medium text-slate-800 dark:text-white">Gateway Selection Summary</h3>
                      <div className="flex items-center gap-1.5">
                        {totalSimulationPayments > 0 && (() => {
                          const r = 6
                          const circ = 2 * Math.PI * r
                          const offset = circ * (1 - simulationProgressPercentage / 100)
                          return (
                            <svg width="16" height="16" viewBox="0 0 16 16" className="-rotate-90">
                              <circle cx="8" cy="8" r={r} fill="none" strokeWidth="2" className="stroke-slate-200 dark:stroke-slate-700" />
                              <circle
                                cx="8" cy="8" r={r} fill="none" strokeWidth="2" strokeLinecap="round"
                                strokeDasharray={circ} strokeDashoffset={offset}
                                className="stroke-brand-500 transition-[stroke-dashoffset] duration-300"
                              />
                            </svg>
                          )
                        })()}
                        <span className="text-[11px] text-slate-500 dark:text-slate-400">
                          {completedSimulationCount} / {totalSimulationPayments || 0}
                        </span>
                        {completedSimulationCount > 0 && (
                          <>
                            <span className="text-slate-300 dark:text-slate-600">·</span>
                            {hedgingHits > 0 ? (
                              <span className="text-[11px] font-medium text-brand-600 dark:text-sky-400">
                                {Math.round((hedgingHits / completedSimulationCount) * 100)}% hedged
                              </span>
                            ) : (
                              <UiTooltip text="Enable ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3_{PMT} or ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3 in Redis">
                                <span className="text-[11px] text-amber-500 dark:text-amber-400 cursor-help">0 hedged</span>
                              </UiTooltip>
                            )}
                            {smartRetryEnabled && smartRetryStats.triggered > 0 && (
                              <>
                                <span className="text-slate-300 dark:text-slate-600">·</span>
                                <UiTooltip text={`${smartRetryStats.triggered} retries triggered · ${smartRetryStats.recovered} recovered`}>
                                  <span className="text-[11px] font-medium text-orange-500 dark:text-orange-400 cursor-help">
                                    {smartRetryStats.triggered} retried · {smartRetryStats.recovered} recovered
                                  </span>
                                </UiTooltip>
                              </>
                            )}
                            {totalCostSaved.value > 0 && totalCostSaved.currency && (
                              <>
                                <span className="text-slate-300 dark:text-slate-600">·</span>
                                <span className="text-[11px] font-medium text-emerald-600 dark:text-emerald-400">
                                  saved {formatCurrencyValue(totalCostSaved.value, totalCostSaved.currency)}
                                </span>
                              </>
                            )}
                          </>
                        )}
                      </div>
                    </div>
                  </CardHeader>
                  <CardBody className="!pt-3 flex flex-1 flex-col">
                    {sortedGatewayStats.length > 0 ? (() => {
                      const totalRouted = totalRoutedPayments
                      const sortedGateways = sortedGatewayStats
                      const hasAnySpark = sortedGateways.some(([gw]) => (gatewaySparklines.series[gw]?.length ?? 0) >= 2)
                      return (
                        <div className="flex flex-1 flex-col gap-4">
                          {/* per-gateway stat rows */}
                          <div className="space-y-2">
                            {sortedGateways.map(([gateway, stats]) => {
                              const share = totalRouted > 0 ? Math.round((stats.total / totalRouted) * 100) : 0
                              const observedSr = stats.total > 0 ? Math.round((stats.success / stats.total) * 100) : 0
                              const srPct = gatewaySrScores[gateway] ?? observedSr
                              const gwColor = gatewayColorMap[gateway] ?? GW_PALETTE[0]
                              return (
                                <div key={gateway} className="space-y-1">
                                  <div className="flex items-center justify-end gap-1.5">
                                    <span className="w-2 h-2 rounded-full shrink-0" style={{ backgroundColor: gwColor }} />
                                    <span className="text-xs font-semibold text-slate-700 dark:text-slate-200 truncate shrink-0 w-14 text-left">{gateway}</span>
                                    <span className={`font-bold tabular-nums text-[11px] w-14 text-right ${srPct >= 80 ? 'text-emerald-600 dark:text-emerald-400' : srPct >= 50 ? 'text-amber-500' : 'text-red-500'}`}>{srPct}% SR</span>
                                    <span className="text-slate-300 dark:text-slate-600 text-[11px]">·</span>
                                    <span className="text-slate-400 dark:text-slate-500 tabular-nums text-[11px] w-16 text-right">{share}% routed</span>
                                    <span className="text-slate-300 dark:text-slate-600 text-[11px]">·</span>
                                    <span className="tabular-nums text-slate-500 dark:text-slate-400 text-[11px] w-12 text-right">
                                      <span className="text-emerald-600 dark:text-emerald-400">{stats.success}</span>
                                      <span className="text-slate-300 dark:text-slate-600 mx-0.5">/</span>
                                      <span className="text-red-400">{stats.failure}</span>
                                    </span>
                                  </div>
                                </div>
                              )
                            })}
                          </div>
                          {/* combined multi-line SR trend chart */}
                          {hasAnySpark && (() => {
                            const { series, paymentNums } = gatewaySparklines
                            // Cost-based routing is eligible for any PSP whose SR sits within the
                            // auth-rate tolerance band of the *leading* PSP's SR. The leader moves
                            // every transaction, so the threshold is computed per point as
                            // topPspSr − band and drawn as a dynamic dashed line: wherever a PSP's
                            // line crosses it you can see exactly when it entered or exited the band.
                            const bandPp = costRoutingTolerancePp * 100
                            const chartData = paymentNums.map((n, i) => {
                              const row: Record<string, number> = { step: n }
                              let topAtPoint: number | null = null
                              sortedGateways.forEach(([gw]) => {
                                const v = series[gw]?.[i]
                                if (v != null) {
                                  row[gw] = v
                                  if (topAtPoint == null || v > topAtPoint) topAtPoint = v
                                }
                              })
                              if (topAtPoint != null) {
                                const threshold = Math.max(0, topAtPoint - bandPp)
                                row.topPspSr = topAtPoint
                                row.threshold = threshold
                                // Span from the threshold up to 100%, stacked over a transparent base,
                                // so the shaded eligible area rises and falls with the band.
                                row.eligibleSpan = Math.max(0, 100 - threshold)
                              }
                              return row
                            })
                            const bandLabel = `Dynamic Threshold (Top PSP − ${bandPp.toFixed(bandPp % 1 ? 1 : 0)}pp)`
                            return (
                              <div className="w-full flex-1 min-h-[320px] flex flex-col">
                                <div className="min-h-0 flex-1">
                                <ResponsiveContainer width="100%" height="100%">
                                  <ComposedChart data={chartData} margin={{ top: 16, right: 8, bottom: 28, left: 0 }}>
                                    <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                                    <Area dataKey="threshold" stackId="elig" stroke="none" fill="transparent" isAnimationActive={false} legendType="none" activeDot={false} connectNulls />
                                    <Area dataKey="eligibleSpan" stackId="elig" stroke="none" fill="#10b981" fillOpacity={0.08} isAnimationActive={false} legendType="none" activeDot={false} connectNulls />
                                    <XAxis
                                      dataKey="step"
                                      tick={{ fontSize: 11, fill: '#94a3b8' }}
                                      tickLine={false}
                                      axisLine={{ stroke: '#e2e8f0' }}
                                      minTickGap={28}
                                      label={{ value: 'Number of Transactions', position: 'insideBottom', offset: -12, fontSize: 11, fill: '#94a3b8' }}
                                    />
                                    <YAxis
                                      domain={[0, 100]}
                                      tick={{ fontSize: 11, fill: '#94a3b8' }}
                                      tickLine={false}
                                      axisLine={{ stroke: '#e2e8f0' }}
                                      width={44}
                                      tickFormatter={(v: number) => `${v}%`}
                                    />
                                    <Tooltip
                                      content={(props) => {
                                        const { active, payload } = props as unknown as { active?: boolean; payload?: Array<{ payload?: Record<string, number> }> }
                                        if (!active || !payload || !payload.length) return null
                                        const row = payload[0].payload
                                        if (!row) return null
                                        return (
                                          <div style={{ ...CHART_TOOLTIP_STYLE, padding: '10px 12px', fontSize: 12, lineHeight: 1.5 }}>
                                            <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>Payment {row.step}</p>
                                            {sortedGateways.map(([gw]) => (
                                              row[gw] == null ? null : (
                                                <p key={gw} style={{ ...CHART_TOOLTIP_ITEM_STYLE, margin: '2px 0', color: gatewayColorMap[gw] ?? GW_PALETTE[0] }}>
                                                  {gw}: {row[gw].toFixed(1)}%
                                                </p>
                                              )
                                            ))}
                                            {row.threshold != null && (
                                              <div style={{ marginTop: 8, paddingTop: 8, borderTop: '1px solid rgba(148,163,184,0.25)' }}>
                                                <p style={{ ...CHART_TOOLTIP_ITEM_STYLE, margin: '3px 0' }}><strong>Top PSP SR:</strong> {row.topPspSr?.toFixed(1)}%</p>
                                                <p style={{ ...CHART_TOOLTIP_ITEM_STYLE, margin: '3px 0' }}><strong>Configured Band:</strong> −{bandPp.toFixed(bandPp % 1 ? 1 : 0)}pp</p>
                                                <p style={{ ...CHART_TOOLTIP_ITEM_STYLE, margin: '3px 0' }}><strong>Cost-Eligible Threshold:</strong> {row.threshold.toFixed(1)}%</p>
                                                <p style={{ ...CHART_TOOLTIP_ITEM_STYLE, margin: '6px 0 0', opacity: 0.7, fontSize: 10 }}>This threshold changes at every x-axis point.</p>
                                              </div>
                                            )}
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
                                    <Line
                                      type="natural"
                                      dataKey="threshold"
                                      name={bandLabel}
                                      stroke="#10b981"
                                      strokeDasharray="5 4"
                                      strokeWidth={1.75}
                                      dot={false}
                                      activeDot={false}
                                      isAnimationActive={false}
                                      connectNulls
                                    />
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
                                  <span className="inline-flex items-center gap-1.5">
                                    <span className="inline-block w-4 border-t-2 border-dashed" style={{ borderColor: '#10b981' }} />
                                    {bandLabel}
                                  </span>
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
                      const previewScores = previewGateways.map(gw => ({ gw, sr: getGwSuccessRate(gw) }))
                      const bestSr = previewScores.reduce((m, g) => Math.max(m, g.sr), 0)
                      const bandPp = DEFAULT_COST_ROUTING_TOLERANCE * 100
                      const costBandThreshold = Math.max(0, bestSr - bandPp)
                      const target = totalSimulationPayments || Number(SIMULATION_TOTAL_PAYMENTS)
                      const chartData = [1, target].map(step => {
                        const row: Record<string, number> = { step }
                        previewScores.forEach(({ gw, sr }) => { row[gw] = sr })
                        return row
                      })
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
                              <LineChart data={chartData} margin={{ top: 16, right: 8, bottom: 20, left: 0 }}>
                                <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" className="dark:opacity-20" vertical={false} />
                                <ReferenceArea y1={costBandThreshold} y2={100} fill="#10b981" fillOpacity={0.08} ifOverflow="extendDomain" />
                                <ReferenceLine
                                  y={costBandThreshold}
                                  stroke="#10b981"
                                  strokeDasharray="4 4"
                                  strokeWidth={1.5}
                                  label={{ value: `Cost-based eligible ≥ ${costBandThreshold.toFixed(costBandThreshold % 1 ? 1 : 0)}% (${bandPp.toFixed(bandPp % 1 ? 1 : 0)}pp band)`, position: 'insideTopLeft', fontSize: 10, fill: '#059669' }}
                                />
                                <XAxis
                                  dataKey="step"
                                  tick={{ fontSize: 11, fill: '#94a3b8' }}
                                  tickLine={false}
                                  axisLine={{ stroke: '#e2e8f0' }}
                                  minTickGap={28}
                                  label={{ value: 'Number of Transactions', position: 'insideBottom', offset: -12, fontSize: 11, fill: '#94a3b8' }}
                                />
                                <YAxis
                                  domain={[0, 100]}
                                  tick={{ fontSize: 11, fill: '#94a3b8' }}
                                  tickLine={false}
                                  axisLine={{ stroke: '#e2e8f0' }}
                                  width={44}
                                  tickFormatter={(v: number) => `${v}%`}
                                />
                                {previewScores.map(({ gw }) => (
                                  <Line
                                    key={gw}
                                    type="monotone"
                                    dataKey={gw}
                                    name={gw}
                                    stroke={gatewayColorMap[gw] ?? GW_PALETTE[0]}
                                    strokeWidth={2.5}
                                    strokeDasharray="5 4"
                                    dot={false}
                                    isAnimationActive={false}
                                    connectNulls
                                  />
                                ))}
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

        <div className="min-w-0 flex flex-col gap-4">
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
              {/* Events — routing events from the active simulation run only. */}
              <Card>
                <CardHeader className="flex flex-row items-center justify-between gap-3">
                  <span className="flex items-center gap-2.5">
                    <span className="h-2 w-2 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.18)]" />
                    <h3 className="text-sm font-medium text-slate-800 dark:text-white">Events</h3>
                  </span>
                  {sessionRoutingEvents.length > 0 && (
                    <span className="text-xs text-slate-400 tabular-nums">{sessionRoutingEvents.length} events</span>
                  )}
                </CardHeader>
                <CardBody className="p-0">
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
                  ) : sessionRoutingEvents.length === 0 ? (
                    <div className="py-12 text-center">
                      <p className="text-sm text-slate-500 dark:text-slate-400">Waiting for events…</p>
                      <p className="mt-1 text-xs text-slate-400 dark:text-slate-500">
                        No leader changes or auth-band crossings for this run yet.
                      </p>
                    </div>
                  ) : (
                    <div className="max-h-[360px] overflow-y-auto divide-y divide-slate-100 dark:divide-[#1a1a22]">
                      {sessionRoutingEvents.slice(0, 50).map((event) => {
                        const meta = SIM_EVENT_META[event.event_type]
                        const Icon = meta?.icon ?? ArrowRightLeft
                        return (
                          <div key={event.id} className="flex items-start gap-2.5 px-4 py-2.5">
                            <div className="mt-0.5 shrink-0">
                              <Icon size={14} className={meta?.iconClass ?? 'text-slate-400'} />
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="text-[13px] text-slate-700 dark:text-slate-200">{describeRoutingEvent(event)}</p>
                              <p className="mt-0.5 text-[11px] text-slate-400">{formatSimEventTime(event.bucket_ms)}</p>
                            </div>
                          </div>
                        )
                      })}
                    </div>
                  )}
                </CardBody>
              </Card>
              {hasSimulationActivity ? (
              <>
                <Card>
                  <CardHeader className="flex flex-row items-center justify-between gap-3">
                    <h3 className="text-sm font-medium text-slate-800 dark:text-white">Transaction Log</h3>
                    {deferredSimulationResults.length > 0 && (
                      <span className="text-xs text-slate-400 tabular-nums">{deferredSimulationResults.length} transactions</span>
                    )}
                  </CardHeader>
                  <CardBody className="p-0">

                    {deferredSimulationResults.length > 0 ? (
                      <div ref={txLogRef} className="max-h-[480px] overflow-y-auto overflow-x-hidden">
                        <table className="w-full text-sm">
                          <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-[11px] text-slate-400 dark:text-slate-500 sticky top-0 border-b border-slate-100 dark:border-[#1c1c24]">
                            <tr>
                              <th className="text-left px-2 py-2 w-8">#</th>
                              <th className="text-left px-2 py-2">Amount</th>
                              <th className="text-left px-2 py-2 whitespace-nowrap">Gateway</th>
                              <th className="text-left px-2 py-2">Routing</th>
                              <th className="text-left px-2 py-2">Outcome</th>
                              <th className="text-right px-2 py-2 whitespace-nowrap">Cost Savings</th>
                              {smartRetryEnabled && <th className="text-left px-2 py-2 whitespace-nowrap">Retry Gateway</th>}
                              {smartRetryEnabled && <th className="text-left px-2 py-2">Retry Outcome</th>}
                            </tr>
                          </thead>
                          <tbody className="divide-y divide-slate-100 dark:divide-[#1a1a22]">
                            {deferredSimulationResults.map((res, idx) => {
                              const absIdx = idx
                              return (
                              <tr
                                key={res.paymentId}
                                className="group cursor-pointer hover:bg-slate-50 dark:hover:bg-[#0d0d14] transition-colors"
                                onClick={() => openAuditModal(res.paymentId)}
                              >
                                <td className="px-2 py-2 text-[11px] text-slate-400 tabular-nums">{absIdx + 1}</td>
                                <td className="px-2 py-2 whitespace-nowrap">
                                  <span className="block font-mono text-xs text-slate-700 dark:text-slate-300 tabular-nums group-hover:text-brand-600 dark:group-hover:text-brand-400 transition-colors">
                                    {formatCurrencyValue(res.amount, res.currency)}
                                  </span>
                                </td>
                                <td className="px-2 py-2 text-xs font-medium text-slate-600 dark:text-slate-300 whitespace-nowrap">{res.decidedGateway}</td>
                                <td className="px-2 py-2">
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
                                <td className="px-2 py-2">
                                  <span className={`text-xs font-semibold ${res.status === 'CHARGED' ? 'text-emerald-600 dark:text-emerald-400' : res.status === 'PENDING_VBV' ? 'text-amber-600 dark:text-amber-400' : 'text-red-500 dark:text-red-400'}`}>{res.status}</span>
                                </td>
                                <td className="px-2 py-2 text-right whitespace-nowrap">
                                  {res.costWon && res.costSavedBps != null && res.costSavedBps > 0 && res.status === 'CHARGED' ? (
                                    <span className="font-mono text-xs text-emerald-700 dark:text-emerald-400 tabular-nums">
                                      {formatSavingsCurrency(res.costSavedBps, res.amount, res.currency)}
                                    </span>
                                  ) : null}
                                </td>
                                {smartRetryEnabled && (
                                  <td className="px-2 py-2 text-xs text-slate-500 dark:text-slate-400 whitespace-nowrap">
                                    {res.retryGateway ?? '—'}
                                  </td>
                                )}
                                {smartRetryEnabled && (
                                  <td className="px-2 py-2">
                                    {res.retryStatus ? (
                                      <Badge variant={res.retryStatus === 'CHARGED' ? 'green' : res.retryStatus === 'PENDING_VBV' ? 'orange' : 'red'}>
                                        {res.retryStatus}
                                      </Badge>
                                    ) : <span className="text-xs text-slate-400">—</span>}
                                  </td>
                                )}
                              </tr>
                            )})}
                          </tbody>
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
              </>
            ) : (
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
            <div className="text-right">
              <span className="text-[11px] uppercase tracking-[0.16em] text-slate-500 dark:text-slate-400">
                Tolerance band
              </span>
              <p className="font-mono text-sm font-semibold text-slate-800 dark:text-slate-100">
                {info.tolerancePp.toFixed(2)} pp
              </p>
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
                  label={isCostWin ? 'Chosen by cost' : 'Final pick'}
                  summary={info.chosen}
                  emphasis={isCostWin}
                />
              )}
            </div>
          )}

          <p className="mt-3 text-[11px] text-slate-500 dark:text-slate-400">
            {info.qualifiedCount} PSP{info.qualifiedCount === 1 ? '' : 's'} qualified under the band.
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
