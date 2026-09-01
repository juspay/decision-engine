import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { useSearchParams } from 'react-router-dom'
import { ArrowLeft, RefreshCw, Search as SearchIcon, SlidersHorizontal } from 'lucide-react'
import { fetcher } from '../../lib/api'
import {
  AnalyticsGatewayScoresResponse,
  AnalyticsRangeValue,
  PaymentAuditEvent,
  PaymentAuditResponse,
  PaymentAuditSummary,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Card as GlassCard, InsetPanel } from '../ui/Card'
import { CopyButton } from '../ui/CopyButton'
import { TimeRangeFilter } from '../ui/TimeRangeFilter'
import {
  TimeWindow,
  customWindowFrom,
  fromDateTimeInputValue,
  parseRange,
  presetWindow,
  toDateTimeInputValue,
} from '../../lib/timeRange'

import { PageHeading } from '../ui/PageHeading'
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
const INSPECTOR_TAB_LABELS: Record<(typeof INSPECTOR_TABS)[number], string> = {
  summary: 'Summary',
  input: 'Request',
  response: 'Response',
  raw: 'Raw JSON',
}
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
  transactions: 'Multi-objective',
  rule_based: 'Rule based / Volume based',
  debit_routing: 'Debit routing',
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

function statusDotClass(status?: string | null) {
  const variant = summaryBadgeVariant(status)
  if (variant === 'green') return 'bg-emerald-500'
  if (variant === 'red') return 'bg-red-500'
  if (variant === 'purple') return 'bg-purple-500'
  return 'bg-slate-300 dark:bg-[#3a4150]'
}

/**
 * The connectors a payment moved through, as `stripe → adyen`, when it touched more than one.
 * A single-connector payment returns null — the row already names that connector on its own.
 */
function connectorPath(row: PaymentAuditSummary) {
  const gateways = (row.gateways || []).filter(Boolean)
  if (gateways.length < 2) return null
  const ordered = row.latest_gateway && gateways.includes(row.latest_gateway)
    ? [...gateways.filter((gateway) => gateway !== row.latest_gateway), row.latest_gateway]
    : gateways
  return ordered.join(' → ')
}

/** One dot-and-count item of the result line above the two panels. */
function SummaryStat({ tone, label, detail }: { tone: string; label: string; detail?: string }) {
  return (
    <span className="flex items-center gap-2 text-slate-500 dark:text-[#a7b2c6]" title={detail || undefined}>
      <span className={`h-1.5 w-1.5 rounded-full ${tone}`} />
      {label}
    </span>
  )
}

/** One `Label: value` fact in the inspector's headline strip. */
function HeadlineFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex min-w-0 items-baseline gap-2">
      <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">{label}:</span>
      <span className="truncate text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{value}</span>
    </div>
  )
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

function fieldClassName() {
  return 'h-11 w-full rounded-2xl border border-slate-200 bg-white px-4 text-[13px] text-slate-700 outline-none transition placeholder:text-slate-500 focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7] dark:placeholder:text-[#555f6e]'
}

function fieldSelectClassName() {
  return `${fieldClassName()} appearance-none pr-9 bg-[url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E")] bg-no-repeat bg-[right_12px_center]`
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <InsetPanel className="!rounded-2xl border-dashed border-slate-200 bg-slate-50/70 px-5 py-8 text-center dark:border-[#2a303a] dark:bg-[#161b24]/80">
      <p className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{title}</p>
      <p className="mt-2 text-[13px] text-slate-500 dark:text-[#b2bdd1] leading-[18px]">{body}</p>
    </InsetPanel>
  )
}

function InspectorKeyValueGrid({ rows }: { rows: Array<{ label: string; value: string; copyText?: string }> }) {
  if (!rows.length) return null

  return (
    <div className="grid gap-x-8 md:grid-cols-2 [&>*:last-child]:border-b-0 md:[&>*:nth-last-child(-n+2)]:border-b-0">
      {rows.map((row) => (
        <div
          key={`${row.label}-${row.value}`}
          className="flex items-center justify-between gap-3 border-b border-slate-100 py-3 dark:border-[#1b2029]"
        >
          <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">{row.label}</span>
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="truncate text-[13px] font-medium text-slate-900 dark:text-white leading-[18px]">{row.value}</span>
            {row.copyText && <CopyButton text={row.copyText} size={12} />}
          </span>
        </div>
      ))}
    </div>
  )
}

function JsonBlock({ value }: { value: unknown }) {
  return (
    <pre className="overflow-x-auto rounded-2xl border border-slate-200/80 bg-slate-50/90 px-5 py-3 font-mono text-[13px] leading-[21px] text-slate-800 shadow-[inset_0_1px_0_rgba(255,255,255,0.75),0_16px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
      {stringifyValue(value)}
    </pre>
  )
}

function PanelSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-3">
      <h3 className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{title}</h3>
      {children}
    </div>
  )
}

function InspectorJsonPanel({ title, value, emptyMessage }: { title: string; value: unknown; emptyMessage: string }) {
  return (
    <PanelSection title={title}>
      {value ? (
        <JsonBlock value={value} />
      ) : (
        <EmptyState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />
      )}
    </PanelSection>
  )
}

/** Entries of a flat record whose values are all finite numbers, else null. */
function asNumberEntries(value: unknown): Array<[string, number]> | null {
  if (!isRecord(value)) return null
  const keys = Object.keys(value)
  const entries = Object.entries(value).filter(
    (entry): entry is [string, number] => typeof entry[1] === 'number' && Number.isFinite(entry[1]),
  )
  return entries.length && entries.length === keys.length ? entries : null
}

/** Entries of a flat record whose values are all scalars (string/number/bool/null), else null. */
function asScalarEntries(value: unknown): Array<[string, string | number | boolean | null]> | null {
  if (!isRecord(value)) return null
  const entries = Object.entries(value)
  if (!entries.length) return null
  const allScalar = entries.every(
    ([, val]) => val === null || ['string', 'number', 'boolean'].includes(typeof val),
  )
  return allScalar ? (entries as Array<[string, string | number | boolean | null]>) : null
}

function formatScalar(value: string | number | boolean | null): string {
  if (value === null) return '—'
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  if (typeof value === 'number') return value.toLocaleString(undefined, { maximumFractionDigits: 6 })
  return value
}

function ConnectorScorePanel({
  title,
  value,
  emptyMessage,
  selectedGateway,
}: {
  title: string
  value: unknown
  emptyMessage: string
  selectedGateway?: string | null
}) {
  const entries = asNumberEntries(value)

  if (!entries) {
    return (
      <PanelSection title={title}>
        {value ? <JsonBlock value={value} /> : <EmptyState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />}
      </PanelSection>
    )
  }

  const sorted = [...entries].sort((left, right) => right[1] - left[1])
  const max = Math.max(...sorted.map(([, score]) => score))
  const asFraction = max <= 1
  const denom = asFraction ? 1 : max || 1
  const winner = selectedGateway && sorted.some(([gateway]) => gateway === selectedGateway)
    ? selectedGateway
    : sorted[0][0]

  return (
    <PanelSection title={title}>
      <div className="space-y-3 rounded-2xl border border-slate-200/80 bg-slate-50/90 px-5 py-3 dark:border-[#2a303a] dark:bg-[#0b1017]">
        {sorted.map(([gateway, score]) => {
          const isWinner = gateway === winner
          const percent = Number(((score / denom) * 100).toFixed(1))
          const width = Math.max(3, Math.min(100, percent))
          return (
            <div key={gateway} className="space-y-1.5">
              <div className="flex items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-2">
                  <span className="truncate text-[13px] font-medium text-slate-900 dark:text-white leading-[18px]">{gateway}</span>
                  {isWinner ? (
                    <span className="shrink-0 rounded-md bg-brand-500/10 px-1.5 py-0.5 text-[11px] font-semibold uppercase tracking-wide text-brand-600 dark:text-brand-300 leading-4">
                      Selected
                    </span>
                  ) : null}
                </div>
                <span className="shrink-0 text-[13px] font-semibold tabular-nums text-slate-700 dark:text-[#d8e1ef] leading-[18px]">
                  {asFraction ? `${(score * 100).toFixed(1)}%` : score.toLocaleString(undefined, { maximumFractionDigits: 4 })}
                </span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-slate-200/70 dark:bg-[#1e2330]">
                <div
                  className="h-full rounded-full transition-all"
                  style={{ width: `${width}%`, backgroundColor: isWinner ? '#0069ED' : '#94a3b8' }}
                />
              </div>
            </div>
          )
        })}
      </div>
    </PanelSection>
  )
}

function StructuredRecordPanel({ title, value, emptyMessage }: { title: string; value: unknown; emptyMessage: string }) {
  const entries = asScalarEntries(value)

  if (!entries) {
    return (
      <PanelSection title={title}>
        {value ? <JsonBlock value={value} /> : <EmptyState title={`No ${title.toLowerCase()} captured`} body={emptyMessage} />}
      </PanelSection>
    )
  }

  return (
    <PanelSection title={title}>
      <InspectorKeyValueGrid
        rows={entries.map(([key, val]) => ({
          label: humanizeAuditValue(key),
          value: formatScalar(val),
        }))}
      />
    </PanelSection>
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
    { label: 'Route', value: routeLabel(event.route) },
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

  const customWindow = useMemo(
    () => (range === 'custom' ? customWindowFrom(customStart, customEnd) : undefined),
    [customEnd, customStart, range],
  )

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

  // The audit results only name the connectors on the visible page, which would make the Gateway
  // dropdown unable to reach a connector that happens to fall on page 2. Gateway scores are
  // merchant-wide, so they name every connector with traffic — read over the same window the page
  // is auditing, so the options can neither omit a connector that was only active back then nor
  // offer one that saw no traffic in it.
  const gatewayCatalogUrl =
    range !== 'custom' || customWindow
      ? `/analytics/gateway-scores?${queryString({
          range: range === 'custom' ? '1h' : range,
          start_ms: customWindow?.start_ms,
          end_ms: customWindow?.end_ms,
        })}`
      : null

  const gatewayCatalog = useSWR<AnalyticsGatewayScoresResponse>(gatewayCatalogUrl, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 60_000,
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
  const hasActiveFilters = Object.values(filters).some(Boolean)
  const gatewayOptions = Array.from(
    new Set(
      [
        ...(gatewayCatalog.data?.snapshots || []).map((snapshot) => snapshot.gateway),
        ...activeGatewayList,
        // A gateway supplied by URL stays selectable even if it has gone quiet since.
        appliedFilters.gateway,
      ].filter(Boolean),
    ),
  ).sort((left, right) => left.localeCompare(right))
  const content = mode === 'rule_based'
    ? {
        title: 'Decision Audit',
        description: 'Inspect rule decisions from /routing/evaluate without mixing them into multi-objective transaction outcomes.',
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

  /** Dropdown filters apply on change — the redesigned bar has no Search button to press. */
  function applyDropdownFilter(field: 'gateway' | 'status' | 'route', value: string) {
    const nextPage = 1
    const normalizedFilters = normalizeAuditFilters({ ...filters, [field]: value })
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

  /** A custom window arrives with both ends at once, so the query and the URL update together. */
  function applyCustomWindow(nextStart: string, nextEnd: string) {
    const nextPage = 1
    setCustomStart(nextStart)
    setCustomEnd(nextEnd)
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    syncSearch(
      mode,
      'custom',
      nextPage,
      appliedFilters,
      selectedKey,
      customWindowFrom(nextStart, nextEnd),
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


  // Full-height column so the two panels fill the shell (78px top bar + main's vertical padding)
  // and scroll internally, as they did before the header moved into the page.
  return (
    <div className="flex min-h-[620px] flex-col gap-5 xl:h-[calc(100vh-140px)]">
      {/* The mock draws these controls in an app-wide bar; the shell already owns that strip, so the
          mode tabs and time range live at the top of the page's own content instead. */}
      <div className="grid grid-cols-1 items-center gap-3 xl:grid-cols-[1fr_auto_1fr]">
        <PageHeading title={content.title} />

        <div className="inline-flex max-w-full flex-wrap items-center gap-1 justify-self-start rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d] xl:justify-self-center">
          {(Object.keys(AUDIT_MODE_LABELS) as AuditMode[]).map((value) => (
            <Button
              key={value}
              size="sm"
              variant="secondary"
              className={sectionButtonClass(mode === value)}
              onClick={() => updateMode(value)}
            >
              {AUDIT_MODE_LABELS[value]}
            </Button>
          ))}
        </div>

        <div className="flex items-center gap-2 justify-self-start xl:justify-self-end">
          <TimeRangeFilter
            range={range}
            customStart={customStart}
            customEnd={customEnd}
            onRangeChange={updateRange}
            onCustomChange={applyCustomWindow}
          />
          <Button
            size="sm"
            variant="secondary"
            onClick={refreshAll}
            aria-label="Refresh"
            title="Refresh"
            className="!h-9 !px-3"
          >
            <RefreshCw className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <form
        className="flex flex-wrap items-center gap-3"
        onSubmit={(e) => { e.preventDefault(); applyFilters() }}
      >
        <div className={`relative ${showAdvancedFilters ? 'min-w-[240px] flex-1' : 'min-w-[300px] flex-[1.618]'}`}>
          <SearchIcon className="pointer-events-none absolute left-4 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500 dark:text-[#78849a]" />
          <input
            className={`${fieldClassName()} pl-11 ${hasActiveFilters ? 'pr-20' : ''}`}
            value={filters.paymentId || filters.requestId}
            onChange={(event) => updateFilter('paymentId', event.target.value)}
            placeholder={
              mode === 'rule_based'
                ? 'Search by decision payment ID or request ID…'
                : 'Search by payment ID or request ID…'
            }
            aria-label={mode === 'rule_based' ? 'Decision payment ID' : 'Payment ID'}
          />
          {hasActiveFilters ? (
            <button
              type="button"
              onClick={clearFilters}
              className="absolute right-4 top-1/2 -translate-y-1/2 text-[13px] font-medium text-brand-600 transition hover:text-brand-600 dark:text-brand-400 leading-[18px]"
            >
              Clear
            </button>
          ) : null}
        </div>

        <div className="flex min-w-[300px] flex-1 items-center gap-3">
        {showAdvancedFilters && mode === 'transactions' ? (
          <div className="flex-1">
            <select
              className={fieldSelectClassName()}
              value={filters.route}
              onChange={(event) => applyDropdownFilter('route', event.target.value)}
              aria-label="Route"
            >
              {ROUTE_OPTIONS.map((option) => (
                <option key={option.value || 'all'} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
        ) : null}
        {showAdvancedFilters ? (
          <div className="flex-1">
            <input
              className={fieldClassName()}
              value={filters.errorCode}
              onChange={(event) => updateFilter('errorCode', event.target.value)}
              placeholder="Error code"
              aria-label="Error code"
            />
          </div>
        ) : null}
        <div className="flex-1">
          <select
            className={fieldSelectClassName()}
            value={filters.gateway}
            onChange={(event) => applyDropdownFilter('gateway', event.target.value)}
            aria-label="Gateway"
          >
            <option value="">Any gateway</option>
            {gatewayOptions.map((gateway) => (
              <option key={gateway} value={gateway}>
                {gateway}
              </option>
            ))}
          </select>
        </div>

        <div className="flex-1">
          <select
            className={fieldSelectClassName()}
            value={filters.status}
            onChange={(event) => applyDropdownFilter('status', event.target.value)}
            aria-label="Status"
          >
            {STATUS_OPTIONS.map((option) => (
              <option key={option.value || 'all'} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>

        <Button
          type="button"
          variant="secondary"
          onClick={() => setShowAdvancedFilters((value) => !value)}
          aria-label="More filters"
          title="More filters"
          className={`h-11 !px-3 ${showAdvancedFilters ? '!text-brand-600 dark:!text-brand-600' : ''}`}
        >
          <SlidersHorizontal className="h-4 w-4" />
        </Button>
        </div>
        {/* The redesigned bar has no Search button; this keeps Enter submitting the text fields. */}
        <button type="submit" className="sr-only">Search</button>
      </form>

      <ErrorMessage error={error} />

      <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-[13px] leading-[18px]">
        {loading ? (
          <span className="flex items-center gap-2 text-slate-500 dark:text-[#8a8a93]">
            <Spinner size={14} />
            Loading decision audit data…
          </span>
        ) : (
          <span className="font-semibold text-slate-900 dark:text-white">
            {totalMatches.toLocaleString()} {totalMatches === 1 ? 'match' : 'matches'} found
          </span>
        )}
        {auditSearch.data ? (
          <>
            <span className="h-4 w-px bg-slate-200 dark:bg-[#2a303a]" />
            <SummaryStat tone="bg-emerald-500" label={`${successCount.toLocaleString()} successful ${successCount === 1 ? 'selection' : 'selections'}`} />
            <SummaryStat tone="bg-red-500" label={`${failureCount.toLocaleString()} ${failureCount === 1 ? 'failure' : 'failures'}`} />
            <SummaryStat
              tone="bg-brand-500"
              label={`${activeGateways} ${activeGateways === 1 ? 'connector' : 'connectors'}`}
              detail={activeGatewayList.slice(0, 3).join(', ')}
            />
          </>
        ) : null}
      </div>

      <div className="grid min-h-0 flex-1 gap-5 xl:grid-cols-[minmax(360px,1fr)_minmax(0,1.618fr)]">
        <GlassCard className="h-full overflow-hidden !rounded-2xl">
          {trailFocused ? (
            <div className="shrink-0 border-b border-slate-200 px-5 py-3 dark:border-[#2a303a]">
              <Button size="sm" variant="secondary" onClick={returnToResults} className="mb-3">
                <ArrowLeft className="h-3.5 w-3.5" />
                Back to results
              </Button>
              <div className="flex items-center justify-between gap-3">
                <h2 className="truncate text-[21px] font-semibold leading-tight text-slate-900 dark:text-white">
                  {selectedSummary?.payment_id || selectedSummary?.request_id || selectedSummary?.lookup_key || 'Selected payment'}
                </h2>
                <div className="flex shrink-0 items-center gap-2">
                  <span className="text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">{totalEvents} event{totalEvents === 1 ? '' : 's'}</span>
                  {selectedSummary?.latest_status ? (
                    <Badge variant={summaryBadgeVariant(selectedSummary.latest_status)}>
                      {humanizeAuditValue(selectedSummary.latest_status)}
                    </Badge>
                  ) : null}
                </div>
              </div>
            </div>
          ) : null}

          {!trailFocused ? (
            <>
            <div className="shrink-0 flex items-center justify-between gap-3 px-5 pb-3 pt-5">
              <h2 className="truncate text-[21px] font-semibold leading-tight text-slate-900 dark:text-white">
                {content.matchingLabel}
              </h2>
              <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                {resultRows.length} of {totalMatches.toLocaleString()}
              </span>
            </div>
            <div className="min-h-0 flex-1 divide-y divide-slate-100 overflow-y-auto border-t border-slate-100 dark:divide-[#1b2029] dark:border-[#1b2029]">
              {resultRows.length > 0 ? resultRows.map((row) => {
                const isSelected = selectedSummary?.lookup_key === row.lookup_key
                const gatewayPath = connectorPath(row)
                return (
                <button
                  key={row.lookup_key}
                  type="button"
                  onClick={() => selectSummary(row.lookup_key, row.event_count)}
                  className={`relative w-full px-5 py-3 text-left transition-colors ${
                    isSelected
                      ? 'bg-brand-50/70 dark:bg-[#161b24]'
                      : 'hover:bg-slate-50/80 dark:hover:bg-[#13131a]'
                  }`}
                >
                  {isSelected && (
                    <span className="absolute inset-y-0 left-0 w-[3px] bg-brand-500" />
                  )}
                  <div className="flex items-start justify-between gap-3">
                    <p className="min-w-0 flex-1 truncate font-mono text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">
                      {row.payment_id || row.request_id || row.lookup_key}
                    </p>
                    <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                      {formatRelative(row.last_seen_ms)}
                    </span>
                  </div>
                  <div className="mt-2 flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2">
                      <span
                        className={`h-1.5 w-1.5 shrink-0 rounded-full ${statusDotClass(row.latest_status)}`}
                        title={humanizeAuditValue(row.latest_status) || 'Unknown'}
                      />
                      {row.latest_gateway ? (
                        <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#a7b2c6] leading-[18px]">
                          {row.latest_gateway}
                        </span>
                      ) : null}
                      <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                        · {row.event_count} event{row.event_count === 1 ? '' : 's'}
                      </span>
                    </div>
                    {gatewayPath ? (
                      <span className="shrink-0 truncate rounded-md bg-orange-500/10 px-2 py-0.5 text-[13px] font-medium text-orange-600 ring-1 ring-inset ring-orange-500/20 dark:text-orange-300 leading-[18px]">
                        {gatewayPath}
                      </span>
                    ) : null}
                  </div>
                </button>
              )}) : (
                <div className="p-5">
                  <EmptyState
                    title={content.noMatchesTitle}
                    body={content.noMatchesBody}
                  />
                </div>
              )}
            </div>
            <div className="shrink-0 flex items-center gap-2 border-t border-slate-200 px-5 py-3 dark:border-[#2a303a]">
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
              <span className="ml-auto text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">Page {page}</span>
            </div>
            </>
          ) : (
            <div className="min-h-0 flex-1 space-y-1.5 overflow-y-auto p-3">
              {timeline.length ? (
                timeline.map((event, index) => {
                  const selected = selectedEvent?.id === event.id
                  return (
                    <button
                      key={event.id}
                      type="button"
                      onClick={() => {
                        setSelectedEventId(event.id)
                        setInspectorTab('summary')
                      }}
                      className={`relative w-full overflow-hidden rounded-2xl px-4 py-3 text-left transition-all ${
                        selected
                          ? 'bg-brand-50 dark:bg-[#161b24]'
                          : 'hover:bg-slate-50/80 dark:hover:bg-[#13131a]'
                      }`}
                    >
                      {selected && (
                        <span className="absolute inset-y-2.5 left-0 w-[3px] rounded-full bg-brand-500" />
                      )}
                      <div className="flex items-start gap-3">
                        <div className={`mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[13px] font-bold ${
                          selected
                            ? 'bg-brand-500/15 text-brand-500 dark:text-brand-400'
                            : 'bg-slate-100 text-slate-500 dark:bg-[#1e2330] dark:text-[#8a8a93]'
                        } leading-[18px]`}>
                          {index + 1}
                        </div>
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">
                            {stageLabel(event)}
                          </p>
                          <p className="mt-2 truncate text-[13px] text-slate-500 dark:text-[#78849a] max-w-[57ch] leading-[18px]">
                            {compactMeta([
                              event.gateway || null,
                              event.routing_approach || null,
                              event.payment_method_type || null,
                            ])}
                          </p>
                          {event.error_message ? (
                            <p className="mt-2 truncate text-[13px] text-red-600 dark:text-red-400 leading-[18px]">
                              {event.error_message}
                            </p>
                          ) : null}
                        </div>
                        <div className="shrink-0 space-y-1 text-right">
                          <p className="text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">{formatRelative(event.created_at_ms)}</p>
                          {event.status ? (
                            <Badge variant={summaryBadgeVariant(event.status)}>
                              {humanizeAuditValue(event.status)}
                            </Badge>
                          ) : null}
                        </div>
                      </div>
                    </button>
                  )
                })
              ) : (
                <EmptyState
                  title="No timeline selected yet"
                  body={content.summaryEmpty}
                />
              )}
            </div>
          )}
        </GlassCard>

        <GlassCard className="h-full overflow-hidden !rounded-2xl">
          {selectedEvent && inspectorModel ? (
            <>
              <div className="shrink-0 space-y-2 px-5 pb-0 pt-5">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="flex min-w-0 items-center gap-2">
                    <h2 className="truncate font-mono text-[21px] font-semibold leading-tight text-slate-900 dark:text-white">
                      {selectedEvent.payment_id || selectedEvent.request_id || stageLabel(selectedEvent)}
                    </h2>
                    {selectedEvent.payment_id || selectedEvent.request_id ? (
                      <CopyButton text={selectedEvent.payment_id || selectedEvent.request_id || ''} size={14} />
                    ) : null}
                  </div>
                  {selectedEvent.status ? (
                    <Badge variant={summaryBadgeVariant(selectedEvent.status)}>
                      {humanizeAuditValue(selectedEvent.status)}
                    </Badge>
                  ) : null}
                </div>

                {/* Outcome is not repeated here — the badge above it already carries the status. */}
                <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
                  <HeadlineFact label="Gateway" value={selectedEvent.gateway || 'Unknown'} />
                  <HeadlineFact label="Stage" value={stageLabel(selectedEvent)} />
                  <HeadlineFact label="Time" value={formatDateTime(selectedEvent.created_at_ms)} />
                </div>

                <div className="!mt-5 flex flex-wrap items-center gap-5 border-b border-slate-200 dark:border-[#2a303a]">
                  {INSPECTOR_TABS.map((tab) => (
                    <button
                      key={tab}
                      type="button"
                      onClick={() => setInspectorTab(tab)}
                      className={`-mb-px border-b-2 pb-3 text-[13px] font-medium transition ${
                        inspectorTab === tab
                          ? 'border-brand-500 text-brand-600 dark:text-brand-400'
                          : 'border-transparent text-slate-500 hover:text-slate-900 dark:text-[#a7b2c6] dark:hover:text-white'
                      } leading-[18px]`}
                    >
                      {INSPECTOR_TAB_LABELS[tab]}
                    </button>
                  ))}
                </div>
              </div>

              <div className="min-h-0 flex-1 space-y-5 overflow-y-auto px-5 py-5">
                {inspectorTab === 'summary' ? (
                  <div className="space-y-5">
                    {selectedEventIsDecision ? (
                      <ConnectorScorePanel
                        title="Connector scores"
                        value={inspectorModel.scoreContext}
                        selectedGateway={selectedEvent.gateway}
                        emptyMessage="No connector score map was captured for this event."
                      />
                    ) : null}
                    {inspectorModel.summaryRows.length ? (
                      <div className="space-y-3">
                        <h3 className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">Decision metadata</h3>
                        <InspectorKeyValueGrid rows={inspectorModel.summaryRows} />
                      </div>
                    ) : null}
                    <StructuredRecordPanel
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
                    title="Request"
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
              </div>
            </>
          ) : (
            <div className="min-h-0 flex-1 overflow-y-auto p-4">
              <EmptyState
                title="No event selected"
                body="Select a timeline event to view scores, routing details, request payload, and response payload."
              />
            </div>
          )}
        </GlassCard>
      </div>
    </div>
  )
}
