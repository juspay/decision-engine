import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, PieChart, Pie } from 'recharts'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost, fetcher } from '../../lib/api'
import { DecideGatewayResponse, GatewayConnector, PaymentAuditEvent, PaymentAuditResponse } from '../../types/api'
import { ROUTING_APPROACH_COLORS } from '../../lib/constants'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import { Play, RefreshCw, ChevronDown, ChevronUp, Activity, Code, Plus, Trash2, PieChart as PieChartIcon, X } from 'lucide-react'

const ALGORITHMS = ['SR_BASED_ROUTING', 'PL_BASED_ROUTING', 'NTW_BASED_ROUTING']

const ALGORITHM_LABELS: Record<string, string> = {
  'SR_BASED_ROUTING': 'Success Rate Based',
  'PL_BASED_ROUTING': 'Priority List Based',
  'NTW_BASED_ROUTING': 'Network Based'
}

type TabType = 'single' | 'batch' | 'rule' | 'volume'

interface FormState {
  amount: string
  currency: string
  payment_method_type: string
  payment_method: string
  card_brand: string
  auth_type: string
  eligible_gateways: string
  ranking_algorithm: string
  elimination_enabled: boolean
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

type AuditInspectorTab = 'summary' | 'input' | 'response' | 'raw'

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
  connector: string
  colorIdx: number
}

function toUpperOptions(values: string[] = []): string[] {
  return values.map(v => v.trim()).filter(Boolean).map(v => v.toUpperCase())
}

function uniqueUpperOptions(values: string[] = []): string[] {
  return Array.from(new Set(toUpperOptions(values)))
}

function mulberry32(seed: number) {
  return function random() {
    let t = seed += 0x6D2B79F5
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

function buildVolumePaymentLog(
  distribution: Array<{ name: string; count: number; percentage: number }>,
  totalPayments: number,
): VolumePaymentEntry[] {
  const total = Math.max(0, totalPayments)
  if (!distribution.length || total === 0) return []

  const payments: VolumePaymentEntry[] = []

  distribution.forEach((item, idx) => {
    for (let count = 0; count < item.count; count += 1) {
      payments.push({ connector: item.name, colorIdx: idx })
    }
  })

  const rankedConnectors = distribution
    .map((item, idx) => ({ connector: item.name, colorIdx: idx, percentage: item.percentage }))
    .sort((a, b) => b.percentage - a.percentage)

  while (payments.length < total) {
    const filler = rankedConnectors[payments.length % rankedConnectors.length]
    payments.push({ connector: filler.connector, colorIdx: filler.colorIdx })
  }

  if (payments.length > total) {
    payments.length = total
  }

  const seed = distribution.reduce((acc, item, idx) => {
    const connectorScore = Array.from(item.name).reduce((sum, char) => sum + char.charCodeAt(0), 0)
    return acc + connectorScore + idx * 31 + item.count * 17 + Math.round(item.percentage * 10)
  }, total * 13)

  const random = mulberry32(seed)
  const shuffled = [...payments]
  for (let idx = shuffled.length - 1; idx > 0; idx -= 1) {
    const swapIndex = Math.floor(random() * (idx + 1))
    ;[shuffled[idx], shuffled[swapIndex]] = [shuffled[swapIndex], shuffled[idx]]
  }

  return shuffled
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
  if (eventType === 'decision') return 'Decide Gateway'
  if (eventType === 'gateway_update') return 'Update Gateway'
  if (eventType === 'rule_hit') return 'Rule Evaluate'
  if (eventType === 'error') return 'Errors'
  return humanizeAuditValue(eventType)
}

function stageLabel(event: PaymentAuditEvent) {
  if (event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_stage === 'score_updated') return 'Update Gateway'
  if (event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_type === 'error') return 'Errors'
  return humanizeAuditValue(event.event_stage || event.event_type)
}

function eventPhase(event: PaymentAuditEvent) {
  if (event.event_type === 'decision' || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_type === 'rule_hit' || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_type === 'gateway_update' || event.event_stage === 'score_updated') return 'Update Gateway'
  return 'Errors'
}

function badgeVariantForEvent(event: PaymentAuditEvent): 'blue' | 'green' | 'purple' | 'red' | 'orange' | 'gray' {
  const normalizedStatus = (event.status || '').toUpperCase()
  if (
    event.event_type === 'error' ||
    normalizedStatus === 'FAILURE' ||
    normalizedStatus.includes('FAILED') ||
    normalizedStatus.includes('DECLINED')
  ) return 'red'
  if (event.event_type === 'rule_hit') return 'purple'
  if (
    normalizedStatus === 'CHARGED' ||
    normalizedStatus === 'AUTHORIZED' ||
    normalizedStatus === 'SUCCESS'
  ) return 'green'
  if (event.event_type === 'gateway_update') return 'green'
  if (event.event_type === 'decision') return 'blue'
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
  if (phase === 'Update Gateway') return 'green'
  if (phase === 'Errors') return 'red'
  return 'gray'
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

function buildAuditUrl(merchantId: string, paymentId: string) {
  const qs = queryString({
    scope: 'current',
    range: '24h',
    page: 1,
    page_size: 25,
    merchant_id: merchantId,
    payment_id: paymentId,
  })
  return `/analytics/payment-audit?${qs}`
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
      event_type: event.event_type,
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
    ? 'bg-brand-600 text-white'
    : 'bg-white text-slate-600 border border-slate-200 hover:bg-slate-50 dark:bg-[#121214] dark:text-[#a1a1aa] dark:border-[#27272a]'
}

function EmptyAuditState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-[22px] border border-dashed border-slate-200 bg-slate-50/80 px-6 py-12 text-center dark:border-[#1f1f26] dark:bg-[#0b0b0f]">
      <p className="text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#8a8a93]">{body}</p>
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
          className="rounded-[20px] border border-slate-200 bg-slate-50/80 px-4 py-3 dark:border-[#1d1d23] dark:bg-[#0b0b10]"
        >
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
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
        <pre className="overflow-x-auto rounded-[22px] bg-slate-950 px-4 py-4 text-xs leading-6 text-slate-200">
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
  const { routingKeysConfig, isLoading: routingKeysLoading, error: routingKeysError } = useDynamicRoutingConfig()
  const hasRoutingKeys = Object.keys(routingKeysConfig).length > 0
  const routingConfigUnavailable = !routingKeysLoading && (!hasRoutingKeys || Boolean(routingKeysError))
  const [activeTab, setActiveTab] = useState<TabType>('single')

  const [form, setForm] = useState<FormState>({
    amount: '1000',
    currency: '',
    payment_method_type: '',
    payment_method: '',
    card_brand: '',
    auth_type: '',
    eligible_gateways: 'stripe, adyen',
    ranking_algorithm: 'SR_BASED_ROUTING',
    elimination_enabled: false,
  })

  const [simulationConfig, setSimulationConfig] = useState<SimulationConfig>({
    totalPayments: '10',
    successCount: '7',
    failureCount: '3',
  })

  const [ruleParams, setRuleParams] = useState<RuleEvaluateParams[]>([
    { key: 'payment_method_type', type: 'enum_variant', value: '', metadataKey: '' },
    { key: 'currency', type: 'enum_variant', value: '', metadataKey: '' },
  ])

  const [fallbackConnectors, setFallbackConnectors] = useState<GatewayConnector[]>([
    { gateway_name: 'stripe', gateway_id: 'gateway_001' },
    { gateway_name: 'adyen', gateway_id: 'gateway_002' },
  ])

  const [volumePayments, setVolumePayments] = useState<string>('100')

  const [result, setResult] = useState<DecideGatewayResponse | null>(null)
  const [ruleResult, setRuleResult] = useState<RuleEvaluateResponse | null>(null)
  const [volumeDistribution, setVolumeDistribution] = useState<{ name: string; count: number; percentage: number }[]>([])
  const [simulationResults, setSimulationResults] = useState<SimulationResult[]>([])
  const [isSimulating, setIsSimulating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [filterOpen, setFilterOpen] = useState(false)
  const [responseOpen, setResponseOpen] = useState(false)
  const [volumeResponseOpen, setVolumeResponseOpen] = useState(false)
  const [selectedAuditPaymentId, setSelectedAuditPaymentId] = useState<string | null>(null)
  const [selectedAuditEventId, setSelectedAuditEventId] = useState<number | null>(null)
  const [auditInspectorTab, setAuditInspectorTab] = useState<AuditInspectorTab>('summary')

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

  const auditUrl = merchantId && selectedAuditPaymentId
    ? buildAuditUrl(merchantId, selectedAuditPaymentId)
    : null

  const auditDetail = useSWR<PaymentAuditResponse>(auditUrl, fetcher, {
    refreshInterval: selectedAuditPaymentId ? 12000 : 0,
    revalidateOnFocus: true,
  })

  useEffect(() => {
    if (routingConfigUnavailable || routingKeysLoading) return

    setForm(prev => {
      const next = { ...prev }

      if (currencyOptions.length > 0 && !currencyOptions.includes(next.currency)) {
        next.currency = currencyOptions[0]
      }

      if (paymentMethodTypeOptions.length > 0 && !paymentMethodTypeOptions.includes(next.payment_method_type)) {
        next.payment_method_type = paymentMethodTypeOptions[0]
      }

      const methodsForType = toUpperOptions(
        routingKeysConfig[next.payment_method_type.toLowerCase()]?.values || []
      )
      if (methodsForType.length > 0 && !methodsForType.includes(next.payment_method)) {
        next.payment_method = methodsForType[0]
      }

      if (authTypeOptions.length > 0 && !authTypeOptions.includes(next.auth_type)) {
        next.auth_type = authTypeOptions[0]
      }

      if (cardBrandOptions.length > 0 && !cardBrandOptions.includes(next.card_brand)) {
        next.card_brand = cardBrandOptions[0]
      }

      return next
    })

    setRuleParams(prev =>
      prev.map(param => {
        if (!param.key || !routingKeysConfig[param.key]) return param
        const keyConfig = routingKeysConfig[param.key]
        const mappedType = mapRoutingTypeToRuleParamType(keyConfig.type)
        const enumValues = keyConfig.values || []
        const nextValue = mappedType === 'enum_variant'
          ? (enumValues.includes(param.value) ? param.value : (enumValues[0] || ''))
          : param.value
        return { ...param, type: mappedType, value: nextValue }
      })
    )
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
    if (!selectedAuditPaymentId) return

    const previousOverflow = document.body.style.overflow
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setSelectedAuditPaymentId(null)
        setSelectedAuditEventId(null)
        setAuditInspectorTab('summary')
      }
    }

    document.body.style.overflow = 'hidden'
    window.addEventListener('keydown', onKeyDown)

    return () => {
      document.body.style.overflow = previousOverflow
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [selectedAuditPaymentId])

  function set(field: keyof FormState, value: string | boolean) {
    setForm(f => ({ ...f, [field]: value }))
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
    if (!merchantId) return setError('Set a merchant ID in the top bar')
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')
    setLoading(true); setError(null)
    const gateways = form.eligible_gateways.split(',').map(s => s.trim()).filter(Boolean)
    try {
      const res = await apiPost<DecideGatewayResponse>('/decide-gateway', {
        merchantId: merchantId,
        paymentInfo: {
          paymentId: `explorer_${Date.now()}`,
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
      setResult(res)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  async function runSimulation() {
    if (!merchantId) return setError('Set a merchant ID in the top bar')
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
          merchantId: merchantId,
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
          merchantId: merchantId,
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
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Simulation failed')
    } finally {
      setIsSimulating(false)
    }
  }

  async function runRuleEvaluation() {
    if (routingConfigUnavailable) return setError('Routing key config unavailable. Fix /config/routing-keys and retry.')
    setLoading(true)
    setError(null)
    setRuleResult(null)
    setVolumeDistribution([])

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
        created_by: merchantId || 'test_user',
        fallback_output: fallbackConnectors.filter(c => c.gateway_name),
        parameters,
      })

      setRuleResult(res)

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
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  async function runVolumeSplit() {
    setLoading(true)
    setError(null)
    setVolumeDistribution([])

    try {
      const res = await apiPost<RuleEvaluateResponse>('/routing/evaluate', {
        created_by: merchantId || 'test_user',
        fallback_output: [
          { gateway_name: 'stripe', gateway_id: 'gateway_001' },
          { gateway_name: 'adyen', gateway_id: 'gateway_002' },
        ],
        parameters: {},
      })

      setRuleResult(res)

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
      setError(e instanceof Error ? e.message : 'Request failed')
    } finally {
      setLoading(false)
    }
  }

  const scoreData = result?.gateway_priority_map
    ? Object.entries(result.gateway_priority_map)
      .sort(([, a], [, b]) => b - a)
      .map(([name, score]) => ({ name, score: Math.round(score * 1000) / 10 }))
    : []

  const gatewayStats = simulationResults.reduce((acc, curr) => {
    if (!acc[curr.decidedGateway]) {
      acc[curr.decidedGateway] = { total: 0, success: 0, failure: 0 }
    }
    acc[curr.decidedGateway].total++
    if (curr.status === 'CHARGED') acc[curr.decidedGateway].success++
    else acc[curr.decidedGateway].failure++
    return acc
  }, {} as Record<string, { total: number; success: number; failure: number }>)

  const pieData = volumeDistribution.map(d => ({ name: d.name, value: d.count }))
  const simulatedVolumePayments = useMemo(
    () => buildVolumePaymentLog(volumeDistribution, parseInt(volumePayments) || 0),
    [volumeDistribution, volumePayments],
  )

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

  function openAuditModal(paymentId: string) {
    setSelectedAuditPaymentId(paymentId)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
  }

  function closeAuditModal() {
    setSelectedAuditPaymentId(null)
    setSelectedAuditEventId(null)
    setAuditInspectorTab('summary')
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-900">Decision Explorer</h1>
        <p className="text-slate-500 mt-1 text-sm">
          Test payment routing with different algorithms: Success Rate, Priority List, Rule-Based, or Volume Split.
        </p>
      </div>

      <div className="flex gap-2 border-b border-slate-200 dark:border-[#1c1c24]">
        <button
          onClick={() => setActiveTab('single')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'single' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-slate-500 hover:text-slate-700'}`}
        >
          Single Test
        </button>
        <button
          onClick={() => setActiveTab('batch')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'batch' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-slate-500 hover:text-slate-700'}`}
        >
          Batch Simulation
        </button>
        <button
          onClick={() => setActiveTab('rule')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'rule' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-slate-500 hover:text-slate-700'}`}
        >
          Rule-Based
        </button>
        <button
          onClick={() => setActiveTab('volume')}
          className={`px-4 py-2 text-sm font-medium ${activeTab === 'volume' ? 'text-brand-500 border-b-2 border-brand-500' : 'text-slate-500 hover:text-slate-700'}`}
        >
          Volume Split
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <h2 className="font-medium text-slate-800">
              {activeTab === 'rule' ? 'Rule Evaluation Parameters' :
                activeTab === 'volume' ? 'Volume Split Configuration' :
                  'Payment Parameters'}
            </h2>
          </CardHeader>
          <CardBody className="space-y-3">
            {!merchantId && activeTab !== 'volume' && (
              <p className="text-xs text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
                Set a merchant ID in the top bar first.
              </p>
            )}
            {activeTab !== 'volume' && routingKeysLoading && (
              <p className="text-xs text-slate-600 bg-slate-50 border border-slate-200 rounded px-3 py-2">
                Loading routing config from backend...
              </p>
            )}
            {activeTab !== 'volume' && routingConfigUnavailable && (
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
            ) : activeTab === 'volume' ? (
              <div>
                <label className="block text-xs font-medium text-slate-600 mb-1">Number of Payments</label>
                <input
                  type="text"
                  value={volumePayments}
                  onChange={e => setVolumePayments(e.target.value)}
                  className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <p className="text-xs text-slate-500 mt-1">
                  Enter the total number of payments to visualize how they would be distributed across gateways.
                </p>
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

                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="block text-xs font-medium text-slate-600 mb-1">Algorithm</label>
                    <select value={form.ranking_algorithm} onChange={e => set('ranking_algorithm', e.target.value)}
                      className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500">
                      {ALGORITHMS.map(a => <option key={a} value={a}>{ALGORITHM_LABELS[a]}</option>)}
                    </select>
                  </div>
                  <div className="flex items-end pb-1">
                    <label className="flex items-center gap-2 text-sm text-slate-700 cursor-pointer">
                      <input type="checkbox" checked={form.elimination_enabled}
                        onChange={e => set('elimination_enabled', e.target.checked)}
                        className="rounded" />
                      Elimination enabled
                    </label>
                  </div>
                </div>

                {activeTab === 'batch' && (
                  <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-4 mt-4 space-y-3">
                    <h3 className="text-sm font-medium text-slate-800 flex items-center gap-2">
                      <Activity size={14} />
                      Simulation Configuration
                    </h3>
                    <div className="grid grid-cols-3 gap-3">
                      <div>
                        <label className="block text-xs font-medium text-slate-600 mb-1">Total Payments</label>
                        <input
                          type="text"
                          value={simulationConfig.totalPayments}
                          onChange={e => setSimulationConfig(s => ({ ...s, totalPayments: e.target.value }))}
                          className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-slate-600 mb-1">Success Count</label>
                        <input
                          type="text"
                          value={simulationConfig.successCount}
                          onChange={e => setSimulationConfig(s => ({ ...s, successCount: e.target.value }))}
                          className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-medium text-slate-600 mb-1">Failure Count</label>
                        <input
                          type="text"
                          value={simulationConfig.failureCount}
                          onChange={e => setSimulationConfig(s => ({ ...s, failureCount: e.target.value }))}
                          className="w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                        />
                      </div>
                    </div>
                    <p className="text-xs text-slate-500">
                      Will run {simulationConfig.totalPayments || 0} payments: {simulationConfig.successCount || 0} SUCCESS, {simulationConfig.failureCount || 0} FAILURE
                    </p>
                  </div>
                )}
              </>
            )}

            <ErrorMessage error={error} />

            {activeTab === 'rule' ? (
              <Button onClick={runRuleEvaluation} disabled={loading || routingConfigUnavailable} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Evaluating…</> : <><Play size={14} /> Evaluate Rules</>}
              </Button>
            ) : activeTab === 'volume' ? (
              <Button onClick={runVolumeSplit} disabled={loading} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Calculating…</> : <><PieChartIcon size={14} /> Visualize Distribution</>}
              </Button>
            ) : activeTab === 'batch' ? (
              <Button onClick={runSimulation} disabled={isSimulating || !merchantId || routingConfigUnavailable} className="w-full justify-center">
                {isSimulating ? (
                  <>
                    <Spinner size={14} />
                    Simulating {simulationResults.length}/{simulationConfig.totalPayments || 0}...
                  </>
                ) : (
                  <>
                    <Activity size={14} /> Run Batch Simulation
                  </>
                )}
              </Button>
            ) : (
              <Button onClick={run} disabled={loading || !merchantId || routingConfigUnavailable} className="w-full justify-center">
                {loading ? <><Spinner size={14} /> Running…</> : <><Play size={14} /> Run Decision</>}
              </Button>
            )}
          </CardBody>
        </Card>

        <div className="space-y-4">
          {activeTab === 'volume' ? (
            volumeDistribution.length > 0 ? (
              <>
                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Volume Distribution Overview</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="text-center mb-4">
                      <p className="text-3xl font-bold text-slate-900">{volumePayments}</p>
                      <p className="text-xs text-slate-500">Total Payments</p>
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                      {volumeDistribution.map((item, idx) => (
                        <div key={idx} className="bg-slate-50 dark:bg-[#111114] rounded-lg p-3">
                          <div className="flex items-center gap-2 mb-1">
                            <div
                              className="w-3 h-3 rounded"
                              style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                            />
                            <span className="font-medium text-sm">{item.name}</span>
                          </div>
                          <div className="flex justify-between text-xs text-slate-500">
                            <span>{item.percentage}%</span>
                            <span className="font-medium text-slate-700">{item.count} payments</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Pie Chart</h3>
                  </CardHeader>
                  <CardBody>
                    <ResponsiveContainer width="100%" height={250}>
                      <PieChart>
                        <Pie
                          data={pieData}
                          cx="50%"
                          cy="50%"
                          innerRadius={60}
                          outerRadius={100}
                          paddingAngle={3}
                          dataKey="value"
                          label={({ name, percent }) => `${name} ${(percent * 100).toFixed(0)}%`}
                          labelLine={false}
                        >
                          {pieData.map((_, index) => (
                            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                          ))}
                        </Pie>
                        <Tooltip
                          formatter={(value: number) => [`${value} payments`, 'Count']}
                          contentStyle={document.documentElement.classList.contains('dark') ? { backgroundColor: '#111114', border: '1px solid #222226', borderRadius: '8px', color: '#fff' } : { backgroundColor: '#fff', border: '1px solid #e5e7eb', borderRadius: '8px', color: '#1f2937' }}
                        />
                      </PieChart>
                    </ResponsiveContainer>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Bar Chart</h3>
                  </CardHeader>
                  <CardBody>
                    <ResponsiveContainer width="100%" height={volumeDistribution.length * 50 + 40}>
                      <BarChart data={volumeDistribution} layout="vertical" margin={{ left: 20, right: 40 }}>
                        <XAxis type="number" tick={{ fontSize: 12, fill: '#666' }} axisLine={{ stroke: '#e5e7eb' }} tickLine={false} />
                        <YAxis type="category" dataKey="name" tick={{ fontSize: 12, fill: '#666' }} width={80} axisLine={false} tickLine={false} />
                        <Tooltip
                          formatter={(value: number) => [`${value} payments`, 'Count']}
                          contentStyle={document.documentElement.classList.contains('dark') ? { backgroundColor: '#111114', border: '1px solid #222226', borderRadius: '8px', color: '#fff' } : { backgroundColor: '#fff', border: '1px solid #e5e7eb', borderRadius: '8px', color: '#1f2937' }}
                        />
                        <Bar dataKey="count" radius={[0, 6, 6, 0]}>
                          {volumeDistribution.map((_, index) => (
                            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Percentage Distribution</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="h-4 rounded-full overflow-hidden flex">
                      {volumeDistribution.map((item, idx) => (
                        <div
                          key={idx}
                          style={{
                            width: `${item.percentage}%`,
                            backgroundColor: COLORS[idx % COLORS.length]
                          }}
                          className="h-full transition-all duration-300"
                          title={`${item.name}: ${item.percentage}%`}
                        />
                      ))}
                    </div>
                    <div className="flex flex-wrap gap-3 mt-3">
                      {volumeDistribution.map((item, idx) => (
                        <div key={idx} className="flex items-center gap-1.5 text-xs">
                          <div
                            className="w-2.5 h-2.5 rounded-sm"
                            style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                          />
                          <span className="text-slate-600">{item.name}</span>
                          <span className="font-medium">{item.percentage}%</span>
                        </div>
                      ))}
                    </div>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Gateway Summary</h3>
                  </CardHeader>
                  <CardBody className="p-0">
                    <table className="w-full text-sm">
                      <thead className="bg-slate-50 dark:bg-[#111114] text-xs text-slate-500">
                        <tr>
                          <th className="text-left px-4 py-2">gateway_name</th>
                          <th className="text-right px-4 py-2">Payments</th>
                          <th className="text-right px-4 py-2">Percentage</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-100 dark:divide-[#222226]">
                        {volumeDistribution.map((item, idx) => (
                          <tr key={idx} className="hover:bg-slate-50 dark:bg-[#111114]">
                            <td className="px-4 py-2">
                              <div className="flex items-center gap-2">
                                <div
                                  className="w-3 h-3 rounded"
                                  style={{ backgroundColor: COLORS[idx % COLORS.length] }}
                                />
                                <span className="font-medium">{item.name}</span>
                              </div>
                            </td>
                            <td className="px-4 py-2 text-right font-medium">{item.count}</td>
                            <td className="px-4 py-2 text-right text-slate-500">{item.percentage}%</td>
                          </tr>
                        ))}
                        <tr className="bg-slate-50 dark:bg-[#111114] font-medium">
                          <td className="px-4 py-2">Total</td>
                          <td className="px-4 py-2 text-right">{volumePayments}</td>
                          <td className="px-4 py-2 text-right">100%</td>
                        </tr>
                      </tbody>
                    </table>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <div>
                      <h3 className="text-sm font-medium text-slate-800">Payment Log</h3>
                      <p className="mt-1 text-xs text-slate-500">
                        Simulated sequence based on the configured split, shown in shuffled order instead of connector blocks.
                      </p>
                    </div>
                  </CardHeader>
                  <CardBody className="p-0 max-h-80 overflow-auto">
                    <table className="w-full text-sm">
                      <thead className="bg-slate-50 dark:bg-[#111114] text-xs text-slate-500 sticky top-0">
                        <tr>
                          <th className="text-left px-4 py-2 w-20">#</th>
                          <th className="text-left px-4 py-2">gateway_name</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-100 dark:divide-[#222226]">
                        {simulatedVolumePayments.map((entry, idx) => (
                          <tr key={`${entry.connector}-${idx}`} className="hover:bg-slate-50 dark:bg-[#111114]">
                            <td className="px-4 py-1.5 text-slate-500 font-mono text-xs">{idx + 1}</td>
                            <td className="px-4 py-1.5">
                              <div className="flex items-center gap-2">
                                <div
                                  className="w-2 h-2 rounded"
                                  style={{ backgroundColor: COLORS[entry.colorIdx % COLORS.length] }}
                                />
                                <span className="font-medium">{entry.connector}</span>
                              </div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader>
                    <button
                      onClick={() => setVolumeResponseOpen(o => !o)}
                      className="flex items-center justify-between w-full text-sm font-medium text-slate-800"
                    >
                      <span className="flex items-center gap-2">
                        <Code size={14} />
                        API Response
                      </span>
                      {volumeResponseOpen ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                    </button>
                  </CardHeader>
                  {volumeResponseOpen && ruleResult && (
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
                  <PieChartIcon size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Enter the number of payments and click "Visualize Distribution" to see how payments are split across gateways.</p>
                </CardBody>
              </Card>
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
            simulationResults.length > 0 ? (
              <>
                <Card>
                  <CardHeader>
                    <h3 className="text-sm font-medium text-slate-800">Simulation Progress</h3>
                  </CardHeader>
                  <CardBody>
                    <div className="mb-4">
                      <div className="flex justify-between text-xs text-slate-600 mb-1">
                        <span>Progress</span>
                        <span>{Math.round((simulationResults.length / (parseInt(simulationConfig.totalPayments) || 1)) * 100)}%</span>
                      </div>
                      <div className="w-full bg-gray-200 rounded-full h-2">
                        <div
                          className="bg-brand-500 h-2 rounded-full transition-all duration-300"
                          style={{ width: `${(simulationResults.length / (parseInt(simulationConfig.totalPayments) || 1)) * 100}%` }}
                        />
                      </div>
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
                        {simulationResults.map((res, idx) => (
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
                  </CardBody>
                </Card>
              </>
            ) : (
              <Card>
                <CardBody className="py-16 text-center">
                  <Activity size={32} className="text-gray-300 mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Configure simulation parameters and click "Run Batch Simulation" to test Success Rate routing.</p>
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
                      <div className="text-right space-y-1">
                        <div>
                          <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${approachColor(result.routing_approach)}`}>
                            {result.routing_approach}
                          </span>
                        </div>
                        {result.is_scheduled_outage && <Badge variant="red">Scheduled Outage</Badge>}
                        {result.latency != null && (
                          <p className="text-xs text-slate-400">{result.latency}ms</p>
                        )}
                      </div>
                    </div>
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
                  <p className="text-slate-400 text-sm">Fill in the parameters and click "Run Decision" to see the routing result.</p>
                </CardBody>
              </Card>
            )
          )}
        </div>
      </div>

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
                  Batch Simulation Audit
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
                                    {humanizeAuditValue(event.status) || eventTypeLabel(event.event_type)}
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
    </div>
  )
}
