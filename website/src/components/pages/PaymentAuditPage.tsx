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
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'

const RANGE_OPTIONS: AnalyticsRange[] = ['15m', '1h', '24h']
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
  return qs ? `/analytics/payment-audit?${qs}` : '/analytics/payment-audit'
}

function parseRange(value: string | null): AnalyticsRange {
  if (value === '15m' || value === '24h') return value
  return '24h'
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
  if (normalizedStatus === 'HIT') return 'purple'
  return 'gray'
}

function phaseBadgeVariant(phase: string): 'blue' | 'green' | 'purple' | 'red' | 'orange' | 'gray' {
  if (phase === 'Decide Gateway') return 'blue'
  if (phase === 'Rule Evaluate') return 'purple'
  if (phase === 'Update Gateway') return 'green'
  if (phase === 'Errors') return 'red'
  return 'orange'
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
  return active ? 'bg-brand-600 text-white' : 'bg-white text-slate-600 border border-slate-200 hover:bg-slate-50 dark:bg-[#121214] dark:text-[#a1a1aa] dark:border-[#27272a]'
}

function controlClassName() {
  return 'h-10 rounded-2xl border border-slate-200 bg-white px-3 text-sm text-slate-700 shadow-sm outline-none transition focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#e5e7eb]'
}

function KeyMetric({ label, value, helper }: { label: string; value: string; helper: string }) {
  return (
    <Card>
      <CardBody>
        <p className="text-xs uppercase tracking-[0.16em] text-slate-500">{label}</p>
        <p className="mt-2 text-3xl font-semibold text-slate-900 dark:text-white">{value}</p>
        <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">{helper}</p>
      </CardBody>
    </Card>
  )
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-slate-200 dark:border-[#222227] bg-white/60 dark:bg-[#0b0b0d] px-6 py-12 text-center">
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
        <div key={`${row.label}-${row.value}`} className="rounded-2xl border border-slate-200 bg-white/70 px-4 py-3 dark:border-[#1d1d23] dark:bg-[#09090b]">
          <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">{row.label}</p>
          <p className="mt-2 text-sm text-slate-900 dark:text-white break-words">{row.value}</p>
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
        <pre className="overflow-x-auto rounded-2xl bg-slate-950/90 px-4 py-4 text-xs leading-6 text-slate-200">
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

  const initialRange = parseRange(searchParams.get('range'))
  const initialFilters = parseFilters(searchParams)
  const initialPage = Math.max(1, Number(searchParams.get('page') || '1'))
  const initialSelectedKey = searchParams.get('selected') || ''

  const [range, setRange] = useState<AnalyticsRange>(initialRange)
  const [filters, setFilters] = useState<AuditFilters>(initialFilters)
  const [appliedFilters, setAppliedFilters] = useState<AuditFilters>(initialFilters)
  const [page, setPage] = useState(initialPage)
  const [selectedKey, setSelectedKey] = useState<string>(initialSelectedKey)
  const [selectedEventId, setSelectedEventId] = useState<number | null>(null)
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('summary')
  const pageSize = 12

  const canQueryCurrent = Boolean(merchantId)

  const searchUrl = canQueryCurrent && merchantId
    ? buildAuditUrl(range, merchantId, page, pageSize, appliedFilters)
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
    ? buildAuditUrl(range, merchantId, 1, 50, detailFilters)
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

  function syncSearch(nextRange: AnalyticsRange, nextPage: number, nextFilters: AuditFilters, nextSelectedKey?: string) {
    const normalizedFilters = normalizeAuditFilters(nextFilters)
    const nextQuery = queryString({
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
    const normalizedFilters = normalizeAuditFilters(filters)
    setPage(nextPage)
    setFilters(normalizedFilters)
    setAppliedFilters(normalizedFilters)
    syncSearch(range, nextPage, normalizedFilters)
  }

  function clearFilters() {
    const nextPage = 1
    setPage(nextPage)
    setFilters(EMPTY_FILTERS)
    setAppliedFilters(EMPTY_FILTERS)
    syncSearch(range, nextPage, EMPTY_FILTERS)
  }

  function refreshAll() {
    auditSearch.mutate()
    auditDetail.mutate()
  }

  function updateRange(nextRange: AnalyticsRange) {
    const nextPage = 1
    setRange(nextRange)
    setPage(nextPage)
    syncSearch(nextRange, nextPage, appliedFilters, selectedKey)
  }

  function selectSummary(lookupKey: string) {
    setSelectedKey(lookupKey)
    syncSearch(range, page, appliedFilters, lookupKey)
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
    syncSearch(range, 1, nextFilters, selectedKey)
  }

  if (!canQueryCurrent) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Decision Audit</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8a8a93]">
            Search a payment and inspect gateway decisions, gateway updates, rule evaluations, and errors in one transaction trail.
          </p>
        </div>
        <EmptyState
          title="Select a merchant to start auditing payments"
          body="Use the merchant selector in the top bar to load the decision trail for a merchant."
        />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Decision Audit</h1>
          <p className="mt-1 max-w-3xl text-sm text-slate-500 dark:text-[#8a8a93]">
            Search by payment or request, then inspect the full sequence of gateway decisions, gateway updates, rule evaluations, and errors with the exact payload captured at each step.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" variant="ghost" onClick={refreshAll}>
            Refresh
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {RANGE_OPTIONS.map((value) => (
          <Button
            key={value}
            size="sm"
            variant={range === value ? 'primary' : 'secondary'}
            onClick={() => updateRange(value)}
          >
            {value}
          </Button>
        ))}
        <Badge variant="green">{merchantId || 'Current merchant'}</Badge>
      </div>

      <Card>
        <CardHeader>
          <div>
            <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Search Decision Trail</h2>
            <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
              Use payment or request IDs when you have them. Error code, gateway, route, and status narrow operational noise quickly.
            </p>
          </div>
        </CardHeader>
        <CardBody className="space-y-4">
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <input className={controlClassName()} value={filters.paymentId} onChange={(event) => updateFilter('paymentId', event.target.value)} placeholder="Payment ID" />
            <input className={controlClassName()} value={filters.requestId} onChange={(event) => updateFilter('requestId', event.target.value)} placeholder="Request ID" />
            <input className={controlClassName()} value={filters.gateway} onChange={(event) => updateFilter('gateway', event.target.value)} placeholder="Gateway" />
            <select className={controlClassName()} value={filters.route} onChange={(event) => updateFilter('route', event.target.value)}>
              {ROUTE_OPTIONS.map((option) => (
                <option key={option.value || 'all'} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
            <input className={controlClassName()} value={filters.errorCode} onChange={(event) => updateFilter('errorCode', event.target.value)} placeholder="Error code" />
            <select className={controlClassName()} value={filters.status} onChange={(event) => updateFilter('status', event.target.value)}>
              {STATUS_OPTIONS.map((option) => (
                <option key={option.value || 'all'} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" onClick={applyFilters}>Search</Button>
            <Button size="sm" variant="secondary" onClick={clearFilters}>Clear</Button>
          </div>
        </CardBody>
      </Card>

      <ErrorMessage error={error} />

      {loading && (
        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
          <Spinner size={16} />
          Loading decision audit data…
        </div>
      )}

      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <KeyMetric label="Matching payments" value={String(auditSearch.data?.total_results || 0)} helper="Results within the selected time window" />
        <KeyMetric label="Timeline events" value={String(totalEvents)} helper="Captured for the selected payment" />
        <KeyMetric label="Active gateways" value={String(activeGateways)} helper="Distinct gateways seen on the selected payment" />
        <KeyMetric label="Latest activity" value={latestSeen} helper="Most recent event on the selected payment" />
      </section>

      <div className="grid gap-4 xl:grid-cols-[360px_minmax(0,1fr)]">
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Matching Payments</h2>
              <div className="flex items-center gap-2">
                <Button size="sm" variant="secondary" disabled={page <= 1} onClick={() => {
                  const nextPage = Math.max(1, page - 1)
                  setPage(nextPage)
                  syncSearch(range, nextPage, appliedFilters, selectedKey)
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
                    syncSearch(range, nextPage, appliedFilters, selectedKey)
                  }}
                >
                  Next
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardBody className="space-y-3">
            {auditSearch.data?.results?.length ? auditSearch.data.results.map((row) => (
              <button
                key={row.lookup_key}
                type="button"
                onClick={() => selectSummary(row.lookup_key)}
                className={`w-full rounded-2xl border p-4 text-left transition-all ${selectedSummary?.lookup_key === row.lookup_key
                  ? 'border-brand-500/50 bg-brand-500/5'
                  : 'border-slate-200 hover:border-slate-300 dark:border-[#1d1d23] dark:hover:border-[#2a2a33]'
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
                <div className="mt-3 flex flex-wrap gap-2">
                  {row.latest_stage ? <Badge variant="blue">{row.latest_stage}</Badge> : null}
                  {row.latest_gateway ? <Badge variant="green">{row.latest_gateway}</Badge> : null}
                  <Badge variant="gray">{row.event_count} events</Badge>
                </div>
                {row.request_id ? (
                  <p className="mt-3 truncate text-[11px] text-slate-500 dark:text-[#8a8a93]">
                    request {row.request_id}
                  </p>
                ) : null}
              </button>
            )) : (
              <EmptyState
                title="No matching payments found"
                body="Try widening the time range or searching by a single payment ID, request ID, or error code."
              />
            )}
          </CardBody>
        </Card>

        <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <Card className="overflow-visible">
            <CardHeader>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Selected Payment Timeline</h2>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    {selectedSummary?.payment_id || selectedSummary?.request_id || 'Choose a payment from the result list to inspect the timeline.'}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  {selectedSummary?.latest_gateway ? <Badge variant="green">{selectedSummary.latest_gateway}</Badge> : null}
                  {selectedSummary?.latest_stage ? <Badge variant="blue">{selectedSummary.latest_stage}</Badge> : null}
                  {selectedSummary?.latest_status ? (
                    <Badge variant={summaryBadgeVariant(selectedSummary.latest_status)}>
                      {humanizeAuditValue(selectedSummary.latest_status)}
                    </Badge>
                  ) : null}
                </div>
              </div>
            </CardHeader>
            <CardBody>
              {groupedTimeline.length ? (
                <div className="space-y-6">
                  {groupedTimeline.map((group) => (
                    <div key={group.phase} className="space-y-3">
                      <div className="flex items-center gap-3">
                        <Badge variant={phaseBadgeVariant(group.phase)}>{group.phase}</Badge>
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
                              className={`relative w-full rounded-[24px] border p-5 text-left shadow-sm transition ${selected
                                ? 'border-brand-500/50 bg-brand-500/5'
                                : 'border-slate-200 bg-white/70 hover:border-slate-300 dark:border-[#1d1d23] dark:bg-[#09090b] dark:hover:border-[#2a2a33]'
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
                                    {routeLabel(event.route)} · {formatDateTime(event.created_at_ms)}
                                  </p>
                                </div>
                                <div className="flex flex-wrap gap-2">
                                  <Badge variant={badgeVariantForEvent(event)}>{eventTypeLabel(event.event_type)}</Badge>
                                  {event.status ? (
                                    <Badge variant={summaryBadgeVariant(event.status)}>
                                      {humanizeAuditValue(event.status)}
                                    </Badge>
                                  ) : null}
                                  {event.gateway ? <Badge variant="green">{event.gateway}</Badge> : null}
                                </div>
                              </div>

                              <div className="mt-4 flex flex-wrap gap-2 text-xs text-slate-500 dark:text-[#8a8a93]">
                                {event.request_id ? <span>request {event.request_id}</span> : null}
                                {event.routing_approach ? <span>approach {event.routing_approach}</span> : null}
                                {event.rule_name ? <span>rule {event.rule_name}</span> : null}
                                {event.payment_method_type ? <span>PMT {event.payment_method_type}</span> : null}
                                {event.payment_method ? <span>method {event.payment_method}</span> : null}
                                {event.error_code ? <span>error {event.error_code}</span> : null}
                              </div>

                              {event.error_message ? (
                                <p className="mt-4 rounded-2xl border border-red-500/20 bg-red-500/5 px-4 py-3 text-sm text-red-300">
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
                  body="Pick a payment from the left column to see the full transaction trail."
                />
              )}
            </CardBody>
          </Card>

          <Card className="overflow-visible xl:sticky xl:top-6 xl:self-start">
            <CardHeader>
              <div className="space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Event Inspector</h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      {selectedEvent ? `${stageLabel(selectedEvent)} · ${formatDateTime(selectedEvent.created_at_ms)}` : 'Select a timeline event to inspect the captured payload.'}
                    </p>
                  </div>
                  {selectedEvent ? <Badge variant={phaseBadgeVariant(eventPhase(selectedEvent))}>{eventPhase(selectedEvent)}</Badge> : null}
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
            </CardHeader>
            <CardBody className="space-y-4">
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
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  )
}
