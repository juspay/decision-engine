import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { useSearchParams } from 'react-router-dom'
import { useMerchantStore } from '../../store/merchantStore'
import { fetcher } from '../../lib/api'
import {
  AnalyticsRange,
  PaymentAuditEvent,
  PaymentAuditResponse,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Card as GlassCard, InsetPanel, SurfaceLabel } from '../ui/Card'

const RANGE_OPTIONS: AnalyticsRange[] = ['15m', '1h', '24h', '30d', '18mo']
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
  eventType: string
  errorCode: string
}

type InspectorTab = (typeof INSPECTOR_TABS)[number]
type AuditMode = 'transactions' | 'rule_based'

const EMPTY_FILTERS: AuditFilters = {
  paymentId: '',
  requestId: '',
  gateway: '',
  route: '',
  status: '',
  eventType: '',
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
    eventType: filters.eventType,
    errorCode: filters.errorCode.trim(),
  }
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
  range: AnalyticsRange,
  merchantId: string,
  page: number,
  pageSize: number,
  filters: AuditFilters,
) {
  const normalizedFilters = normalizeAuditFilters(filters)
  const params: Record<string, string | number | undefined> = {
    scope: 'current',
    range,
    page,
    page_size: pageSize,
    merchant_id: merchantId,
    payment_id: normalizedFilters.paymentId || undefined,
    request_id: normalizedFilters.requestId || undefined,
    gateway: normalizedFilters.gateway || undefined,
    route: normalizedFilters.route || undefined,
    status: normalizedFilters.status || undefined,
    event_type: normalizedFilters.eventType || undefined,
    error_code: normalizedFilters.errorCode || undefined,
  }
  const qs = queryString(params)
  return qs ? `${path}?${qs}` : path
}

function parseRange(value: string | null): AnalyticsRange {
  if (value === '15m' || value === '1h' || value === '24h' || value === '30d' || value === '18mo') return value
  return '24h'
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
    eventType: searchParams.get('event_type') || '',
    errorCode: searchParams.get('error_code') || '',
  })
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
  if (event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_stage === 'score_updated') return 'Update Gateway'
  if (event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_stage === 'preview_evaluated' || event.event_type === 'rule_evaluation_preview') {
    return 'Preview Result'
  }
  if (event.event_type === 'error') return 'Errors'
  return humanizeAuditValue(event.event_stage || event.event_type)
}

function eventPhase(event: PaymentAuditEvent) {
  if (event.event_type === 'decision' || event.event_stage === 'gateway_decided') return 'Decide Gateway'
  if (event.event_type === 'rule_hit' || event.event_stage === 'rule_applied') return 'Rule Evaluate'
  if (event.event_type === 'rule_evaluation_preview' || event.event_stage === 'preview_evaluated') {
    return 'Rule Preview'
  }
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
  if (event.event_type === 'rule_evaluation_preview') return 'purple'
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
  const [searchParams, setSearchParams] = useSearchParams()

  const initialMode = parseAuditMode(searchParams.get('mode'))
  const initialRange = parseRange(searchParams.get('range'))
  const initialFilters = parseFilters(searchParams)
  const initialPage = Math.max(1, Number(searchParams.get('page') || '1'))
  const initialSelectedKey = searchParams.get('selected') || ''

  const [mode, setMode] = useState<AuditMode>(initialMode)
  const [range, setRange] = useState<AnalyticsRange>(initialRange)
  const [filters, setFilters] = useState<AuditFilters>(initialFilters)
  const [appliedFilters, setAppliedFilters] = useState<AuditFilters>(initialFilters)
  const [page, setPage] = useState(initialPage)
  const [selectedKey, setSelectedKey] = useState<string>(initialSelectedKey)
  const [selectedEventId, setSelectedEventId] = useState<number | null>(null)
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('summary')
  const pageSize = 12

  const canQueryCurrent = Boolean(merchantId)
  const auditPath = mode === 'rule_based' ? '/analytics/preview-trace' : '/analytics/payment-audit'

  const searchUrl = canQueryCurrent && merchantId
    ? buildAuditUrl(auditPath, range, merchantId, page, pageSize, appliedFilters)
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
      eventType: '',
      errorCode: '',
    }
  }, [selectedSummary])

  const detailUrl = canQueryCurrent && merchantId && detailFilters
    ? buildAuditUrl(auditPath, range, merchantId, 1, 50, detailFilters)
    : null

  const auditDetail = useSWR<PaymentAuditResponse>(detailUrl, fetcher, {
    refreshInterval: 12000,
    revalidateOnFocus: true,
  })

  const selectedEvent = useMemo(() => {
    const timeline = auditDetail.data?.timeline || []
    return timeline.find((event) => event.id === selectedEventId) || timeline[0] || null
  }, [auditDetail.data?.timeline, selectedEventId])

  useEffect(() => {
    if (selectedEvent?.id) {
      setSelectedEventId(selectedEvent.id)
      return
    }
    const first = auditDetail.data?.timeline?.[0]
    if (first?.id) {
      setSelectedEventId(first.id)
    }
  }, [auditDetail.data?.timeline, selectedEvent?.id])

  const groupedTimeline = useMemo(() => {
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

  const inspectorModel = useMemo(() => buildInspectorModel(selectedEvent), [selectedEvent])

  const error = auditSearch.error?.message || auditDetail.error?.message || null
  const loading = auditSearch.isLoading || auditDetail.isLoading
  const totalEvents = auditDetail.data?.timeline?.length || 0
  const activeGateways = selectedSummary?.gateways?.length || 0
  const latestSeen = selectedSummary ? formatRelative(selectedSummary.last_seen_ms) : 'No activity'
  const content = mode === 'rule_based'
    ? {
        title: 'Decision Audit',
        description: 'Inspect preview-only rule activity from /routing/evaluate without mixing it into transaction outcomes.',
        merchantPrompt: 'Use the merchant selector in the top bar to load the preview trace for a merchant.',
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
        merchantPrompt: 'Use the merchant selector in the top bar to load the decision trail for a merchant.',
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
    nextRange: AnalyticsRange,
    nextPage: number,
    nextFilters: AuditFilters,
    nextSelectedKey?: string,
  ) {
    const normalizedFilters = normalizeAuditFilters(nextFilters)
    const nextQuery = queryString({
      mode: nextMode === 'rule_based' ? nextMode : undefined,
      range: nextRange,
      page: nextPage > 1 ? nextPage : undefined,
      payment_id: normalizedFilters.paymentId || undefined,
      request_id: normalizedFilters.requestId || undefined,
      gateway: normalizedFilters.gateway || undefined,
      route: normalizedFilters.route || undefined,
      status: normalizedFilters.status || undefined,
      event_type: normalizedFilters.eventType || undefined,
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
    syncSearch(mode, range, nextPage, normalizedFilters)
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
    syncSearch(mode, range, nextPage, clearedFilters)
  }

  function refreshAll() {
    auditSearch.mutate()
    auditDetail.mutate()
  }

  function updateRange(nextRange: AnalyticsRange) {
    const nextPage = 1
    setRange(nextRange)
    setPage(nextPage)
    syncSearch(mode, nextRange, nextPage, appliedFilters, selectedKey)
  }

  function selectSummary(lookupKey: string) {
    setSelectedKey(lookupKey)
    syncSearch(mode, range, page, appliedFilters, lookupKey)
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
    syncSearch(nextMode, range, nextPage, nextFilters)
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
      eventType: '',
      errorCode: '',
    }
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    setPage(1)
    syncSearch(mode, range, 1, nextFilters, selectedKey)
  }

  if (!canQueryCurrent) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">{content.title}</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8a8a93]">
            {content.description}
          </p>
        </div>
        <EmptyState
          title={mode === 'rule_based' ? 'Select a merchant to start auditing previews' : 'Select a merchant to start auditing payments'}
          body={content.merchantPrompt}
        />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">{content.title}</h1>
          <p className="mt-1 max-w-3xl text-sm text-slate-500 dark:text-[#8a8a93]">
            {content.description}
          </p>
          <div className="mt-4 flex flex-wrap items-center gap-3">
            <Badge variant="green">{merchantId || 'Current merchant'}</Badge>
            <Button size="sm" variant="ghost" onClick={refreshAll}>
              Refresh
            </Button>
          </div>
        </div>
      </div>

      <div className="space-y-5">
        <div className="space-y-3">
          <div className="space-y-1">
            <SurfaceLabel>Audit Scope</SurfaceLabel>
            <p className="text-sm text-slate-500 dark:text-[#b2bdd1]">
              Start by choosing whether you are reviewing live transactions or rule-preview traces.
            </p>
          </div>
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
        </div>

        <div className="space-y-3">
          <div className="space-y-1">
            <SurfaceLabel>Time Window</SurfaceLabel>
            <p className="text-sm text-slate-500 dark:text-[#b2bdd1]">
              Narrow the activity window before applying detailed filters.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
              Time window
            </p>
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

      <GlassCard className="p-6">
        <div className="border-b border-slate-200 pb-5 dark:border-[#2a303a]">
          <SurfaceLabel>Filters</SurfaceLabel>
          <h2 className="mt-3 text-lg font-semibold text-slate-900 dark:text-white">
            {content.searchTitle}
          </h2>
          <p className="mt-2 max-w-3xl text-sm text-slate-500 dark:text-[#b2bdd1]">
            {content.searchDescription}
          </p>
        </div>
        <div className="space-y-5 pt-5">
          <div className={`grid gap-3 md:grid-cols-2 ${mode === 'rule_based' ? 'xl:grid-cols-3' : 'xl:grid-cols-4'}`}>
            <FilterField label="Payment ID">
              <input className={controlClassName()} value={filters.paymentId} onChange={(event) => updateFilter('paymentId', event.target.value)} placeholder="Payment ID" />
            </FilterField>
            <FilterField label="Request ID">
              <input className={controlClassName()} value={filters.requestId} onChange={(event) => updateFilter('requestId', event.target.value)} placeholder="Request ID" />
            </FilterField>
            <FilterField label="Gateway">
              <input className={controlClassName()} value={filters.gateway} onChange={(event) => updateFilter('gateway', event.target.value)} placeholder="Gateway" />
            </FilterField>
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
              <input className={controlClassName()} value={filters.errorCode} onChange={(event) => updateFilter('errorCode', event.target.value)} placeholder="Error code" />
            </FilterField>
            <FilterField label="Status">
              <select className={controlClassName()} value={filters.status} onChange={(event) => updateFilter('status', event.target.value)}>
                {STATUS_OPTIONS.map((option) => (
                  <option key={option.value || 'all'} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </FilterField>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              size="md"
              onClick={applyFilters}
              className="min-w-[124px] shadow-[0_18px_35px_-24px_rgba(37,99,235,0.55)]"
            >
              Search
            </Button>
            <Button
              size="md"
              variant="secondary"
              onClick={clearFilters}
              className="min-w-[108px] border-slate-300 dark:border-[#384152]"
            >
              Clear
            </Button>
          </div>
        </div>
      </GlassCard>

      <ErrorMessage error={error} />

      {loading && (
        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
          <Spinner size={16} />
          Loading decision audit data…
        </div>
      )}

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <KeyMetric label={content.matchingLabel} value={String(auditSearch.data?.total_results || 0)} helper="Results within the selected time window" />
        <KeyMetric label="Timeline events" value={String(totalEvents)} helper={mode === 'rule_based' ? 'Captured for the selected preview' : 'Captured for the selected payment'} />
        <KeyMetric label="Active gateways" value={String(activeGateways)} helper={mode === 'rule_based' ? 'Distinct gateways seen on the selected preview' : 'Distinct gateways seen on the selected payment'} />
        <KeyMetric label="Latest activity" value={latestSeen} helper={mode === 'rule_based' ? 'Most recent event on the selected preview' : 'Most recent event on the selected payment'} />
      </section>

      <div className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
        <GlassCard className="p-6">
          <div className="flex items-center justify-between gap-3 border-b border-slate-200 pb-5 dark:border-[#2a303a]">
            <div>
              <SurfaceLabel>{content.matchingLabel}</SurfaceLabel>
              <p className="mt-3 text-sm text-slate-500 dark:text-[#b2bdd1]">
                {content.matchingDescription}
              </p>
            </div>
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <Button size="sm" variant="secondary" disabled={page <= 1} onClick={() => {
                  const nextPage = Math.max(1, page - 1)
                  setPage(nextPage)
                  syncSearch(mode, range, nextPage, appliedFilters, selectedKey)
                }}>
                  Prev
                </Button>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={(auditSearch.data?.results?.length || 0) < pageSize}
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
          </div>
          <div className="space-y-3 pt-5">
            {auditSearch.data?.results?.length ? auditSearch.data.results.map((row) => (
              <button
                key={row.lookup_key}
                type="button"
                onClick={() => selectSummary(row.lookup_key)}
                className={`w-full rounded-[24px] border p-4 text-left transition-all ${selectedSummary?.lookup_key === row.lookup_key
                  ? 'border-slate-200 bg-slate-50 shadow-[0_14px_30px_-28px_rgba(15,23,42,0.25)] dark:border-[#2a303a] dark:bg-[#161b24] dark:shadow-none'
                  : 'border-transparent bg-transparent hover:border-slate-200 hover:bg-slate-50/80 dark:hover:border-[#2a303a] dark:hover:bg-[#161b24]/75'
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
                <p className="mt-3 text-xs text-slate-500 dark:text-[#9dabc0]">
                  {compactMeta([
                    row.latest_stage ? humanizeAuditValue(row.latest_stage) : null,
                    row.latest_gateway ? `gateway ${row.latest_gateway}` : null,
                    `${row.event_count} events`,
                  ])}
                </p>
                {row.request_id ? (
                  <p className="mt-3 truncate text-[11px] text-slate-500 dark:text-[#8a8a93]">
                    request {row.request_id}
                  </p>
                ) : null}
              </button>
            )) : (
              <EmptyState
                title={content.noMatchesTitle}
                body={content.noMatchesBody}
              />
            )}
          </div>
        </GlassCard>

        <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <GlassCard className="overflow-visible p-6">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 pb-5 dark:border-[#2a303a]">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <SurfaceLabel>{content.summaryLabel}</SurfaceLabel>
                  <p className="mt-3 text-sm text-slate-500 dark:text-[#b2bdd1]">
                    {selectedSummary?.payment_id || selectedSummary?.request_id || (mode === 'rule_based' ? 'Choose a preview from the result list to inspect the timeline.' : 'Choose a payment from the result list to inspect the timeline.')}
                  </p>
                </div>
                <div className="flex flex-wrap items-center gap-3">
                  <p className="text-xs text-slate-500 dark:text-[#9dabc0]">
                    {compactMeta([
                      selectedSummary?.latest_stage ? humanizeAuditValue(selectedSummary.latest_stage) : null,
                      selectedSummary?.latest_gateway ? `gateway ${selectedSummary.latest_gateway}` : null,
                    ])}
                  </p>
                  {selectedSummary?.latest_status ? (
                    <Badge variant={summaryBadgeVariant(selectedSummary.latest_status)}>
                      {humanizeAuditValue(selectedSummary.latest_status)}
                    </Badge>
                  ) : null}
                </div>
              </div>
            </div>
            <div className="pt-5">
              {groupedTimeline.length ? (
                <div className="space-y-6">
                  {groupedTimeline.map((group) => (
                    <div key={group.phase} className="space-y-3">
                      <div className="flex items-center gap-3">
                        <p className="text-sm font-semibold text-slate-900 dark:text-white">{group.phase}</p>
                        <p className="text-xs text-slate-500 dark:text-[#8a8a93]">{group.events.length} event{group.events.length === 1 ? '' : 's'}</p>
                      </div>

                      <div className="relative space-y-3 pl-6 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-px before:bg-slate-200 dark:before:bg-[#23232a]">
                        {group.events.map((event) => {
                          const selected = selectedEvent?.id === event.id
                          return (
                            <button
                              key={event.id}
                              type="button"
                              onClick={() => {
                                setSelectedEventId(event.id)
                                setInspectorTab('summary')
                              }}
                              className={`relative w-full rounded-[24px] border p-5 text-left transition ${selected
                                ? 'border-slate-200 bg-slate-50 shadow-[0_16px_30px_-28px_rgba(15,23,42,0.28)] dark:border-[#2a303a] dark:bg-[#161b24] dark:shadow-none'
                                : 'border-slate-200/70 bg-white/70 hover:border-slate-300 hover:bg-white dark:border-[#2a303a]/70 dark:bg-[#131923] dark:hover:border-[#2a303a] dark:hover:bg-[#161b24]'
                              }`}
                            >
                              <span className={`absolute -left-[25px] top-6 h-3 w-3 rounded-full ${badgeVariantForEvent(event) === 'red'
                                ? 'bg-red-400'
                                : badgeVariantForEvent(event) === 'green'
                                  ? 'bg-emerald-400'
                                  : badgeVariantForEvent(event) === 'purple'
                                    ? 'bg-purple-400'
                                    : badgeVariantForEvent(event) === 'orange'
                                      ? 'bg-orange-400'
                                      : 'bg-blue-400'
                              }`} />
                              <div className="flex flex-wrap items-start justify-between gap-3">
                                <div>
                                  <p className="text-sm font-semibold text-slate-900 dark:text-white">{stageLabel(event)}</p>
                                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                                    {compactMeta([
                                      routeLabel(event.route),
                                      formatDateTime(event.created_at_ms),
                                      event.gateway ? `gateway ${event.gateway}` : null,
                                    ])}
                                  </p>
                                </div>
                                <div className="flex flex-wrap gap-2">
                                  {event.status ? (
                                    <Badge variant={summaryBadgeVariant(event.status)}>
                                      {humanizeAuditValue(event.status)}
                                    </Badge>
                                  ) : null}
                                </div>
                              </div>

                              <p className="mt-4 text-xs text-slate-500 dark:text-[#8a8a93]">
                                {compactMeta([
                                  event.request_id ? `request ${event.request_id}` : null,
                                  event.routing_approach ? `approach ${event.routing_approach}` : null,
                                  event.rule_name ? `rule ${event.rule_name}` : null,
                                  event.payment_method_type || event.payment_method
                                    ? compactMeta([
                                        event.payment_method_type ? `PMT ${event.payment_method_type}` : null,
                                        event.payment_method ? `method ${event.payment_method}` : null,
                                      ])
                                    : null,
                                  event.error_code ? `error ${event.error_code}` : null,
                                ])}
                              </p>

                              {event.error_message ? (
                                <p className="mt-4 rounded-2xl border border-red-500/20 bg-red-500/[0.08] px-4 py-3 text-sm text-red-600 dark:text-red-300">
                                  {event.error_message}
                                </p>
                              ) : null}
                            </button>
                          )
                        })}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <EmptyState
                  title="No timeline selected yet"
                  body={content.summaryEmpty}
                />
              )}
            </div>
          </GlassCard>

          <GlassCard className="overflow-visible p-6 xl:sticky xl:top-6 xl:self-start">
            <div className="space-y-4 border-b border-slate-200 pb-5 dark:border-[#2a303a]">
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <SurfaceLabel>Event Inspector</SurfaceLabel>
                      <p className="mt-3 text-sm text-slate-500 dark:text-[#b2bdd1]">
                        {selectedEvent ? `${stageLabel(selectedEvent)} · ${formatDateTime(selectedEvent.created_at_ms)}` : 'Select a timeline event to inspect the captured payload.'}
                      </p>
                    </div>
                    {selectedEvent ? (
                      <p className="text-xs text-slate-500 dark:text-[#9dabc0]">{eventPhase(selectedEvent)}</p>
                    ) : null}
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
              </div>
            </div>
            <div className="space-y-4 pt-5">
              {selectedEvent && inspectorModel ? (
                <>
                  <div className="flex flex-wrap gap-2">
                    <Button size="sm" variant="secondary" onClick={() => setInspectorTab('raw')}>
                      View payload
                    </Button>
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

                  {inspectorTab === 'summary' ? (
                    <div className="space-y-4">
                      <InspectorKeyValueGrid rows={inspectorModel.summaryRows} />
                      <InspectorJsonPanel
                        title="Connector score context"
                        value={inspectorModel.scoreContext}
                        emptyMessage="No connector score map was captured for this event."
                      />
                      <InspectorJsonPanel
                        title="Selection reason"
                        value={inspectorModel.selectionReason}
                        emptyMessage="No explicit selection reason was captured for this event."
                      />
                      <InspectorJsonPanel
                        title="Score / rule details"
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
                </>
              ) : (
                <EmptyState
                  title="No event selected"
                  body="Pick a timeline step to see the request, response, transaction context, and raw payload."
                />
              )}
            </div>
          </GlassCard>
        </div>
      </div>
    </div>
  )
}
