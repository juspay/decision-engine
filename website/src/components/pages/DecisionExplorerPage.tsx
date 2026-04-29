import { useDeferredValue, useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, PieChart, Pie } from 'recharts'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import { DecideGatewayResponse, GatewayConnector, PaymentAuditEvent, PaymentAuditResponse, RoutingAlgorithmName } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'
import { Play, RefreshCw, ChevronDown, ChevronUp, Activity, Code, Plus, Trash2, PieChart as PieChartIcon, X, Network, Settings } from 'lucide-react'

const ALGORITHMS: RoutingAlgorithmName[] = [
  'SR_BASED_ROUTING',
  'PL_BASED_ROUTING',
  'NTW_BASED_ROUTING',
  'NTW_SR_HYBRID_ROUTING',
]

const ALGORITHM_LABELS: Record<RoutingAlgorithmName, string> = {
  SR_BASED_ROUTING: 'Success Rate Based',
  PL_BASED_ROUTING: 'Priority List Based',
  NTW_BASED_ROUTING: 'Network Based',
  NTW_SR_HYBRID_ROUTING: 'Network + SR Hybrid',
}

type TabType = 'single' | 'batch' | 'rule' | 'volume' | 'debit'

interface FormState {
  amount: string
  currency: string
  payment_method_type: string
  payment_method: string
  card_brand: string
  auth_type: string
  eligible_gateways: string
  ranking_algorithm: RoutingAlgorithmName
  elimination_enabled: boolean
}

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
  successCount: string
  failureCount: string
}

interface SimulationResult {
  paymentId: string
  decidedGateway: string
  status: 'CHARGED' | 'FAILURE'
  timestamp: string
}

type TransactionOutcome = 'CHARGED' | 'FAILURE'

type AuditInspectorTab = 'summary' | 'input' | 'response' | 'raw'

interface SetupPromptState {
  title: string
  body: string
  detail?: string
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

type VolumePaymentEntry = {
  paymentId: string
  connector: string
}

const EXPLORER_STORAGE_KEY = 'decision-explorer-state-v2'
const EXPLORER_RESULT_TTL_MS = 10 * 60 * 1000

const DEFAULT_FORM: FormState = {
  amount: '1000',
  currency: '',
  payment_method_type: '',
  payment_method: '',
  card_brand: '',
  auth_type: '',
  eligible_gateways: 'stripe, adyen',
  ranking_algorithm: 'SR_BASED_ROUTING',
  elimination_enabled: false,
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

const DEFAULT_SIMULATION_CONFIG: SimulationConfig = {
  totalPayments: '10',
  successCount: '7',
  failureCount: '3',
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
}

function cloneRuleParams(params: RuleEvaluateParams[]) {
  return params.map((param) => ({ ...param }))
}

function cloneConnectors(connectors: GatewayConnector[]) {
  return connectors.map((connector) => ({ ...connector }))
}

function normalizeRankingAlgorithm(value: unknown): RoutingAlgorithmName {
  if (value === 'SrBasedRouting') return 'SR_BASED_ROUTING'
  if (value === 'PlBasedRouting') return 'PL_BASED_ROUTING'
  if (value === 'NtwBasedRouting') return 'NTW_BASED_ROUTING'
  if (value === 'NtwSrHybridRouting') return 'NTW_SR_HYBRID_ROUTING'
  return ALGORITHMS.includes(value as RoutingAlgorithmName)
    ? value as RoutingAlgorithmName
    : DEFAULT_FORM.ranking_algorithm
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
  }
}

function explorerScopeKey(userId: string, userEmail: string, merchantId: string) {
  return `${userId || userEmail || 'anonymous'}:${merchantId || 'no-merchant'}`
}

function hasExpiredExplorerResults(resultDataUpdatedAtMs?: number | null) {
  return Boolean(
    resultDataUpdatedAtMs &&
    Date.now() - resultDataUpdatedAtMs > EXPLORER_RESULT_TTL_MS,
  )
}

function loadExplorerState(scopeKey: string): ExplorerPersistedState {
  if (typeof window === 'undefined') return getDefaultExplorerState()

  try {
    const raw = window.localStorage.getItem(EXPLORER_STORAGE_KEY)
    if (!raw) return { ...getDefaultExplorerState(), scopeKey }
    const parsed = JSON.parse(raw) as Partial<ExplorerPersistedState>
    const defaults = getDefaultExplorerState()
    if (parsed.scopeKey !== scopeKey || hasExpiredExplorerResults(parsed.resultDataUpdatedAtMs)) {
      return { ...defaults, scopeKey }
    }

    return {
      ...defaults,
      ...parsed,
      scopeKey,
      resultDataUpdatedAtMs: parsed.resultDataUpdatedAtMs || null,
      activeTab:
        parsed.activeTab && parsed.activeTab !== 'single'
          ? parsed.activeTab
          : defaults.activeTab,
      form: {
        ...defaults.form,
        ...(parsed.form || {}),
        ranking_algorithm: normalizeRankingAlgorithm(parsed.form?.ranking_algorithm),
      },
      simulationConfig: { ...defaults.simulationConfig, ...(parsed.simulationConfig || {}) },
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

function explorerModeButtonClass(active: boolean) {
  return active
    ? 'border-slate-300 bg-white text-slate-950 shadow-sm dark:border-[#3b82f6]/45 dark:bg-[#182131] dark:text-white'
    : 'border-transparent text-slate-500 hover:border-slate-200 hover:bg-white/70 hover:text-slate-900 dark:text-[#8d9ab2] dark:hover:border-[#2a303a] dark:hover:bg-[#151b24] dark:hover:text-white'
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
      body: 'Volume evaluation needs an active volume split rule for this merchant before it can sample distribution.',
      detail,
    }
  }

  if (tab === 'rule') {
    return {
      title: 'Configure rule-based routing first',
      body: 'Rule evaluation needs an active rule-based strategy for this merchant before it can return a policy decision.',
      detail,
    }
  }

  if (tab === 'debit') {
    return {
      title: 'Enable debit routing first',
      body: 'Debit network decisions need the merchant debit routing flag enabled before this explorer can run network routing.',
      detail,
    }
  }

  return {
    title: 'Configure auth-rate routing first',
    body: 'Auth-rate simulation needs success-rate routing configured for this merchant before it can run gateway decisions.',
    detail,
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
        <pre className="overflow-x-auto rounded-[22px] border border-slate-200 bg-slate-950/95 px-4 py-4 text-xs leading-6 text-slate-200 shadow-[0_16px_30px_-28px_rgba(15,23,42,0.4)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
          {stringifyValue(value)}
        </pre>
      ) : (
        <EmptyAuditState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />
      )}
    </div>
  )
}

export function DecisionExplorerPage() {
  const { merchantId } = useMerchantStore()
  const authUser = useAuthStore((state) => state.user)
  const authMerchantId = authUser?.merchantId || ''
  const effectiveMerchantId = merchantId || authMerchantId
  const currentScopeKey = explorerScopeKey(
    authUser?.userId || '',
    authUser?.email || '',
    effectiveMerchantId,
  )
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
  const [successRate, setSuccessRate] = useState(70)

  const [debitForm, setDebitForm] = useState<DebitRoutingFormState>(initialState.debitForm)

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
  const [error, setError] = useState<string | null>(null)
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

  const routingKeyNames = useMemo(
    () => Object.keys(routingKeysConfig).sort(),
    [routingKeysConfig]
  )

  const paymentMethodTypeOptions = useMemo(
    () => toUpperOptions(routingKeysConfig.payment_method?.values || []),
    [routingKeysConfig]
  )

  const paymentMethodOptions = useMemo(() => {
    const methodTypeKey = form.payment_method_type.toLowerCase()
    return toUpperOptions(routingKeysConfig[methodTypeKey]?.values || [])
  }, [form.payment_method_type, routingKeysConfig])

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
    setIsSimulating(false)
  }

  function markExplorerRunDataUpdated() {
    setResultDataUpdatedAtMs(Date.now())
  }

  useEffect(() => {
    if (stateScopeKey === currentScopeKey) return
    clearExplorerRunData()
    setStateScopeKey(currentScopeKey)
    if (typeof window !== 'undefined') {
      window.localStorage.removeItem(EXPLORER_STORAGE_KEY)
    }
  }, [currentScopeKey, stateScopeKey])

  useEffect(() => {
    if (!resultDataUpdatedAtMs) return

    const remainingMs = EXPLORER_RESULT_TTL_MS - (Date.now() - resultDataUpdatedAtMs)
    if (remainingMs <= 0) {
      clearExplorerRunData()
      if (typeof window !== 'undefined') {
        window.localStorage.removeItem(EXPLORER_STORAGE_KEY)
      }
      return
    }

    const timer = window.setTimeout(() => {
      clearExplorerRunData()
      window.localStorage.removeItem(EXPLORER_STORAGE_KEY)
    }, remainingMs)

    return () => window.clearTimeout(timer)
  }, [resultDataUpdatedAtMs])

  useEffect(() => {
    const nextState: ExplorerPersistedState = {
      scopeKey: currentScopeKey,
      resultDataUpdatedAtMs,
      activeTab,
      form,
      simulationConfig,
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
    }

    if (typeof window !== 'undefined') {
      window.localStorage.setItem(EXPLORER_STORAGE_KEY, JSON.stringify(nextState))
    }
  }, [
    currentScopeKey,
    resultDataUpdatedAtMs,
    activeTab,
    form,
    simulationConfig,
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
  ])

  function set<K extends keyof FormState>(field: K, value: FormState[K]) {
    setForm(f => ({ ...f, [field]: value }))
  }

  function setDebitField<K extends keyof DebitRoutingFormState>(field: K, value: DebitRoutingFormState[K]) {
    setDebitForm(f => ({ ...f, [field]: value }))
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
        },
        eligibleGatewayList: gateways,
        rankingAlgorithm: form.ranking_algorithm,
        eliminationEnabled: form.elimination_enabled,
      })
      await apiPost('/update-gateway-score', {
        merchantId: effectiveMerchantId,
        gateway: res.decided_gateway,
        gatewayReferenceId: null,
        status: singleRunOutcome,
        paymentId: paymentId,
        enforceDynamicRoutingFailure: null,
      })
      setResult(res)
      setSingleRunPaymentId(paymentId)
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
    if (!debitRoutingFlag.isEnabled) return openSetupPrompt('debit', 'Debit routing is disabled for this merchant.')

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
    const success = parseInt(simulationConfig.successCount) || 0
    const failure = parseInt(simulationConfig.failureCount) || 0

    if (total <= 0) return setError('Total Payments must be greater than 0')
    if (success + failure !== total) {
      return setError('Success + Failure count must equal Total Payments')
    }

    setIsSimulating(true)
    setError(null)
    setSetupPrompt(null)
    setSimulationResults([])

    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    const results: SimulationResult[] = []

    const outcomes: ('CHARGED' | 'FAILURE')[] = [
      ...Array(success).fill('CHARGED'),
      ...Array(failure).fill('FAILURE'),
    ]

    for (let i = outcomes.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [outcomes[i], outcomes[j]] = [outcomes[j], outcomes[i]]
    }

    try {
      for (let i = 0; i < total; i++) {
        const paymentId = `sim_${Date.now()}_${i}`

        const decideRes = await apiPost<DecideGatewayResponse>('/decide-gateway', {
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
          },
          eligibleGatewayList: gateways,
          rankingAlgorithm: form.ranking_algorithm,
          eliminationEnabled: form.elimination_enabled,
        })

        const decidedGateway = decideRes.decided_gateway
        const outcome = outcomes[i]

        await apiPost('/update-gateway-score', {
          merchantId: effectiveMerchantId,
          gateway: decidedGateway,
          gatewayReferenceId: null,
          status: outcome,
          paymentId: paymentId,
          enforceDynamicRoutingFailure: null,
        })

        results.push({
          paymentId,
          decidedGateway,
          status: outcome,
          timestamp: new Date().toISOString(),
        })

        setSimulationResults([...results])
        markExplorerRunDataUpdated()
      }
    } catch (e: unknown) {
      handleRunError(e, 'batch', 'Simulation failed')
    } finally {
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
          } else {
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
              fallback_output: [
                { gateway_name: 'stripe', gateway_id: 'gateway_001' },
                { gateway_name: 'adyen', gateway_id: 'gateway_002' },
              ],
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
        setVolumeEvaluationLog(logEntries)
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

  const gatewayStats = deferredSimulationResults.reduce((acc, curr) => {
    if (!acc[curr.decidedGateway]) {
      acc[curr.decidedGateway] = { total: 0, success: 0, failure: 0 }
    }
    acc[curr.decidedGateway].total++
    if (curr.status === 'CHARGED') acc[curr.decidedGateway].success++
    else acc[curr.decidedGateway].failure++
    return acc
  }, {} as Record<string, { total: number; success: number; failure: number }>)

  const pieData = volumeDistribution.map(d => ({ name: d.name, value: d.count }))
  const debitNetworkRows = debitResult?.debit_routing_output?.co_badged_card_networks_info || []
  const volumeColorIndex = useMemo(
    () => new Map(volumeDistribution.map((item, index) => [item.name, index] as const)),
    [volumeDistribution],
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
    if (!selectedPreviewPaymentId) return
    void previewTraceDetail.mutate()
  }, [selectedPreviewPaymentId])

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

    if (activeTab === 'single') {
      setForm(defaults.form)
      setResult(defaults.result)
      setSingleRunPaymentId(defaults.singleRunPaymentId)
      setSingleRunOutcome(defaults.singleRunOutcome)
      setResponseOpen(defaults.responseOpen)
    } else if (activeTab === 'batch') {
      setForm(defaults.form)
      setSimulationConfig(defaults.simulationConfig)
      setSuccessRate(70)
      setSimulationResults(defaults.simulationResults)
      setIsSimulating(false)
    } else if (activeTab === 'rule') {
      setRuleParams(defaults.ruleParams)
      setFallbackConnectors(defaults.fallbackConnectors)
      setRuleResult(defaults.ruleResult)
      setSelectedPreviewPaymentId(null)
      setSelectedPreviewEventId(null)
      setPreviewInspectorTab('summary')
      setPreviewTraceLabel('Rule Evaluation Decision')
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
      ? 'Reset Auth-Rate Based Routing'
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

      <div className="rounded-2xl border border-slate-200 bg-slate-50/80 p-1 dark:border-[#242b36] dark:bg-[#0c1118]">
        <div className="grid gap-1 sm:grid-cols-2 xl:grid-cols-4">
          <button
            onClick={() => setActiveTab('batch')}
            className={`rounded-xl border px-4 py-3 text-left text-sm font-semibold transition ${explorerModeButtonClass(activeTab === 'batch')}`}
          >
            <span className="block">Auth-rate</span>
            <span className="mt-1 block text-[11px] font-medium text-slate-400 dark:text-[#738097]">Score simulation</span>
          </button>
          <button
            onClick={() => setActiveTab('rule')}
            className={`rounded-xl border px-4 py-3 text-left text-sm font-semibold transition ${explorerModeButtonClass(activeTab === 'rule')}`}
          >
            <span className="block">Rule based</span>
            <span className="mt-1 block text-[11px] font-medium text-slate-400 dark:text-[#738097]">Policy evaluator</span>
          </button>
          <button
            onClick={() => setActiveTab('volume')}
            className={`rounded-xl border px-4 py-3 text-left text-sm font-semibold transition ${explorerModeButtonClass(activeTab === 'volume')}`}
          >
            <span className="block">Volume split</span>
            <span className="mt-1 block text-[11px] font-medium text-slate-400 dark:text-[#738097]">Distribution run</span>
          </button>
          <button
            onClick={() => setActiveTab('debit')}
            className={`rounded-xl border px-4 py-3 text-left text-sm font-semibold transition ${explorerModeButtonClass(activeTab === 'debit')}`}
          >
            <span className="block">Debit routing</span>
            <span className="mt-1 block text-[11px] font-medium text-slate-400 dark:text-[#738097]">Network decision</span>
          </button>
        </div>
      </div>

      <div className={activeTab === 'volume'
        ? 'grid grid-cols-1 gap-5 xl:grid-cols-[minmax(340px,420px)_minmax(0,1fr)]'
        : 'grid grid-cols-1 gap-6 lg:grid-cols-2'
      }>
        <Card className="!rounded-2xl self-start">
          <CardHeader className="!px-5 !py-4">
            <div>
              <SurfaceLabel>
                {activeTab === 'rule' ? 'Rule Evaluation' :
                  activeTab === 'volume' ? 'Volume Split' :
                    activeTab === 'debit' ? 'Network Routing' :
                      'Simulation'}
              </SurfaceLabel>
              <h2 className="mt-3 font-medium text-slate-800 dark:text-white">
                {activeTab === 'rule' ? 'Rule Evaluation Parameters' :
                  activeTab === 'volume' ? 'Volume Split Configuration' :
                    activeTab === 'debit' ? 'Debit Routing Parameters' :
                      'Auth-Rate Based Routing Parameters'}
              </h2>
            </div>
          </CardHeader>
          <CardBody className="space-y-4 !px-5 !py-5">
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
                <div>
                  <label className="block text-xs font-medium text-slate-600 mb-1">Parameters</label>
                  <div className="space-y-2">
                    {ruleParams.map((param, idx) => (
                      <div key={idx} className="space-y-2">
                        <div className="flex gap-2 items-center">
                          <select
                            value={param.key}
                            onChange={e => updateRuleParamKey(idx, e.target.value)}
                            disabled={routingConfigUnavailable || routingKeysLoading}
                            className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                          >
                            {routingKeyNames.length === 0 ? (
                              <option value="">No keys available</option>
                            ) : (
                              routingKeyNames.map(name => <option key={name} value={name}>{name}</option>)
                            )}
                          </select>
                          <input
                            value={param.type}
                            readOnly
                            className="w-36 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                          />
                          <button
                            onClick={() => removeRuleParam(idx)}
                            className="p-1.5 text-slate-400 hover:text-red-500"
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                        {param.type === 'metadata_variant' ? (
                          <div className="flex gap-2 items-center pl-1">
                            <input
                              placeholder="Metadata Key"
                              value={param.metadataKey || ''}
                              onChange={e => updateRuleParamMetadataKey(idx, e.target.value)}
                              className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                            <input
                              placeholder="Metadata Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        ) : param.type === 'enum_variant' ? (
                          <div className="flex gap-2 items-center pl-1">
                            <select
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            >
                              {(routingKeysConfig[param.key]?.values || []).map(v => (
                                <option key={v} value={v}>{v}</option>
                              ))}
                            </select>
                          </div>
                        ) : param.type === 'number' ? (
                          <div className="flex gap-2 items-center pl-1">
                            <input
                              type="number"
                              placeholder="Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        ) : (
                          <div className="flex gap-2 items-center pl-1">
                            <input
                              placeholder="Value"
                              value={param.value}
                              onChange={e => updateRuleParam(idx, 'value', e.target.value)}
                              className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addRuleParam}
                    disabled={routingConfigUnavailable || routingKeysLoading || routingKeyNames.length === 0}
                    className="mt-2 flex items-center gap-1 text-xs text-brand-500 hover:text-brand-600"
                  >
                    <Plus size={12} /> Add Parameter
                  </button>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-600 mb-1">Fallback gateway_name/gateway_id</label>
                  <div className="space-y-2">
                    {fallbackConnectors.map((connector, idx) => (
                      <div key={idx} className="flex gap-2 items-center">
                        <input
                          placeholder="gateway_name"
                          value={connector.gateway_name}
                          onChange={e => updateFallbackConnector(idx, 'gateway_name', e.target.value)}
                          className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                        <input
                          placeholder="gateway_id"
                          value={connector.gateway_id || ''}
                          onChange={e => updateFallbackConnector(idx, 'gateway_id', e.target.value)}
                          className="flex-1 border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                        <button
                          onClick={() => removeFallbackConnector(idx)}
                          className="p-1.5 text-slate-400 hover:text-red-500"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    ))}
                  </div>
                  <button
                    onClick={addFallbackConnector}
                    className="mt-2 flex items-center gap-1 text-xs text-brand-500 hover:text-brand-600"
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
                    Debit routing is enabled for this merchant. This tab will call /decide-gateway with NTW_BASED_ROUTING.
                  </p>
                ) : (
                  <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <span>Debit routing is disabled for this merchant.</span>
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
                <div className="rounded-xl border border-slate-200 bg-slate-50 p-4 dark:border-[#283241] dark:bg-[#0b111a]">
                  <label className="block text-xs font-semibold uppercase tracking-[0.14em] text-slate-500 dark:text-[#7f8ca3]">
                    Evaluation count
                  </label>
                  <div className="mt-3 flex items-center gap-3 rounded-xl border border-slate-200 bg-white px-3 py-2 dark:border-[#283241] dark:bg-[#101722]">
                    <input
                      type="number"
                      min="1"
                      inputMode="numeric"
                      value={volumePayments}
                      onChange={e => setVolumePayments(e.target.value)}
                      className="min-w-0 flex-1 bg-transparent text-2xl font-semibold text-slate-950 outline-none dark:text-white"
                    />
                    <span className="rounded-full bg-slate-100 px-2.5 py-1 text-xs font-semibold text-slate-500 dark:bg-[#1d2633] dark:text-[#91a0b8]">
                      runs
                    </span>
                  </div>
                  <p className="mt-3 text-xs leading-5 text-slate-500 dark:text-[#8f9bb0]">
                    Samples the active volume split strategy through <code>/routing/evaluate</code> and records each decision trace.
                  </p>
                </div>

                <div className="grid grid-cols-2 gap-3">
                  <div className="rounded-xl border border-slate-200 px-3 py-3 dark:border-[#283241]">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#77849a]">Target</p>
                    <p className="mt-2 text-lg font-semibold text-slate-950 dark:text-white">{volumeRunTarget || '--'}</p>
                  </div>
                  <div className="rounded-xl border border-slate-200 px-3 py-3 dark:border-[#283241]">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#77849a]">Completed</p>
                    <p className="mt-2 text-lg font-semibold text-slate-950 dark:text-white">
                      {loading ? volumeProgress : volumeEvaluationCount || '--'}
                    </p>
                  </div>
                </div>

                {loading && activeTab === 'volume' ? (
                  <div className="rounded-xl border border-sky-200 bg-sky-50 px-3 py-3 dark:border-sky-500/25 dark:bg-sky-500/10">
                    <div className="flex items-center justify-between text-xs font-semibold text-sky-700 dark:text-sky-200">
                      <span>Run progress</span>
                      <span>{volumeProgressPercentage}%</span>
                    </div>
                    <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-sky-100 dark:bg-sky-950/70">
                      <div className="h-full rounded-full bg-sky-500" style={{ width: `${volumeProgressPercentage}%` }} />
                    </div>
                  </div>
                ) : null}
              </div>
            ) : (
              <>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Amount</label>
                    <input value={form.amount} onChange={e => set('amount', e.target.value)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500" />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Currency</label>
                    <select value={form.currency} onChange={e => set('currency', e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {currencyOptions.map(c => <option key={c}>{c}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Payment Method Type</label>
                    <select value={form.payment_method_type} onChange={e => set('payment_method_type', e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {paymentMethodTypeOptions.map(p => <option key={p}>{p}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Payment Method</label>
                    <select value={form.payment_method} onChange={e => set('payment_method', e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {paymentMethodOptions.map(p => <option key={p}>{p}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Card Brand</label>
                    <select value={form.card_brand} onChange={e => set('card_brand', e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {cardBrandOptions.map(b => <option key={b}>{b}</option>)}
                    </select>
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Auth Type</label>
                    <select value={form.auth_type} onChange={e => set('auth_type', e.target.value)}
                      disabled={routingConfigUnavailable || routingKeysLoading}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {authTypeOptions.map(a => <option key={a}>{a}</option>)}
                    </select>
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-slate-600 mb-1">Eligible Gateways (comma-separated)</label>
                  <input value={form.eligible_gateways} onChange={e => set('eligible_gateways', e.target.value)}
                    placeholder="stripe, adyen"
                    className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500" />
                </div>

                <div>
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Algorithm</label>
                    <select value={form.ranking_algorithm} onChange={e => set('ranking_algorithm', e.target.value as RoutingAlgorithmName)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {ALGORITHMS.map(a => <option key={a} value={a}>{ALGORITHM_LABELS[a]}</option>)}
                    </select>
                  </div>
                </div>

                {activeTab === 'single' && (
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
                )}

                {activeTab === 'batch' && (
                  <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-4 mt-4 space-y-3">
                    <h3 className="text-sm font-medium text-slate-800 dark:text-slate-200 flex items-center gap-2">
                      <Activity size={14} />
                      Simulation
                    </h3>
                    <div className="flex items-end gap-3">
                      <div className="w-28">
                        <label className="block text-xs font-medium text-slate-600 dark:text-slate-400 mb-1">Payments</label>
                        <input
                          type="number"
                          min={1}
                          max={1000}
                          value={simulationConfig.totalPayments}
                          onChange={e => {
                            const total = Math.max(1, parseInt(e.target.value) || 1)
                            const s = Math.round(total * successRate / 100)
                            setSimulationConfig({ totalPayments: String(total), successCount: String(s), failureCount: String(total - s) })
                          }}
                          className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                      <div className="flex-1">
                        <div className="flex justify-between mb-1">
                          <label className="text-xs font-medium text-slate-600 dark:text-slate-400">Success Rate</label>
                          <span className="text-xs font-semibold text-brand-600 dark:text-sky-400">{successRate}%</span>
                        </div>
                        <input
                          type="range"
                          min={0}
                          max={100}
                          value={successRate}
                          onChange={e => {
                            const rate = parseInt(e.target.value)
                            const total = parseInt(simulationConfig.totalPayments) || 10
                            const s = Math.round(total * rate / 100)
                            setSuccessRate(rate)
                            setSimulationConfig({ totalPayments: String(total), successCount: String(s), failureCount: String(total - s) })
                          }}
                          className="w-full accent-brand-600"
                        />
                        <div className="flex justify-between mt-1 text-[10px] text-slate-400">
                          <span>{simulationConfig.successCount} success</span>
                          <span>{simulationConfig.failureCount} failure</span>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
              </>
            )}

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
            ) : activeTab === 'volume' ? (
              <Button
                onClick={runVolumeSplit}
                disabled={loading || !effectiveMerchantId}
                className="w-full justify-center dark:bg-sky-500 dark:text-white dark:hover:bg-sky-400"
              >
                {loading ? (
                  <><Spinner size={14} /> Running {volumeProgress}/{volumePayments || 0} decisions…</>
                ) : (
                  <><PieChartIcon size={14} /> Run Volume Evaluation</>
                )}
              </Button>
            ) : activeTab === 'batch' ? (
              <Button onClick={runSimulation} disabled={isSimulating || !effectiveMerchantId || routingConfigUnavailable} className="w-full justify-center">
                {isSimulating ? (
                  <>
                    <Spinner size={14} />
                    Simulating {simulationResults.length}/{simulationConfig.totalPayments || 0}...
                  </>
                ) : (
                  <>
                    <Activity size={14} /> Run Auth-Rate Simulation
                  </>
                )}
              </Button>
            ) : (
              <Button onClick={run} disabled={loading || !effectiveMerchantId || routingConfigUnavailable} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Running…</> : <><Play size={14} /> Run Single Transaction</>}
              </Button>
            )}
          </CardBody>
        </Card>

        <div
          className={
            activeTab === 'volume' && volumeDistribution.length > 0
              ? 'min-w-0 space-y-4'
              : 'min-w-0 space-y-4'
          }
        >
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
                        <pre className="mt-3 max-h-96 overflow-auto rounded-lg bg-slate-950 p-4 text-xs text-slate-200">
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
                  <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 px-5 py-4 dark:border-[#263141]">
                    <div>
                      <SurfaceLabel>Volume result</SurfaceLabel>
                      <h3 className="mt-2 text-lg font-semibold text-slate-950 dark:text-white">Distribution analysis</h3>
                      <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#8f9bb0]">
                        {volumeEvaluationCount} evaluations from <code>/routing/evaluate</code> using the active volume split rule.
                      </p>
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

                  <div className="grid gap-5 px-5 py-5 2xl:grid-cols-[minmax(0,1fr)_300px]">
                    <div className="min-w-0 space-y-5">
                      <div className="grid gap-3 sm:grid-cols-3">
                        <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-[#293546] dark:bg-[#0b111a]">
                          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-[#77849a]">Runs</p>
                          <p className="mt-2 text-2xl font-semibold text-slate-950 dark:text-white">{volumeEvaluationCount}</p>
                        </div>
                        <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-[#293546] dark:bg-[#0b111a]">
                          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-[#77849a]">Leader</p>
                          <p className="mt-2 truncate text-2xl font-semibold text-slate-950 dark:text-white">{volumeLeader?.name || '--'}</p>
                        </div>
                        <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 dark:border-[#293546] dark:bg-[#0b111a]">
                          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400 dark:text-[#77849a]">Share</p>
                          <p className="mt-2 text-2xl font-semibold text-slate-950 dark:text-white">{volumeLeader?.percentage ?? 0}%</p>
                        </div>
                      </div>

                      <div>
                        <div className="flex items-center justify-between text-xs text-slate-500 dark:text-[#8f9bb0]">
                          <span>Observed percentage split</span>
                          <span>{sortedVolumeDistribution.length} gateways</span>
                        </div>
                        <div className="mt-2 flex h-3 overflow-hidden rounded-full bg-slate-100 dark:bg-[#1d2633]">
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

                      <div className="space-y-3">
                        {sortedVolumeDistribution.map((item) => {
                          const color = COLORS[(volumeColorIndex.get(item.name) ?? 0) % COLORS.length]
                          return (
                            <div key={item.name} className="rounded-xl border border-slate-200 bg-white px-4 py-3 dark:border-[#263141] dark:bg-[#0c121c]">
                              <div className="flex items-center justify-between gap-4">
                                <div className="flex min-w-0 items-center gap-3">
                                  <span className="h-3 w-3 shrink-0 rounded-full" style={{ backgroundColor: color }} />
                                  <div className="min-w-0">
                                    <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">{item.name}</p>
                                    <p className="text-xs text-slate-500 dark:text-[#8290a5]">{item.count} payments</p>
                                  </div>
                                </div>
                                <p className="text-sm font-semibold text-slate-900 dark:text-white">{item.percentage}%</p>
                              </div>
                              <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-slate-100 dark:bg-[#1d2633]">
                                <div className="h-full rounded-full" style={{ width: `${item.percentage}%`, backgroundColor: color }} />
                              </div>
                            </div>
                          )
                        })}
                      </div>
                    </div>

                    <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-4 dark:border-[#293546] dark:bg-[#0b111a]">
                      <div className="flex items-center justify-between">
                        <p className="text-sm font-semibold text-slate-900 dark:text-white">Connector share</p>
                        <Badge variant="gray">Live sample</Badge>
                      </div>
                      <div className="mt-4 h-[260px]">
                        <ResponsiveContainer width="100%" height="100%">
                          <PieChart>
                            <Pie
                              data={pieData}
                              cx="50%"
                              cy="50%"
                              innerRadius={72}
                              outerRadius={104}
                              paddingAngle={2}
                              dataKey="value"
                              label={false}
                              labelLine={false}
                            >
                              {pieData.map((_, index) => (
                                <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                              ))}
                            </Pie>
                            <Tooltip
                              formatter={(value: number) => [`${value} payments`, 'Count']}
                              contentStyle={{ backgroundColor: '#111827', border: '1px solid #293546', borderRadius: '12px', color: '#fff' }}
                            />
                          </PieChart>
                        </ResponsiveContainer>
                      </div>
                      <div className="mt-4 space-y-2">
                        {sortedVolumeDistribution.map((item) => (
                          <div key={item.name} className="flex items-center justify-between gap-3 text-xs">
                            <span className="flex min-w-0 items-center gap-2 text-slate-500 dark:text-[#9aa6bb]">
                              <span
                                className="h-2.5 w-2.5 rounded-full"
                                style={{ backgroundColor: COLORS[(volumeColorIndex.get(item.name) ?? 0) % COLORS.length] }}
                              />
                              <span className="truncate">{item.name}</span>
                            </span>
                            <span className="font-semibold text-slate-900 dark:text-white">{item.percentage}%</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </section>

                <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(360px,0.8fr)]">
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
                          {volumeEvaluationLog.map((entry, idx) => (
                            <tr
                              key={entry.paymentId}
                              className="cursor-pointer transition hover:bg-slate-50 dark:hover:bg-[#151d2a]"
                              onClick={() => openPreviewModal(entry.paymentId, 'Volume Split Decision')}
                            >
                              <td className="px-4 py-2 font-mono text-xs text-slate-500">{idx + 1}</td>
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
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </section>

                  <section className="rounded-2xl border border-slate-200 bg-white dark:border-[#283241] dark:bg-[#101722]">
                    <div className="border-b border-slate-200 px-5 py-4 dark:border-[#263141]">
                      <h3 className="text-sm font-semibold text-slate-900 dark:text-white">Gateway totals</h3>
                    </div>
                    <div className="p-5">
                      <table className="w-full text-sm">
                        <tbody className="divide-y divide-slate-100 dark:divide-[#263141]">
                          {sortedVolumeDistribution.map((item) => (
                            <tr key={item.name}>
                              <td className="py-3">
                                <div className="flex items-center gap-2">
                                  <span
                                    className="h-2.5 w-2.5 rounded-full"
                                    style={{ backgroundColor: COLORS[(volumeColorIndex.get(item.name) ?? 0) % COLORS.length] }}
                                  />
                                  <span className="font-medium text-slate-900 dark:text-white">{item.name}</span>
                                </div>
                              </td>
                              <td className="py-3 text-right text-slate-500 dark:text-[#9aa6bb]">{item.count}</td>
                              <td className="py-3 text-right font-semibold text-slate-900 dark:text-white">{item.percentage}%</td>
                            </tr>
                          ))}
                          <tr>
                            <td className="py-3 font-semibold text-slate-900 dark:text-white">Total</td>
                            <td className="py-3 text-right font-semibold text-slate-900 dark:text-white">{volumeEvaluationCount}</td>
                            <td className="py-3 text-right font-semibold text-slate-900 dark:text-white">100%</td>
                          </tr>
                        </tbody>
                      </table>
                    </div>
                  </section>
                </div>

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
                  <h3 className="mt-4 text-sm font-semibold text-slate-900 dark:text-white">No volume sample yet</h3>
                  <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-[#9aa6bb]">
                    Set the run size, execute the volume test, then inspect distribution and traces here.
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
            hasSimulationActivity ? (
              <>
                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Simulation Progress</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="mb-4">
                      <div className="flex justify-between text-xs text-slate-600 mb-1">
                        <span>Progress</span>
                        <span>{simulationProgressPercentage}%</span>
                      </div>
                      <div className="w-full overflow-hidden rounded-full bg-gray-200 h-2">
                        <div
                          className={`h-2 rounded-full bg-brand-500 transition-[width] duration-300 ease-out ${isSimulating && completedSimulationCount === 0 ? 'animate-pulse' : ''}`}
                          style={{ width: `${simulationProgressPercentage}%` }}
                        />
                      </div>
                      <p className="mt-2 text-xs text-slate-500">
                        {completedSimulationCount} of {totalSimulationPayments || 0} payments processed.
                      </p>
                    </div>

                    {Object.keys(gatewayStats).length > 0 && (
                      <div className="space-y-2">
                        <h4 className="text-xs font-medium text-slate-700">Gateway Selection Summary</h4>
                        {Object.entries(gatewayStats).map(([gateway, stats]) => (
                          <div key={gateway} className="flex items-center justify-between text-sm">
                            <span className="font-medium">{gateway}</span>
                            <div className="flex gap-3 text-xs">
                              <span className="text-emerald-600">{stats.success} ✓</span>
                              <span className="text-red-500">{stats.failure} ✗</span>
                              <span className="text-slate-500">({stats.total} total)</span>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Transaction Log</h3>
                  </CardHeader>
                  <CardBody className="p-0 max-h-96 overflow-auto">
                    {deferredSimulationResults.length > 0 ? (
                      <table className="w-full text-sm">
                        <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-xs text-slate-500 sticky top-0">
                          <tr>
                            <th className="text-left px-3 py-2">#</th>
                            <th className="text-left px-3 py-2">Payment ID</th>
                            <th className="text-left px-3 py-2">Gateway</th>
                            <th className="text-left px-3 py-2">Outcome</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-[#1c1c24]">
                          {deferredSimulationResults.map((res, idx) => (
                            <tr key={res.paymentId} className="hover:bg-slate-100 dark:bg-[#0f0f16]">
                              <td className="px-3 py-2 text-slate-500">{idx + 1}</td>
                              <td className="px-3 py-2">
                                <button
                                  type="button"
                                  title={res.paymentId}
                                  onClick={() => openAuditModal(res.paymentId)}
                                  className="group flex items-start gap-3 text-left"
                                >
                                  <span className="inline-flex h-8 w-8 items-center justify-center rounded-full bg-brand-500/10 text-[11px] font-semibold uppercase tracking-[0.16em] text-brand-600 dark:text-brand-300">
                                    {idx + 1}
                                  </span>
                                  <span className="min-w-0">
                                    <span className="block truncate font-mono text-xs font-semibold text-slate-900 transition group-hover:text-brand-600 dark:text-white">
                                      {res.paymentId}
                                    </span>
                                    <span className="mt-1 block text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400 transition group-hover:text-brand-500">
                                      View audit
                                    </span>
                                  </span>
                                </button>
                              </td>
                              <td className="px-3 py-2 font-medium">{res.decidedGateway}</td>
                              <td className="px-3 py-2">
                                <Badge variant={res.status === 'CHARGED' ? 'green' : 'red'}>
                                  {res.status}
                                </Badge>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
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
              <Card>
                <CardBody className="py-16 text-center">
                  <Activity size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Configure simulation parameters and click "Run Auth-Rate Simulation" to test auth-rate based routing.</p>
                </CardBody>
              </Card>
            )
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
                          <Tooltip formatter={v => `${v}%`} contentStyle={{ backgroundColor: '#0d0d12', border: '1px solid #1c1c24', borderRadius: '8px', color: '#e8e8f4' }} />
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
                <Button onClick={() => setSetupPrompt(null)}>
                  Dismiss
                </Button>
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
                          ? 'We already found the decision summary for this run, but the step-by-step timeline has not been flushed yet. Waiting for the latest events.'
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
                          ? 'The decision record exists, but no inspectable step payload has arrived yet. The inspector will unlock as soon as the first timeline event is available.'
                          : 'Inspector will unlock as soon as the first decision event is available.'
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
