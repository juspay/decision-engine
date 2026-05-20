import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { useSearchParams } from 'react-router-dom'
import { ArrowLeft, SlidersHorizontal } from 'lucide-react'
import { fetcher } from '../../lib/api'
import {
  AnalyticsRange,
  AnalyticsRangeValue,
  PaymentAuditEvent,
  PaymentAuditResponse,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Card as GlassCard, InsetPanel, SurfaceLabel } from '../ui/Card'
import { CopyButton } from '../ui/CopyButton'
import { DateTimePicker } from '../ui/DateTimePicker'

const RANGE_OPTIONS: AnalyticsRangeValue[] = ['15m', '1h', '12h', '1d', '1w', 'custom']
const STATUS_OPTIONS = [
  { value: '', label: 'Any status' },
  { value: 'success', label: 'Success' },
  { value: 'failure', label: 'Failure' },
]
const ROUTE_OPTIONS = [
  { value: '', label: 'Any route' },
  { value: 'decide_gateway', label: 'Decide Gateway' },
  { value: 'update_gateway_score', label: 'Update Gateway' },
  { value: 'routing_evaluate', label: 'Rule Evaluate' },
]
const INSPECTOR_TABS = ['summary', 'input', 'response', 'raw'] as const
const DEBIT_ROUTING_APPROACH = 'NTW_BASED_ROUTING'


type AuditFilters = {
  paymentId: string
  requestId: string
  gateway: string
  route: string
  status: string
  flowType: string
  errorCode: string
}

type InspectorTab = (typeof INSPECTOR_TABS)[number]
type AuditMode = 'transactions' | 'rule_based' | 'debit_routing'
const AUDIT_MODE_LABELS: Record<AuditMode, string> = {
  transactions: 'Auth-rate based',
  rule_based: 'Rule based / Volume based',
  debit_routing: 'Debit routing',
}

type TimeWindow = {
  start_ms: number
  end_ms: number
}

const EMPTY_FILTERS: AuditFilters = {
  paymentId: '',
  requestId: '',
  gateway: '',
  route: '',
  status: '',
  flowType: '',
  errorCode: '',
}

function normalizeAuditFilters(filters: AuditFilters): AuditFilters {
  const lookupValue = filters.paymentId.trim() || filters.requestId.trim()
  const requestId = looksLikeRequestIdentifier(lookupValue) ? lookupValue : ''
  const paymentId = requestId ? '' : lookupValue
  return {
    paymentId,
    requestId,
    gateway: filters.gateway.trim(),
    route: filters.route,
    status: filters.status,
    flowType: filters.flowType.trim(),
    errorCode: filters.errorCode.trim(),
  }
}

function looksLikeRequestIdentifier(value: string) {
  return (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value) ||
    /^req[_-]/i.test(value)
  )
}

function flowTypeValue(event: PaymentAuditEvent) {
  return event.flow_type || ''
}

function isErrorFlow(flowType: string) {
  return flowType.endsWith('_error')
}

function isPreviewFlow(flowType: string) {
  return flowType.startsWith('routing_evaluate_') && flowType !== 'routing_evaluate_request_hit'
}

function isRuleHitFlow(flowType: string) {
  return flowType === 'decide_gateway_rule_hit'
}

function isUpdateFlow(flowType: string) {
  return flowType.startsWith('update_gateway_score_') || flowType.startsWith('update_score_legacy_')
}

function isDecisionFlow(flowType: string) {
  return flowType.startsWith('decide_gateway_') && !isRuleHitFlow(flowType)
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

function buildAuditUrl(
  path: '/analytics/payment-audit' | '/analytics/preview-trace',
  range: AnalyticsRangeValue,
  page: number,
  pageSize: number,
  filters: AuditFilters,
  customWindow?: TimeWindow,
  routingApproach?: string,
  excludedRoutingApproach?: string,
) {
  const normalizedFilters = normalizeAuditFilters(filters)
  const params: Record<string, string | number | undefined> = {
    range: range === 'custom' ? '1h' : range,
    page,
    page_size: pageSize,
    start_ms: customWindow?.start_ms,
    end_ms: customWindow?.end_ms,
    payment_id: normalizedFilters.paymentId || undefined,
    request_id: normalizedFilters.requestId || undefined,
    gateway: normalizedFilters.gateway || undefined,
    route: normalizedFilters.route || undefined,
    status: normalizedFilters.status || undefined,
    flow_type: normalizedFilters.flowType || undefined,
    routing_approach: routingApproach,
    exclude_routing_approach: excludedRoutingApproach,
    error_code: normalizedFilters.errorCode || undefined,
  }
  const qs = queryString(params)
  return qs ? `${path}?${qs}` : path
}

function parseRange(value: string | null): AnalyticsRangeValue {
  if (value === 'custom') return value
  if (value === '15m' || value === '1h' || value === '12h' || value === '1d' || value === '1w') return value
  return '1d'
}

function parseAuditMode(value: string | null): AuditMode {
  if (value === 'debit_routing') return 'debit_routing'
  return value === 'rule_based' ? 'rule_based' : 'transactions'
}

function routingApproachForMode(mode: AuditMode): string | undefined {
  return mode === 'debit_routing' ? DEBIT_ROUTING_APPROACH : undefined
}

function excludedRoutingApproachForMode(mode: AuditMode): string | undefined {
  return mode === 'transactions' ? DEBIT_ROUTING_APPROACH : undefined
}

function parseFilters(searchParams: URLSearchParams): AuditFilters {
  return normalizeAuditFilters({
    paymentId: searchParams.get('payment_id') || searchParams.get('request_id') || '',
    requestId: '',
    gateway: searchParams.get('gateway') || '',
    route: searchParams.get('route') || '',
    status: searchParams.get('status') || '',
    flowType: searchParams.get('flow_type') || searchParams.get('event_type') || '',
    errorCode: searchParams.get('error_code') || '',
  })
}

function presetWindow(range: AnalyticsRange) {
  const now = Date.now()
  const duration =
    range === '15m'
      ? 15 * 60 * 1000
      : range === '1h'
        ? 60 * 60 * 1000
        : range === '12h'
          ? 12 * 60 * 60 * 1000
          : range === '1d'
            ? 24 * 60 * 60 * 1000
            : 7 * 24 * 60 * 60 * 1000

  return {
    start_ms: now - duration,
    end_ms: now,
  }
}

function toDateTimeInputValue(timestampMs: number) {
  const date = new Date(timestampMs)
  const pad = (value: number) => value.toString().padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

function fromDateTimeInputValue(value: string) {
  const timestamp = new Date(value).getTime()
  return Number.isFinite(timestamp) ? timestamp : null
}

function formatDateTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(ms))
}

function formatRelative(ms: number) {
  const diffMinutes = Math.max(0, Math.round((Date.now() - ms) / 60000))
  if (diffMinutes < 1) return 'just now'
  if (diffMinutes < 60) return `${diffMinutes}m ago`
  const diffHours = Math.round(diffMinutes / 60)
  if (diffHours < 24) return `${diffHours}h ago`
  const diffDays = Math.round(diffHours / 24)
  return `${diffDays}d ago`
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

function compactMeta(parts: Array<string | null | undefined | false>) {
  return parts.filter(Boolean).join(' · ')
}

function routeLabel(route?: string | null) {
  if (!route) return 'Unknown route'
  if (route === 'decision_gateway' || route === 'decide_gateway') return 'Decide Gateway'
  if (route === 'update_gateway_score') return 'Update Gateway'
  if (route === 'routing_evaluate') return 'Rule Evaluate'
  return humanizeAuditValue(route)
}

function stageLabel(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if (event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_stage === 'score_updated') return 'Update Gateway'
  if (event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_stage === 'preview_evaluated' || isPreviewFlow(flowType)) {
    return 'Decision Result'
  }
  if (isErrorFlow(flowType)) return 'Errors'
  return humanizeAuditValue(event.event_stage || flowType)
}

function eventPhase(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if (isDecisionFlow(flowType) || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (isRuleHitFlow(flowType) || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (isPreviewFlow(flowType) || event.event_stage === 'preview_evaluated') {
    return 'Rule Decision'
  }
  if (isUpdateFlow(flowType) || event.event_stage === 'score_updated') return 'Update Gateway'
  return 'Errors'
}

function isDecideGatewayEvent(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  return isDecisionFlow(flowType) || event.event_stage === 'gateway_decided'
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
  if (normalizedStatus === 'HIT') return 'purple'
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

function sectionButtonClass(active: boolean) {
  return active
    ? '!border-brand-500/70 !bg-white !text-slate-950 shadow-[0_14px_30px_-24px_rgba(59,130,246,0.55)] ring-2 ring-brand-500/55 dark:!border-brand-500/70 dark:!bg-[#161b24] dark:!text-white dark:ring-brand-500/55'
    : '!border-transparent !bg-slate-100 !text-slate-600 hover:!bg-slate-200 hover:!text-slate-900 dark:!bg-[#161b24] dark:!text-[#a7b2c6] dark:hover:!bg-[#1c2330] dark:hover:!text-white'
}

function controlClassName() {
  return 'h-8 rounded-xl border border-slate-200 bg-white/90 pl-3 pr-3 text-xs text-slate-700 outline-none transition focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7]'
}

function selectClassName() {
  return `${controlClassName()} appearance-none pr-7 bg-[url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E")] bg-no-repeat bg-[right_8px_center]`
}

function FilterField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center gap-3">
      <span className="shrink-0 text-xs text-slate-400 dark:text-[#555f6e]">
        {label}
      </span>
      {children}
    </label>
  )
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <InsetPanel className="border-dashed border-slate-200 bg-slate-50/70 px-6 py-12 text-center dark:border-[#2a303a] dark:bg-[#161b24]/80">
      <p className="text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#b2bdd1]">{body}</p>
    </InsetPanel>
  )
}

function InspectorKeyValueGrid({ rows }: { rows: Array<{ label: string; value: string; copyText?: string }> }) {
  if (!rows.length) return null

  return (
    <div className="grid grid-cols-2 gap-x-6 gap-y-3">
      {rows.map((row) => (
        <div key={`${row.label}-${row.value}`}>
          <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#555f6e]">{row.label}</p>
          <div className="mt-0.5 flex items-center gap-1.5">
            <p className="text-sm text-slate-900 dark:text-white break-all">{row.value}</p>
            {row.copyText && <CopyButton text={row.copyText} size={12} />}
          </div>
        </div>
      ))}
    </div>
  )
}

function InspectorJsonPanel({ title, value, emptyMessage }: { title: string; value: unknown; emptyMessage: string }) {
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
        <EmptyState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />
      )}
    </div>
  )
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
    ...(event.payment_id ? [{ label: 'Payment ID', value: event.payment_id, copyText: event.payment_id }] : []),
    ...(event.request_id ? [{ label: 'Request ID', value: event.request_id, copyText: event.request_id }] : []),
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

export function PaymentAuditPage() {
  const [searchParams, setSearchParams] = useSearchParams()

  const initialMode = searchParams.get('routing_approach') === DEBIT_ROUTING_APPROACH
    ? 'debit_routing'
    : parseAuditMode(searchParams.get('mode'))
  const initialRange = searchParams.get('start_ms') && searchParams.get('end_ms')
    ? 'custom'
    : parseRange(searchParams.get('range'))
  const initialFilters = parseFilters(searchParams)
  const initialPage = Math.max(1, Number(searchParams.get('page') || '1'))
  const initialSelectedKey = searchParams.get('selected') || ''
  const initialStartMs = Number(searchParams.get('start_ms') || '0')
  const initialEndMs = Number(searchParams.get('end_ms') || '0')
  const initialCustomWindow =
    initialStartMs > 0 && initialEndMs > initialStartMs
      ? { start_ms: initialStartMs, end_ms: initialEndMs }
      : presetWindow('1d')

  const [mode, setMode] = useState<AuditMode>(initialMode)
  const [range, setRange] = useState<AnalyticsRangeValue>(initialRange)
  const [filters, setFilters] = useState<AuditFilters>(initialFilters)
  const [appliedFilters, setAppliedFilters] = useState<AuditFilters>(initialFilters)
  const [page, setPage] = useState(initialPage)
  const [selectedKey, setSelectedKey] = useState<string>(initialSelectedKey)
  const [trailFocused, setTrailFocused] = useState(Boolean(initialSelectedKey))
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null)
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('summary')
  const [showAdvancedFilters, setShowAdvancedFilters] = useState(false)
  const [customStart, setCustomStart] = useState(() =>
    toDateTimeInputValue(initialCustomWindow.start_ms),
  )
  const [customEnd, setCustomEnd] = useState(() =>
    toDateTimeInputValue(initialCustomWindow.end_ms),
  )
  const pageSize = 12

  const customWindow = useMemo(() => {
    if (range !== 'custom') return undefined
    const start_ms = fromDateTimeInputValue(customStart)
    const end_ms = fromDateTimeInputValue(customEnd)
    const now = Date.now()
    if (start_ms === null || end_ms === null || end_ms <= start_ms || start_ms > now || end_ms > now) return undefined
    return { start_ms, end_ms }
  }, [customEnd, customStart, range])

  const auditPath = mode === 'rule_based' ? '/analytics/preview-trace' : '/analytics/payment-audit'
  const modeRoutingApproach = routingApproachForMode(mode)
  const modeExcludedRoutingApproach = excludedRoutingApproachForMode(mode)

  const searchUrl =
    range !== 'custom' || customWindow
      ? buildAuditUrl(
          auditPath,
          range,
          page,
          pageSize,
          appliedFilters,
          customWindow,
          modeRoutingApproach,
          modeExcludedRoutingApproach,
        )
      : null

  const auditSearch = useSWR<PaymentAuditResponse>(searchUrl, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 5000,
  })

  const selectedSummary = useMemo(() => {
    const rows = auditSearch.data?.results || []
    return rows.find((row) => row.lookup_key === selectedKey) || rows[0] || null
  }, [auditSearch.data?.results, selectedKey])

  useEffect(() => {
    if (selectedSummary?.lookup_key) {
      setSelectedKey(selectedSummary.lookup_key)
      return
    }
    const first = auditSearch.data?.results?.[0]
    if (first?.lookup_key) {
      setSelectedKey(first.lookup_key)
    }
  }, [auditSearch.data?.results, selectedSummary?.lookup_key])

  const detailFilters = useMemo<AuditFilters | null>(() => {
    if (!selectedSummary) return null
    const lookupValue = selectedSummary.payment_id || selectedSummary.request_id || ''
    return {
      paymentId: lookupValue,
      requestId: '',
      gateway: '',
      route: '',
      status: '',
      flowType: '',
      errorCode: '',
    }
  }, [selectedSummary])

  const detailUrl = detailFilters
    ? buildAuditUrl(
        auditPath,
        range,
        1,
        50,
        detailFilters,
        customWindow,
        modeRoutingApproach,
        modeExcludedRoutingApproach,
      )
    : null

  const auditDetail = useSWR<PaymentAuditResponse>(detailUrl, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 5000,
  })


  const timeline = auditDetail.data?.timeline || []

  const selectedEvent = useMemo(() => {
    return timeline.find((event) => event.id === selectedEventId) || timeline[0] || null
  }, [selectedEventId, timeline])

  useEffect(() => {
    if (selectedEvent?.id) {
      setSelectedEventId(selectedEvent.id)
      return
    }
    const first = timeline[0]
    if (first?.id) {
      setSelectedEventId(first.id)
    }
  }, [selectedEvent?.id, timeline])

  const inspectorModel = useMemo(() => buildInspectorModel(selectedEvent), [selectedEvent])
  const selectedEventIsDecision = selectedEvent ? isDecideGatewayEvent(selectedEvent) : false

  const error = auditSearch.error?.message || auditDetail.error?.message || null
  const loading = auditSearch.isLoading
  const resultRows = auditSearch.data?.results || []
  const totalMatches = auditSearch.data?.total_results || 0
  const totalEvents = timeline.length
  const successCount = auditSearch.data?.total_success ?? resultRows.filter((row) => summaryBadgeVariant(row.latest_status) === 'green').length
  const failureCount = auditSearch.data?.total_failure ?? resultRows.filter((row) => summaryBadgeVariant(row.latest_status) === 'red').length
  const activeGatewayList = Array.from(
    new Set(
      resultRows.flatMap((row) => {
        if (row.gateways?.length) return row.gateways.filter(Boolean)
        return row.latest_gateway ? [row.latest_gateway] : []
      }),
    ),
  )
  const activeGateways = activeGatewayList.length
  const content = mode === 'rule_based'
    ? {
        title: 'Decision Audit',
        description: 'Inspect rule decisions from /routing/evaluate without mixing them into auth-rate transaction outcomes.',
        merchantPrompt: 'Audit data follows your signed-in merchant account.',
        searchTitle: 'Search Rule Decision Trail',
        searchDescription: 'Use decision payment IDs or request IDs when you have them. Gateway, status, and error code help narrow rule decision activity quickly.',
        matchingLabel: 'Matches',
        matchingDescription: 'Scan the current result set and pick a decision to open its full trace.',
        summaryLabel: 'Selected Decision Timeline',
        summaryEmpty: 'Pick a decision from the left column to see the full rule evaluation trace.',
        noMatchesTitle: 'No matching decisions found',
        noMatchesBody: 'Try widening the time range or searching by a decision payment ID, request ID, or gateway.',
      }
    : mode === 'debit_routing'
      ? {
          title: 'Decision Audit',
          description: 'Search debit-routing decisions produced by /decide-gateway with NTW_BASED_ROUTING.',
          merchantPrompt: 'Audit data follows your signed-in merchant account.',
          searchTitle: 'Search Debit Routing Trail',
          searchDescription: 'Use payment or request IDs when you have them. Gateway, status, and error code help narrow debit-routing outcomes quickly.',
          matchingLabel: 'Matches',
          matchingDescription: 'Scan the current result set and pick a debit-routing payment to open its full event trail.',
          summaryLabel: 'Selected Debit Routing Timeline',
          summaryEmpty: 'Pick a debit-routing payment from the left column to see the full decision trail.',
          noMatchesTitle: 'No debit-routing decisions found',
          noMatchesBody: 'Run the Debit Routing tab in Decision Explorer, or widen the time range.',
        }
      : {
          title: 'Decision Audit',
          description: 'Search by payment or request, then inspect gateway decisions, gateway updates, rule evaluations, and errors with the exact payload captured at each step.',
          merchantPrompt: 'Audit data follows your signed-in merchant account.',
          searchTitle: 'Search Decision Trail',
          searchDescription: 'Use payment or request IDs when you have them. Error code, gateway, route, and status narrow results quickly.',
          matchingLabel: 'Matches',
          matchingDescription: 'Scan the current result set and pick a payment to open its full event trail.',
          summaryLabel: 'Selected Payment Timeline',
          summaryEmpty: 'Pick a payment from the left column to see the full transaction trail.',
          noMatchesTitle: 'No matching payments found',
          noMatchesBody: 'Try widening the time range or searching by a single payment ID, request ID, or error code.',
        }

  function syncSearch(
    nextMode: AuditMode,
    nextRange: AnalyticsRangeValue,
    nextPage: number,
    nextFilters: AuditFilters,
    nextSelectedKey?: string,
    nextCustomWindow?: TimeWindow,
  ) {
    const normalizedFilters = normalizeAuditFilters(nextFilters)
    const nextQuery = queryString({
      mode: nextMode === 'transactions' ? undefined : nextMode,
      range: nextRange,
      page: nextPage > 1 ? nextPage : undefined,
      start_ms: nextRange === 'custom' ? nextCustomWindow?.start_ms : undefined,
      end_ms: nextRange === 'custom' ? nextCustomWindow?.end_ms : undefined,
      payment_id: normalizedFilters.paymentId || undefined,
      request_id: normalizedFilters.requestId || undefined,
      gateway: normalizedFilters.gateway || undefined,
      route: normalizedFilters.route || undefined,
      status: normalizedFilters.status || undefined,
      flow_type: normalizedFilters.flowType || undefined,
      routing_approach: routingApproachForMode(nextMode),
      exclude_routing_approach: excludedRoutingApproachForMode(nextMode),
      error_code: normalizedFilters.errorCode || undefined,
      selected: nextSelectedKey || undefined,
    })
    setSearchParams(nextQuery)
  }

  function updateFilter(field: keyof AuditFilters, value: string) {
    setFilters((current) => normalizeAuditFilters({ ...current, [field]: value }))
  }

  function applyFilters() {
    const nextPage = 1
    const normalizedFilters = normalizeAuditFilters({
      ...filters,
      route: mode === 'transactions' ? filters.route : '',
    })
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    setFilters(normalizedFilters)
    setAppliedFilters(normalizedFilters)
    syncSearch(mode, range, nextPage, normalizedFilters, undefined, customWindow)
  }

  function clearFilters() {
    const nextPage = 1
    const clearedFilters = {
      ...EMPTY_FILTERS,
      route: mode === 'transactions' ? EMPTY_FILTERS.route : '',
    }
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    setFilters(clearedFilters)
    setAppliedFilters(clearedFilters)
    syncSearch(mode, range, nextPage, clearedFilters, undefined, customWindow)
  }

  function refreshAll() {
    auditSearch.mutate()
    auditDetail.mutate()
  }

  function updateRange(nextRange: AnalyticsRangeValue) {
    const nextPage = 1
    const nextCustomWindow =
      nextRange === 'custom'
        ? (() => {
            const start_ms = fromDateTimeInputValue(customStart)
            const end_ms = fromDateTimeInputValue(customEnd)
            const now = Date.now()
            return start_ms !== null && end_ms !== null && end_ms > start_ms && start_ms <= now && end_ms <= now
              ? { start_ms, end_ms }
              : undefined
          })()
        : undefined
    setRange(nextRange)
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    if (nextRange !== 'custom') {
      const preset = presetWindow(nextRange)
      setCustomStart(toDateTimeInputValue(preset.start_ms))
      setCustomEnd(toDateTimeInputValue(preset.end_ms))
    }
    syncSearch(
      mode,
      nextRange,
      nextPage,
      appliedFilters,
      selectedKey,
      nextCustomWindow,
    )
  }

  function selectSummary(lookupKey: string, eventCount?: number) {
    setSelectedKey(lookupKey)
    setSelectedEventId(null)
    // Single-event payments have nothing to choose in the trail — keep the results
    // list visible and let the right panel populate directly from the auto-selected event.
    if (eventCount !== 1) setTrailFocused(true)
    syncSearch(mode, range, page, appliedFilters, lookupKey, customWindow)
  }

  function returnToResults() {
    setTrailFocused(false)
    setSelectedEventId(null)
    syncSearch(mode, range, page, appliedFilters, undefined, customWindow)
  }

  function updateMode(nextMode: AuditMode) {
    const nextPage = 1
    const nextFilters = normalizeAuditFilters({
      ...filters,
      route: nextMode === 'transactions' ? filters.route : '',
    })

    setMode(nextMode)
    setPage(nextPage)
    setSelectedKey('')
    setSelectedEventId(null)
    setTrailFocused(false)
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    syncSearch(nextMode, range, nextPage, nextFilters, undefined, customWindow)
  }


  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">{content.title}</h1>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" variant="ghost" onClick={refreshAll}>
            Refresh
          </Button>
          <div className="flex items-center gap-1 rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#161b24]">
            {RANGE_OPTIONS.map((value) => (
              <Button
                key={value}
                size="sm"
                variant="secondary"
                className={sectionButtonClass(range === value)}
                onClick={() => updateRange(value)}
              >
                {value}
              </Button>
            ))}
          </div>
        </div>
      </div>

      {range === 'custom' ? (
        <GlassCard className="overflow-visible p-4">
          <div className="grid gap-3 md:grid-cols-2">
            <FilterField label="Start time">
              <DateTimePicker
                className="w-full"
                value={customStart}
                onChange={setCustomStart}
              />
            </FilterField>
            <FilterField label="End time">
              <DateTimePicker
                className="w-full"
                value={customEnd}
                onChange={setCustomEnd}
              />
            </FilterField>
          </div>
          {!customWindow ? (
            <p className="mt-3 text-xs text-red-500">
              Choose an end time after the start time. Future dates are not available.
            </p>
          ) : null}
        </GlassCard>
      ) : null}

      <div className="space-y-4">
        <div className="inline-flex max-w-full flex-wrap items-center gap-1 rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d]">
          <Button
            size="sm"
            variant="secondary"
            className={sectionButtonClass(mode === 'transactions')}
            onClick={() => updateMode('transactions')}
          >
            {AUDIT_MODE_LABELS.transactions}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            className={sectionButtonClass(mode === 'rule_based')}
            onClick={() => updateMode('rule_based')}
          >
            {AUDIT_MODE_LABELS.rule_based}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            className={sectionButtonClass(mode === 'debit_routing')}
            onClick={() => updateMode('debit_routing')}
          >
            {AUDIT_MODE_LABELS.debit_routing}
          </Button>
        </div>

        <form
          className="flex flex-wrap items-center gap-3"
          onSubmit={(e) => { e.preventDefault(); applyFilters() }}
        >
          <input
            className={`${controlClassName()} min-w-[320px] flex-[1.4]`}
            value={filters.paymentId || filters.requestId}
            onChange={(event) => updateFilter('paymentId', event.target.value)}
            placeholder={mode === 'rule_based' ? 'Decision payment ID' : 'Payment ID'}
          />
          <input
            className={`${controlClassName()} min-w-[180px] flex-1`}
            value={filters.gateway}
            onChange={(event) => updateFilter('gateway', event.target.value)}
            placeholder="Any gateway"
          />
          <select
            className={`${selectClassName()} min-w-[160px]`}
            value={filters.status}
            onChange={(event) => updateFilter('status', event.target.value)}
          >
            {STATUS_OPTIONS.map((option) => (
              <option key={option.value || 'all'} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <Button type="submit" size="sm" className="min-w-[72px]">
            Search
          </Button>
          <Button size="sm" variant="ghost" onClick={clearFilters}>
            Clear
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => setShowAdvancedFilters((value) => !value)}
            className={showAdvancedFilters ? 'text-brand-600 dark:text-brand-400' : ''}
          >
            <SlidersHorizontal className="h-3.5 w-3.5" />
            Filters
          </Button>
        </form>

        {showAdvancedFilters ? (
          <GlassCard className="p-4">
            <div className={`grid gap-3 md:grid-cols-2 ${mode === 'transactions' ? 'xl:grid-cols-3' : 'xl:grid-cols-2'}`}>
              {mode === 'transactions' ? (
                <FilterField label="Route">
                  <select className={selectClassName()} value={filters.route} onChange={(event) => updateFilter('route', event.target.value)}>
                    {ROUTE_OPTIONS.map((option) => (
                      <option key={option.value || 'all'} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </FilterField>
              ) : null}
              <FilterField label="Error Code">
                <input
                  className={controlClassName()}
                  value={filters.errorCode}
                  onChange={(event) => updateFilter('errorCode', event.target.value)}
                  placeholder="Error code"
                />
              </FilterField>
            </div>
          </GlassCard>
        ) : null}
      </div>

      <ErrorMessage error={error} />

      {loading && (
        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
          <Spinner size={16} />
          Loading decision audit data…
        </div>
      )}

      {auditSearch.data ? (
        <div className="flex flex-wrap items-stretch gap-2">
          <div className="flex items-center gap-1.5 rounded-[14px] border border-slate-200/80 bg-white/60 px-3.5 py-2 dark:border-[#23232a] dark:bg-[#0e1117]">
            <span className="text-base font-bold tabular-nums text-slate-900 dark:text-white">{totalMatches}</span>
            <span className="text-xs font-medium text-slate-500 dark:text-[#8a8a93]">Matches</span>
          </div>
          <div className="flex items-center gap-1.5 rounded-[14px] border border-emerald-200/60 bg-emerald-50/70 px-3.5 py-2 dark:border-emerald-500/20 dark:bg-emerald-500/[0.07]">
            <span className="text-base font-bold tabular-nums text-emerald-700 dark:text-emerald-400">{successCount}</span>
            <span className="text-xs font-medium text-emerald-600/80 dark:text-emerald-500/80">Success</span>
          </div>
          <div className="flex items-center gap-1.5 rounded-[14px] border border-red-200/60 bg-red-50/70 px-3.5 py-2 dark:border-red-500/20 dark:bg-red-500/[0.07]">
            <span className="text-base font-bold tabular-nums text-red-600 dark:text-red-400">{failureCount}</span>
            <span className="text-xs font-medium text-red-500/80 dark:text-red-400/80">Failure</span>
          </div>
          {activeGatewayList.length > 0 && (
            <div className="flex items-center gap-1.5 rounded-[14px] border border-violet-200/60 bg-violet-50/70 px-3.5 py-2 dark:border-violet-500/20 dark:bg-violet-500/[0.07]">
              <span className="text-base font-bold tabular-nums text-violet-700 dark:text-violet-400">{activeGateways}</span>
              <span className="text-xs font-medium text-violet-600/80 dark:text-violet-400/80">Connectors</span>
              <span className="text-[11px] text-violet-500/60 dark:text-violet-600/60">· {activeGatewayList.slice(0, 2).join(', ')}</span>
            </div>
          )}
        </div>
      ) : null}

      <div className="grid gap-4 xl:grid-cols-[minmax(360px,0.74fr)_minmax(0,1.26fr)] xl:h-[calc(100vh-110px)]">
        <GlassCard className="h-full overflow-hidden">
          <div className="shrink-0 border-b border-slate-200 px-5 py-3 dark:border-[#2a303a]">
            {trailFocused ? (
              <>
                <Button size="sm" variant="secondary" onClick={returnToResults} className="mb-3">
                  <ArrowLeft className="h-3.5 w-3.5" />
                  Back to results
                </Button>
                <div className="flex items-center justify-between gap-3">
                  <h2 className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                    {selectedSummary?.payment_id || selectedSummary?.request_id || selectedSummary?.lookup_key || 'Selected payment'}
                  </h2>
                  <div className="flex shrink-0 items-center gap-2">
                    <span className="text-xs text-slate-400 dark:text-[#555f6e]">{totalEvents} event{totalEvents === 1 ? '' : 's'}</span>
                    {selectedSummary?.latest_status ? (
                      <Badge variant={summaryBadgeVariant(selectedSummary.latest_status)}>
                        {humanizeAuditValue(selectedSummary.latest_status)}
                      </Badge>
                    ) : null}
                  </div>
                </div>
              </>
            ) : (
              <SurfaceLabel>{content.matchingLabel}</SurfaceLabel>
            )}
          </div>

          {!trailFocused ? (
            <>
            <div className="flex-1 overflow-y-auto p-2">
              {resultRows.length > 0 ? resultRows.map((row) => {
                const isSelected = selectedSummary?.lookup_key === row.lookup_key
                return (
                <button
                  key={row.lookup_key}
                  type="button"
                  onClick={() => selectSummary(row.lookup_key, row.event_count)}
                  className={`relative w-full rounded-lg px-3 py-2.5 text-left transition-all ${
                    isSelected
                      ? 'bg-brand-50 dark:bg-[#161b24]'
                      : 'hover:bg-slate-50/80 dark:hover:bg-[#13131a]'
                  }`}
                >
                  {isSelected && (
                    <span className="absolute inset-y-1.5 left-0 w-[3px] rounded-full bg-brand-500" />
                  )}
                  <div className="flex items-center gap-2.5">
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                        {row.payment_id || row.request_id || row.lookup_key}
                      </p>
                      <p className="mt-0.5 truncate text-xs text-slate-400 dark:text-[#555f6e]">
                        {compactMeta([row.latest_gateway || null, `${row.event_count} event${row.event_count === 1 ? '' : 's'}`])}
                      </p>
                    </div>
                    <div className="shrink-0 text-right">
                      <p className="text-[11px] text-slate-400 dark:text-[#555f6e]">{formatRelative(row.last_seen_ms)}</p>
                      <Badge variant={summaryBadgeVariant(row.latest_status)}>
                        {humanizeAuditValue(row.latest_status) || 'Unknown'}
                      </Badge>
                    </div>
                  </div>
                </button>
              )}) : (
                <EmptyState
                  title={content.noMatchesTitle}
                  body={content.noMatchesBody}
                />
              )}
            </div>
            <div className="shrink-0 flex items-center gap-2 border-t border-slate-200 px-4 py-3 dark:border-[#2a303a]">
              <Button
                size="sm"
                variant="secondary"
                disabled={page <= 1}
                onClick={() => {
                  const nextPage = Math.max(1, page - 1)
                  setPage(nextPage)
                  setTrailFocused(false)
                  syncSearch(mode, range, nextPage, appliedFilters, selectedKey)
                }}
              >
                Prev
              </Button>
              <Button
                size="sm"
                variant="secondary"
                disabled={resultRows.length < pageSize}
                onClick={() => {
                  const nextPage = page + 1
                  setPage(nextPage)
                  setTrailFocused(false)
                  syncSearch(mode, range, nextPage, appliedFilters, selectedKey)
                }}
              >
                Next
              </Button>
              <span className="ml-auto text-xs text-slate-400 dark:text-[#555f6e]">Page {page}</span>
            </div>
            </>
          ) : (
            <div className="flex-1 overflow-y-auto p-2">
              {timeline.length ? (
                <div>
                  {timeline.map((event, index) => {
                    const selected = selectedEvent?.id === event.id
                    return (
                      <button
                        key={event.id}
                        type="button"
                        onClick={() => {
                          setSelectedEventId(event.id)
                          setInspectorTab('summary')
                        }}
                        className={`relative w-full rounded-lg px-3 py-2.5 text-left transition-all ${
                          selected
                            ? 'bg-brand-50 dark:bg-[#161b24]'
                            : 'hover:bg-slate-50/80 dark:hover:bg-[#13131a]'
                        }`}
                      >
                        {selected && (
                          <span className="absolute inset-y-1.5 left-0 w-[3px] rounded-full bg-brand-500" />
                        )}
                        <div className="flex items-center gap-2.5">
                          <div className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] font-bold ${
                            selected
                              ? 'bg-brand-500/15 text-brand-500 dark:text-brand-400'
                              : 'bg-slate-100 text-slate-500 dark:bg-[#1e2330] dark:text-[#8a8a93]'
                          }`}>
                            {index + 1}
                          </div>
                          <div className="min-w-0 flex-1">
                            <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                              {stageLabel(event)}
                            </p>
                            <p className="mt-0.5 truncate text-xs text-slate-400 dark:text-[#555f6e]">
                              {compactMeta([
                                event.gateway || null,
                                event.routing_approach || null,
                                event.payment_method_type || null,
                              ])}
                            </p>
                            {event.error_message ? (
                              <p className="mt-1 truncate text-xs text-red-500 dark:text-red-400">
                                {event.error_message}
                              </p>
                            ) : null}
                          </div>
                          <div className="shrink-0 text-right">
                            <p className="text-[11px] text-slate-400 dark:text-[#555f6e]">{formatRelative(event.created_at_ms)}</p>
                            {event.status ? (
                              <Badge variant={summaryBadgeVariant(event.status)}>
                                {humanizeAuditValue(event.status)}
                              </Badge>
                            ) : null}
                          </div>
                        </div>
                      </button>
                    )
                  })}
                </div>
              ) : (
                <EmptyState
                  title="No timeline selected yet"
                  body={content.summaryEmpty}
                />
              )}
            </div>
          )}
        </GlassCard>

        <GlassCard className="h-full overflow-hidden">
          <div className="shrink-0 border-b border-slate-200 px-5 py-3 dark:border-[#2a303a]">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {selectedEvent ? stageLabel(selectedEvent) : 'No event selected'}
              </h2>
              {selectedEvent?.status ? (
                <Badge variant={summaryBadgeVariant(selectedEvent.status)}>
                  {humanizeAuditValue(selectedEvent.status)}
                </Badge>
              ) : null}
            </div>
          </div>

          <div className="flex-1 overflow-y-auto space-y-4 p-4">
            {selectedEvent && inspectorModel ? (
              <>
                <div className="grid gap-3 md:grid-cols-3">
                  <InsetPanel className="px-4 py-3">
                    <SurfaceLabel>Gateway</SurfaceLabel>
                    <p className="mt-2 text-base font-semibold text-slate-900 dark:text-[#7da6ff]">
                      {selectedEvent.gateway || 'Unknown'}
                    </p>
                  </InsetPanel>
                  <InsetPanel className="px-4 py-3">
                    <SurfaceLabel>Outcome</SurfaceLabel>
                    <p className="mt-2 text-base font-semibold text-slate-900 dark:text-[#34d399]">
                      {humanizeAuditValue(selectedEvent.status) || 'Unknown'}
                    </p>
                  </InsetPanel>
                  <InsetPanel className="px-4 py-3">
                    <SurfaceLabel>Time</SurfaceLabel>
                    <p className="mt-2 text-sm font-semibold text-slate-900 dark:text-white">
                      {formatDateTime(selectedEvent.created_at_ms)}
                    </p>
                  </InsetPanel>
                </div>

                <div className="flex flex-wrap gap-2">
                  {INSPECTOR_TABS.map((tab) => (
                    <Button
                      key={tab}
                      size="sm"
                      variant="secondary"
                      className={sectionButtonClass(inspectorTab === tab)}
                      onClick={() => setInspectorTab(tab)}
                    >
                      {tab === 'summary' ? 'Summary' : tab === 'input' ? 'Input' : tab === 'response' ? 'Response' : 'Raw JSON'}
                    </Button>
                  ))}
                </div>

                {inspectorTab === 'summary' ? (
                  <div className="space-y-4">
                    {selectedEventIsDecision ? (
                      <InspectorJsonPanel
                        title="Connector scores"
                        value={inspectorModel.scoreContext}
                        emptyMessage="No connector score map was captured for this event."
                      />
                    ) : null}
                    <InspectorKeyValueGrid rows={inspectorModel.summaryRows} />
                    <InspectorJsonPanel
                      title="Selection reason"
                      value={inspectorModel.selectionReason}
                      emptyMessage="No explicit selection reason was captured for this event."
                    />
                    {selectedEventIsDecision ? (
                      <InspectorJsonPanel
                        title="Details"
                        value={inspectorModel.signalRecord}
                        emptyMessage="This event did not capture additional scoring or rule metadata."
                      />
                    ) : null}
                  </div>
                ) : null}

                {inspectorTab === 'input' ? (
                  <InspectorJsonPanel
                    title="Input"
                    value={inspectorModel.requestPayload}
                    emptyMessage="No dedicated request payload was captured for this event."
                  />
                ) : null}

                {inspectorTab === 'response' ? (
                  <InspectorJsonPanel
                    title="Response"
                    value={inspectorModel.responsePayload}
                    emptyMessage="No dedicated response payload was captured for this event."
                  />
                ) : null}

                {inspectorTab === 'raw' ? (
                  <InspectorJsonPanel
                    title="Raw JSON"
                    value={inspectorModel.rawEvent}
                    emptyMessage="No raw payload is available for this event."
                  />
                ) : null}

              </>
            ) : (
              <EmptyState
                title="No event selected"
                body="Select a timeline event to view scores, routing details, request payload, and response payload."
              />
            )}
          </div>
        </GlassCard>
      </div>
    </div>
  )
}
