import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { useSearchParams } from 'react-router-dom'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
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
type AuditMode = 'transactions' | 'rule_based'
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
  const paymentId = filters.paymentId.trim()
  const requestId = paymentId ? '' : filters.requestId.trim()
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
  return value === 'rule_based' ? 'rule_based' : 'transactions'
}

function parseFilters(searchParams: URLSearchParams): AuditFilters {
  return normalizeAuditFilters({
    paymentId: searchParams.get('payment_id') || '',
    requestId: searchParams.get('request_id') || '',
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
    return 'Preview Result'
  }
  if (isErrorFlow(flowType)) return 'Errors'
  return humanizeAuditValue(event.event_stage || flowType)
}

function eventPhase(event: PaymentAuditEvent) {
  const flowType = flowTypeValue(event)
  if (isDecisionFlow(flowType) || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (isRuleHitFlow(flowType) || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (isPreviewFlow(flowType) || event.event_stage === 'preview_evaluated') {
    return 'Rule Preview'
  }
  if (isUpdateFlow(flowType) || event.event_stage === 'score_updated') return 'Update Gateway'
  return 'Errors'
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
    ? '!border-slate-200 !bg-white !text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.28)] dark:!border-[#2a303a] dark:!bg-[#161b24] dark:!text-white'
    : '!border-transparent !bg-slate-100 !text-slate-600 hover:!bg-slate-200 hover:!text-slate-900 dark:!bg-[#161b24] dark:!text-[#a7b2c6] dark:hover:!bg-[#1c2330] dark:hover:!text-white'
}

function controlClassName() {
  return 'h-11 rounded-2xl border border-slate-200 bg-white/90 px-4 text-sm text-slate-700 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.2)] outline-none transition focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7] dark:shadow-none'
}

function FilterField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="space-y-2">
      <span className="block text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
        {label}
      </span>
      {children}
    </label>
  )
}

function KeyMetric({ label, value, helper }: { label: string; value: string; helper: string }) {
  return (
    <GlassCard className="p-5">
      <SurfaceLabel>{label}</SurfaceLabel>
      <p className="mt-4 text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">{value}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#b2bdd1]">{helper}</p>
    </GlassCard>
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

function InspectorKeyValueGrid({ rows }: { rows: Array<{ label: string; value: string }> }) {
  if (!rows.length) return null

  return (
    <div className="grid gap-3 md:grid-cols-2">
      {rows.map((row) => (
        <InsetPanel key={`${row.label}-${row.value}`} className="px-4 py-3">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8390a7]">{row.label}</p>
          <p className="mt-2 text-sm text-slate-900 dark:text-white break-words">{row.value}</p>
        </InsetPanel>
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
        <pre className="overflow-x-auto rounded-[22px] border border-slate-200 bg-slate-950/95 px-4 py-4 text-xs leading-6 text-slate-200 shadow-[0_16px_30px_-28px_rgba(15,23,42,0.4)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
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
    { label: 'Merchant', value: event.merchant_id || 'unknown merchant' },
    ...(event.payment_id ? [{ label: 'Payment ID', value: event.payment_id }] : []),
    ...(event.request_id ? [{ label: 'Request ID', value: event.request_id }] : []),
    ...(event.gateway ? [{ label: 'Gateway', value: event.gateway }] : []),
    ...(event.status ? [{ label: 'Status', value: event.status }] : []),
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
  const { merchantId } = useMerchantStore()
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const effectiveMerchantId = merchantId || authMerchantId
  const [searchParams, setSearchParams] = useSearchParams()

  const initialMode = parseAuditMode(searchParams.get('mode'))
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
      : presetWindow('1h')

  const [mode, setMode] = useState<AuditMode>(initialMode)
  const [range, setRange] = useState<AnalyticsRangeValue>(initialRange)
  const [filters, setFilters] = useState<AuditFilters>(initialFilters)
  const [appliedFilters, setAppliedFilters] = useState<AuditFilters>(initialFilters)
  const [page, setPage] = useState(initialPage)
  const [selectedKey, setSelectedKey] = useState<string>(initialSelectedKey)
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
    if (start_ms === null || end_ms === null || end_ms <= start_ms) return undefined
    return { start_ms, end_ms }
  }, [customEnd, customStart, range])

  const auditPath = mode === 'rule_based' ? '/analytics/preview-trace' : '/analytics/payment-audit'

  const searchUrl =
    range !== 'custom' || customWindow
      ? buildAuditUrl(auditPath, range, page, pageSize, appliedFilters, customWindow)
      : null

  const auditSearch = useSWR<PaymentAuditResponse>(searchUrl, fetcher, {
    refreshInterval: 12000,
    revalidateOnFocus: true,
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
    const paymentId = selectedSummary.payment_id || ''
    return {
      paymentId,
      requestId: paymentId ? '' : (selectedSummary.request_id || ''),
      gateway: '',
      route: '',
      status: '',
      flowType: '',
      errorCode: '',
    }
  }, [selectedSummary])

  const detailUrl = detailFilters
    ? buildAuditUrl(auditPath, range, 1, 50, detailFilters, customWindow)
    : null

  const auditDetail = useSWR<PaymentAuditResponse>(detailUrl, fetcher, {
    refreshInterval: 12000,
    revalidateOnFocus: true,
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

  const error = auditSearch.error?.message || auditDetail.error?.message || null
  const loading = auditSearch.isLoading || auditDetail.isLoading
  const resultRows = auditSearch.data?.results || []
  const totalMatches = auditSearch.data?.total_results || 0
  const totalEvents = timeline.length
  const failureCount = resultRows.filter((row) => summaryBadgeVariant(row.latest_status) === 'red').length
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
        description: 'Inspect preview-only rule activity from /routing/evaluate without mixing it into transaction outcomes.',
        merchantPrompt: 'Analytics is scoped to your signed-in merchant. The top-bar merchant selector is not used for preview-trace access.',
        searchTitle: 'Search Rule Preview Trail',
        searchDescription: 'Use preview payment IDs or request IDs when you have them. Gateway, status, and error code help narrow rule-preview activity quickly.',
        matchingLabel: 'Matching previews',
        matchingDescription: 'Scan the current result set and pick a preview to open its full trace.',
        summaryLabel: 'Selected Preview Timeline',
        summaryEmpty: 'Pick a preview from the left column to see the full rule evaluation trace.',
        noMatchesTitle: 'No matching previews found',
        noMatchesBody: 'Try widening the time range or searching by a preview payment ID, request ID, or gateway.',
      }
    : {
        title: 'Decision Audit',
        description: 'Search by payment or request, then inspect gateway decisions, gateway updates, rule evaluations, and errors with the exact payload captured at each step.',
        merchantPrompt: 'Analytics is scoped to your signed-in merchant. The top-bar merchant selector is not used for payment-audit access.',
        searchTitle: 'Search Decision Trail',
        searchDescription: 'Use payment or request IDs when you have them. Error code, gateway, route, and status narrow operational noise quickly.',
        matchingLabel: 'Matching payments',
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
      mode: nextMode === 'rule_based' ? nextMode : undefined,
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
      route: mode === 'rule_based' ? '' : filters.route,
    })
    setPage(nextPage)
    setFilters(normalizedFilters)
    setAppliedFilters(normalizedFilters)
    syncSearch(mode, range, nextPage, normalizedFilters, undefined, customWindow)
  }

  function clearFilters() {
    const nextPage = 1
    const clearedFilters = {
      ...EMPTY_FILTERS,
      route: mode === 'rule_based' ? '' : EMPTY_FILTERS.route,
    }
    setPage(nextPage)
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
            return start_ms !== null && end_ms !== null && end_ms > start_ms
              ? { start_ms, end_ms }
              : undefined
          })()
        : undefined
    setRange(nextRange)
    setPage(nextPage)
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

  function selectSummary(lookupKey: string) {
    setSelectedKey(lookupKey)
    syncSearch(mode, range, page, appliedFilters, lookupKey, customWindow)
  }

  function updateMode(nextMode: AuditMode) {
    const nextPage = 1
    const nextFilters = normalizeAuditFilters({
      ...filters,
      route: nextMode === 'rule_based' ? '' : filters.route,
    })

    setMode(nextMode)
    setPage(nextPage)
    setSelectedKey('')
    setSelectedEventId(null)
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    syncSearch(nextMode, range, nextPage, nextFilters, undefined, customWindow)
  }

  async function copyValue(value: string | null | undefined) {
    if (!value) return
    try {
      await navigator.clipboard.writeText(value)
    } catch {
      // Ignore clipboard failures in unsupported contexts.
    }
  }

  function openRelatedEvents() {
    if (!selectedEvent) return
    const paymentId = selectedEvent.payment_id || ''
    const nextFilters: AuditFilters = {
      paymentId,
      requestId: paymentId ? '' : (selectedEvent.request_id || ''),
      gateway: selectedEvent.gateway || '',
      route: '',
      status: '',
      flowType: '',
      errorCode: '',
    }
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    setPage(1)
    syncSearch(mode, range, 1, nextFilters, selectedKey)
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">{content.title}</h1>
            <Badge variant="green">{auditSearch.data?.merchant_id || effectiveMerchantId || 'Signed-in merchant'}</Badge>
          </div>
          <p className="max-w-3xl text-sm text-slate-500 dark:text-[#8a8a93]">
            {mode === 'rule_based'
              ? 'Inspect preview traces, routing logic, and simulated rule outcomes for any request.'
              : 'Inspect gateway decisions, routing logic, and connector scores for any payment.'}
          </p>
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
              Choose an end time after the start time.
            </p>
          ) : null}
        </GlassCard>
      ) : null}

      <div className="space-y-4">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            size="sm"
            variant="secondary"
            className={sectionButtonClass(mode === 'transactions')}
            onClick={() => updateMode('transactions')}
          >
            Transactions
          </Button>
          <Button
            size="sm"
            variant="secondary"
            className={sectionButtonClass(mode === 'rule_based')}
            onClick={() => updateMode('rule_based')}
          >
            Rule-Based
          </Button>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <input
            className={`${controlClassName()} min-w-[280px] flex-1`}
            value={filters.paymentId}
            onChange={(event) => updateFilter('paymentId', event.target.value)}
            placeholder={mode === 'rule_based' ? 'Preview payment ID' : 'Payment ID or request ID'}
          />
          <input
            className={`${controlClassName()} min-w-[240px] flex-1`}
            value={filters.requestId}
            onChange={(event) => updateFilter('requestId', event.target.value)}
            placeholder="Request ID"
          />
          <input
            className={`${controlClassName()} min-w-[180px] flex-1`}
            value={filters.gateway}
            onChange={(event) => updateFilter('gateway', event.target.value)}
            placeholder="Any gateway"
          />
          <select
            className={`${controlClassName()} min-w-[160px]`}
            value={filters.status}
            onChange={(event) => updateFilter('status', event.target.value)}
          >
            {STATUS_OPTIONS.map((option) => (
              <option key={option.value || 'all'} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <Button size="md" onClick={applyFilters} className="min-w-[116px]">
            Search
          </Button>
          <Button size="md" variant="secondary" onClick={clearFilters} className="min-w-[98px]">
            Clear
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => setShowAdvancedFilters((value) => !value)}
            className="min-w-[116px]"
          >
            {showAdvancedFilters ? 'Less filters' : 'More filters'}
          </Button>
        </div>

        {showAdvancedFilters ? (
          <GlassCard className="p-4">
            <div className={`grid gap-3 md:grid-cols-2 ${mode === 'rule_based' ? 'xl:grid-cols-2' : 'xl:grid-cols-3'}`}>
              {mode === 'transactions' ? (
                <FilterField label="Route">
                  <select className={controlClassName()} value={filters.route} onChange={(event) => updateFilter('route', event.target.value)}>
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

      <section className="grid gap-4 xl:grid-cols-3">
        <KeyMetric label={content.matchingLabel} value={String(totalMatches)} helper="In this time window" />
        <KeyMetric label="Failures" value={String(failureCount)} helper="In current results" />
        <KeyMetric
          label="Active gateways"
          value={String(activeGateways)}
          helper={activeGatewayList.length ? activeGatewayList.slice(0, 3).join(', ') : 'No gateway activity yet'}
        />
      </section>

      <div className="grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)_340px]">
        <GlassCard className="overflow-hidden">
          <div className="border-b border-slate-200 px-4 py-4 dark:border-[#2a303a]">
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3">
                <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-slate-200 text-[11px] font-semibold text-slate-600 dark:border-[#2a303a] dark:text-[#8a8a93]">
                  1
                </div>
                <div>
                  <p className="text-sm font-semibold text-slate-900 dark:text-white">Results</p>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    Click a payment to open its decision trail
                  </p>
                </div>
              </div>
              <p className="text-xs text-slate-500 dark:text-[#8a8a93]">{totalMatches}</p>
            </div>
          </div>

          <div className="border-b border-slate-200 px-4 py-4 dark:border-[#2a303a]">
            <div className="flex items-center justify-between gap-3">
              <SurfaceLabel>{content.matchingLabel}</SurfaceLabel>
              <p className="text-xs text-slate-500 dark:text-[#8a8a93]">{resultRows.length} results</p>
            </div>
          </div>

          <div className="space-y-3 p-4">
            {resultRows.length ? resultRows.map((row) => (
              <button
                key={row.lookup_key}
                type="button"
                onClick={() => selectSummary(row.lookup_key)}
                className={`w-full rounded-[20px] border p-4 text-left transition-all ${
                  selectedSummary?.lookup_key === row.lookup_key
                    ? 'border-brand-500/70 bg-slate-50 shadow-[0_14px_30px_-28px_rgba(59,130,246,0.35)] dark:border-brand-500 dark:bg-[#161b24]'
                    : 'border-slate-200/80 bg-white/40 hover:border-slate-300 hover:bg-slate-50/80 dark:border-[#23232a] dark:bg-[#131318] dark:hover:border-[#2a303a] dark:hover:bg-[#17171d]'
                }`}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                      {row.payment_id || row.request_id || row.lookup_key}
                    </p>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      {row.merchant_id || 'unknown merchant'} · {formatDateTime(row.last_seen_ms)}
                    </p>
                  </div>
                  <Badge variant={summaryBadgeVariant(row.latest_status)}>
                    {humanizeAuditValue(row.latest_status) || 'Unknown'}
                  </Badge>
                </div>
                <p className="mt-3 text-xs text-slate-500 dark:text-[#8a8a93]">
                  {compactMeta([
                    row.latest_gateway || null,
                    `${row.event_count} events`,
                    formatRelative(row.last_seen_ms),
                  ])}
                </p>
              </button>
            )) : (
              <EmptyState
                title={content.noMatchesTitle}
                body={content.noMatchesBody}
              />
            )}

            <div className="flex items-center gap-2 pt-1">
              <Button
                size="sm"
                variant="secondary"
                disabled={page <= 1}
                onClick={() => {
                  const nextPage = Math.max(1, page - 1)
                  setPage(nextPage)
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
                  syncSearch(mode, range, nextPage, appliedFilters, selectedKey)
                }}
              >
                Next
              </Button>
            </div>
          </div>
        </GlassCard>

        <GlassCard className="overflow-hidden">
          <div className="border-b border-slate-200 px-5 py-4 dark:border-[#2a303a]">
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3">
                <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-slate-200 text-[11px] font-semibold text-slate-600 dark:border-[#2a303a] dark:text-[#8a8a93]">
                  2
                </div>
                <div>
                  <p className="text-sm font-semibold text-slate-900 dark:text-white">Decision trail</p>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    Click an event to inspect its scores and payload
                  </p>
                </div>
              </div>
              {selectedSummary?.payment_id || selectedSummary?.request_id ? (
                <p className="truncate text-xs text-slate-500 dark:text-[#8a8a93]">
                  {selectedSummary?.payment_id || selectedSummary?.request_id}
                </p>
              ) : null}
            </div>
          </div>

          <div className="space-y-4 p-5">
            {selectedSummary ? (
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-slate-900 dark:text-white">
                    {selectedSummary.payment_id || selectedSummary.request_id || selectedSummary.lookup_key}
                  </p>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    {totalEvents} event{totalEvents === 1 ? '' : 's'} in this decision trail
                  </p>
                </div>
                {selectedSummary.latest_status ? (
                  <Badge variant={summaryBadgeVariant(selectedSummary.latest_status)}>
                    {humanizeAuditValue(selectedSummary.latest_status)}
                  </Badge>
                ) : null}
              </div>
            ) : null}

            {timeline.length ? (
              <div className="space-y-3">
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
                      className={`w-full rounded-[20px] border px-4 py-4 text-left transition ${
                        selected
                          ? 'border-brand-500/70 bg-slate-50 shadow-[0_14px_30px_-28px_rgba(59,130,246,0.35)] dark:border-brand-500 dark:bg-[#161b24]'
                          : 'border-slate-200/70 bg-white/40 hover:border-slate-300 hover:bg-slate-50/80 dark:border-[#23232a] dark:bg-[#131318] dark:hover:border-[#2a303a] dark:hover:bg-[#17171d]'
                      }`}
                    >
                      <div className="flex items-start gap-4">
                        <div className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full border text-sm font-semibold ${
                          selected
                            ? 'border-brand-500/50 bg-brand-500/10 text-brand-300'
                            : 'border-slate-200 text-slate-600 dark:border-[#3a284f] dark:text-[#b38cff]'
                        }`}>
                          {index + 1}
                        </div>

                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-start justify-between gap-3">
                            <div>
                              <div className="flex flex-wrap items-center gap-2">
                                <p className="text-lg font-semibold text-slate-900 dark:text-[#7da6ff]">
                                  {stageLabel(event)}
                                </p>
                                {event.status ? (
                                  <Badge variant={summaryBadgeVariant(event.status)}>
                                    {humanizeAuditValue(event.status)}
                                  </Badge>
                                ) : null}
                              </div>
                              <p className="mt-2 text-xs text-slate-500 dark:text-[#8a8a93]">
                                {compactMeta([
                                  event.gateway ? `gateway ${event.gateway}` : null,
                                  formatDateTime(event.created_at_ms),
                                  event.routing_approach || null,
                                  event.payment_method_type || null,
                                ])}
                              </p>
                              <p className="mt-2 text-[11px] text-slate-500 dark:text-[#667085]">
                                {event.request_id || event.id}
                              </p>
                            </div>

                            {selected ? (
                              <p className="text-[11px] font-medium uppercase tracking-[0.14em] text-slate-500 dark:text-[#8a8a93]">
                                Inspecting →
                              </p>
                            ) : null}
                          </div>

                          {event.error_message ? (
                            <p className="mt-4 rounded-2xl border border-red-500/20 bg-red-500/[0.08] px-4 py-3 text-sm text-red-600 dark:text-red-300">
                              {event.error_message}
                            </p>
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
        </GlassCard>

        <GlassCard className="overflow-hidden xl:sticky xl:top-6 xl:self-start">
          <div className="border-b border-slate-200 px-5 py-4 dark:border-[#2a303a]">
            <div className="flex items-start gap-3">
              <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-slate-200 text-[11px] font-semibold text-slate-600 dark:border-[#2a303a] dark:text-[#8a8a93]">
                3
              </div>
              <div>
                <p className="text-sm font-semibold text-slate-900 dark:text-white">Event inspector</p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  Click a timeline event to inspect its data
                </p>
              </div>
            </div>
          </div>

          <div className="space-y-4 p-5">
            {selectedEvent && inspectorModel ? (
              <>
                <div className="space-y-3">
                  <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                    Connector scores, routing logic, and full payload
                  </p>
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="text-xl font-semibold text-slate-900 dark:text-[#7da6ff]">
                      {stageLabel(selectedEvent)}
                    </p>
                    {selectedEvent.status ? (
                      <Badge variant={summaryBadgeVariant(selectedEvent.status)}>
                        {humanizeAuditValue(selectedEvent.status)}
                      </Badge>
                    ) : null}
                  </div>
                  <p className="text-sm text-slate-500 dark:text-[#8a8a93]">
                    {formatDateTime(selectedEvent.created_at_ms)}
                  </p>
                </div>

                <div className="grid grid-cols-2 gap-3">
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
                    <InspectorJsonPanel
                      title="Connector scores"
                      value={inspectorModel.scoreContext}
                      emptyMessage="No connector score map was captured for this event."
                    />
                    <InspectorKeyValueGrid rows={inspectorModel.summaryRows} />
                    <InspectorJsonPanel
                      title="Selection reason"
                      value={inspectorModel.selectionReason}
                      emptyMessage="No explicit selection reason was captured for this event."
                    />
                    <InspectorJsonPanel
                      title="Details"
                      value={inspectorModel.signalRecord}
                      emptyMessage="This event did not capture additional scoring or rule metadata."
                    />
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

                <div className="flex flex-wrap gap-2">
                  <Button size="sm" variant="secondary" disabled={!selectedEvent.request_id} onClick={() => copyValue(selectedEvent.request_id)}>
                    Copy request ID
                  </Button>
                  <Button size="sm" variant="secondary" disabled={!selectedEvent.payment_id} onClick={() => copyValue(selectedEvent.payment_id)}>
                    Copy payment ID
                  </Button>
                  <Button size="sm" variant="secondary" onClick={openRelatedEvents}>
                    Open related events
                  </Button>
                </div>
              </>
            ) : (
              <EmptyState
                title="No event selected"
                body="Select a timeline event to inspect connector scores, routing logic, and full payloads."
              />
            )}
          </div>
        </GlassCard>
      </div>
    </div>
  )
}
