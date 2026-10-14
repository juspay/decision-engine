import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import { Link, useNavigationType, useSearchParams } from 'react-router-dom'
import { ArrowLeft, RefreshCw, Search as SearchIcon, SlidersHorizontal } from 'lucide-react'
import { fetcher } from '../../lib/api'
import { shellHeightClass } from '../../lib/embedMode'
import {
  AnalyticsRangeValue,
  PaymentAuditEvent,
  PaymentAuditFilterOptions,
  PaymentAuditExperiment,
  PaymentAuditResponse,
  PaymentAuditSummary,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { SearchableMultiSelect } from '../ui/SearchableMultiSelect'
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
import { useMerchantStore } from '../../store/merchantStore'
import {
  eventPhase as auditEventPhase,
  flowTypeValue,
  humanizeAuditValue,
  isDecisionFlow,
  routeLabel,
  stageLabel,
} from '../../lib/auditLabels'

/** Decision Audit files rule evaluations under "Rule Decision" — see `eventPhase`. */
function eventPhase(event: PaymentAuditEvent) {
  return auditEventPhase(event, 'Rule Decision')
}
/** The payment's outcome from its reported statuses, not any one event's status. Pick any number. */
const OUTCOME_OPTIONS = [
  { value: 'success', label: 'Success' },
  { value: 'failure', label: 'Failure' },
  { value: 'pending', label: 'Pending' },
]

type PaymentAttributeField = 'paymentMethod' | 'paymentMethodType' | 'cardNetwork' | 'authType' | 'currency'

/**
 * The payment attribute dropdowns. Their options are what the merchant's own payments carried in
 * the window (`/payment-audit/filter-options`), so each merchant sees its own methods, networks and
 * currencies. `labels` only names the common Hyperswitch values; any other value is shown as is.
 */
const PAYMENT_ATTRIBUTE_FILTERS: {
  field: PaymentAttributeField
  optionsKey: keyof Omit<PaymentAuditFilterOptions, 'gateways'>
  anyLabel: string
  labels: Record<string, string>
}[] = [
  {
    field: 'paymentMethod',
    optionsKey: 'payment_methods',
    anyLabel: 'Any payment method',
    labels: { upi: 'UPI', bank_redirect: 'Bank redirect', bank_transfer: 'Bank transfer', bank_debit: 'Bank debit', pay_later: 'Pay later' },
  },
  {
    field: 'paymentMethodType',
    optionsKey: 'payment_method_types',
    anyLabel: 'Any method type',
    labels: { apple_pay: 'Apple Pay', google_pay: 'Google Pay', paypal: 'PayPal', upi_collect: 'UPI collect', upi_intent: 'UPI intent' },
  },
  {
    field: 'cardNetwork',
    optionsKey: 'card_networks',
    anyLabel: 'Any card network',
    labels: { amex: 'American Express', americanexpress: 'American Express', rupay: 'RuPay', mastercard: 'Mastercard' },
  },
  {
    field: 'authType',
    optionsKey: 'auth_types',
    anyLabel: 'Any auth type',
    labels: { three_ds: '3DS', no_three_ds: 'No 3DS' },
  },
  { field: 'currency', optionsKey: 'currencies', anyLabel: 'Any currency', labels: {} },
]

/** How an attribute value reads in its dropdown: its known label, else the value tidied up. */
function attributeLabel(field: PaymentAttributeField, value: string, labels: Record<string, string>) {
  if (labels[value]) return labels[value]
  if (field === 'currency') return value.toUpperCase()
  return value
    .split('_')
    .map((word, index) => (index === 0 ? word.charAt(0).toUpperCase() + word.slice(1) : word))
    .join(' ')
}
const ROUTE_OPTIONS = [
  { value: 'decide_gateway', label: 'Decide Gateway' },
  { value: 'update_gateway_score', label: 'Update Gateway' },
  { value: 'routing_evaluate', label: 'Rule Evaluate' },
  { value: 'routing_hybrid', label: 'Hybrid Routing' },
]
const INSPECTOR_TABS = ['summary', 'input', 'response', 'raw'] as const
const INSPECTOR_TAB_LABELS: Record<(typeof INSPECTOR_TABS)[number], string> = {
  summary: 'Summary',
  input: 'Request',
  response: 'Response',
  raw: 'Raw JSON',
}
const DEBIT_ROUTING_APPROACH = 'NTW_BASED_ROUTING'
const AUDIT_PATH = '/analytics/payment-audit'
/** Payments per page; the API serves up to 100. */
const PAGE_SIZE_OPTIONS = [10, 20, 50, 100] as const
const DEFAULT_PAGE_SIZE = 20

/**
 * The last search per merchant, so leaving the page and coming back keeps its filters, range and
 * page size. Storage can be unavailable (private windows, blocked site data); the page then just
 * starts unfiltered.
 */
function savedSearchKey(merchantId: string) {
  return `decision-audit:last-search:${merchantId}`
}

function readSavedSearch(merchantId: string): URLSearchParams | null {
  if (!merchantId) return null
  try {
    const saved = window.localStorage.getItem(savedSearchKey(merchantId))
    return saved ? new URLSearchParams(saved) : null
  } catch {
    return null
  }
}

/** Saves what defines the search, not where the user was in it (page, selected payment). */
function saveSearch(merchantId: string, query: string) {
  if (!merchantId) return
  const params = new URLSearchParams(query)
  params.delete('page')
  params.delete('selected')
  try {
    window.localStorage.setItem(savedSearchKey(merchantId), params.toString())
  } catch {
    // Not saved; the URL still carries this search.
  }
}

function parsePageSize(searchParams: URLSearchParams): number {
  const value = Number(searchParams.get('page_size'))
  return (PAGE_SIZE_OPTIONS as readonly number[]).includes(value) ? value : DEFAULT_PAGE_SIZE
}
/**
 * The routing-type filter. These were the page's tabs; each value lists the payments whose
 * trail holds that kind of event, while every payment's trail itself always shows all of its
 * events (rule evaluation, gateway decision, score update).
 */
const ROUTING_KIND_OPTIONS = [
  { value: '', label: 'Any routing type' },
  { value: 'multi_objective', label: 'Multi-objective' },
  { value: 'rule_based', label: 'Rule based / Volume based' },
  { value: 'debit_routing', label: 'Debit routing' },
  { value: 'hybrid', label: 'Hybrid' },
]

type AuditFilters = {
  paymentId: string
  requestId: string
  gateway: string
  route: string
  status: string
  flowType: string
  errorCode: string
  routingKind: string
  currency: string
  paymentMethod: string
  paymentMethodType: string
  cardNetwork: string
  authType: string
  minAmount: string
  maxAmount: string
}

type InspectorTab = (typeof INSPECTOR_TABS)[number]

const EMPTY_FILTERS: AuditFilters = {
  paymentId: '',
  requestId: '',
  gateway: '',
  route: '',
  status: '',
  flowType: '',
  errorCode: '',
  routingKind: '',
  currency: '',
  paymentMethod: '',
  paymentMethodType: '',
  cardNetwork: '',
  authType: '',
  minAmount: '',
  maxAmount: '',
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
    routingKind: filters.routingKind,
    currency: filters.currency.trim(),
    paymentMethod: filters.paymentMethod.trim(),
    paymentMethodType: filters.paymentMethodType.trim(),
    cardNetwork: filters.cardNetwork.trim(),
    authType: filters.authType.trim(),
    minAmount: filters.minAmount.trim(),
    maxAmount: filters.maxAmount.trim(),
  }
}

/**
 * The payment filters as query params, for both the API and the page URL. The status dropdown is
 * the payment's outcome (`outcome`), not an event's raw status.
 */
function paymentFilterParams(filters: AuditFilters): Record<string, string | undefined> {
  return {
    outcome: filters.status || undefined,
    currency: filters.currency || undefined,
    payment_method: filters.paymentMethod || undefined,
    payment_method_type: filters.paymentMethodType || undefined,
    card_network: filters.cardNetwork || undefined,
    auth_type: filters.authType || undefined,
    min_amount: filters.minAmount || undefined,
    max_amount: filters.maxAmount || undefined,
  }
}

function looksLikeRequestIdentifier(value: string) {
  return (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value) ||
    /^req[_-]/i.test(value)
  )
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
    ...paymentFilterParams(normalizedFilters),
    flow_type: normalizedFilters.flowType || undefined,
    routing_kind: normalizedFilters.routingKind || undefined,
    error_code: normalizedFilters.errorCode || undefined,
  }
  const qs = queryString(params)
  return qs ? `${AUDIT_PATH}?${qs}` : AUDIT_PATH
}

function parseRoutingKind(searchParams: URLSearchParams): string {
  const explicit = searchParams.get('routing_kind') || ''
  if (ROUTING_KIND_OPTIONS.some((option) => option.value && option.value === explicit)) return explicit
  const legacyMode = searchParams.get('mode')
  if (legacyMode === 'rule_based' || legacyMode === 'debit_routing') return legacyMode
  if (searchParams.get('routing_approach') === DEBIT_ROUTING_APPROACH) return 'debit_routing'
  if (searchParams.get('exclude_routing_approach') === DEBIT_ROUTING_APPROACH) return 'multi_objective'
  return ''
}

/** The outcome filter; a `status=success|failure` link from before outcomes reads as one. */
/** The outcome filter (a comma-separated list); a `status=success|failure` link from before outcomes reads as one. */
function parseOutcome(searchParams: URLSearchParams): string {
  const raw = (searchParams.get('outcome') || searchParams.get('status') || '').toLowerCase()
  return raw
    .split(',')
    .map((value) => value.trim())
    .filter((value) => OUTCOME_OPTIONS.some((option) => option.value === value))
    .join(',')
}

function parseFilters(searchParams: URLSearchParams): AuditFilters {
  return normalizeAuditFilters({
    paymentId: searchParams.get('payment_id') || searchParams.get('request_id') || '',
    requestId: '',
    gateway: searchParams.get('gateway') || '',
    route: searchParams.get('route') || '',
    status: parseOutcome(searchParams),
    flowType: searchParams.get('flow_type') || searchParams.get('event_type') || '',
    errorCode: searchParams.get('error_code') || '',
    routingKind: parseRoutingKind(searchParams),
    currency: searchParams.get('currency') || '',
    paymentMethod: searchParams.get('payment_method') || '',
    paymentMethodType: searchParams.get('payment_method_type') || '',
    cardNetwork: searchParams.get('card_network') || '',
    authType: searchParams.get('auth_type') || '',
    minAmount: searchParams.get('min_amount') || '',
    maxAmount: searchParams.get('max_amount') || '',
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

function compactMeta(parts: Array<string | null | undefined | false>) {
  return parts.filter(Boolean).join(' · ')
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
  const raw = (row.gateways || []).filter(Boolean)
  // A trail names the same connector both ways: the rule step records `paypal_test`, the decision
  // and score update record `paypal_test:mca_…`. Keep the id-bearing name and drop its bare twin,
  // so only a genuine change of connector shows as a path.
  const gateways = raw.filter(
    (gateway) => !raw.some((other) => other !== gateway && other.split(':')[0] === gateway),
  )
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

const EXPERIMENT_ENDPOINT_LABELS: Record<string, string> = {
  hybrid_routing: 'hybrid routing',
  decide_gateway: 'decide gateway',
  evaluate: 'rule evaluate',
}

/**
 * Which A/B arm routed the payment, linking to its experiment; or that it wasn't routed under one.
 * Experiment events stay out of the timeline itself, so this is the audit's only A/B signal.
 */
function ExperimentAssignment({ experiments }: { experiments: PaymentAuditExperiment[] }) {
  if (experiments.length === 0) {
    return <Badge variant="gray">Not in an A/B experiment</Badge>
  }
  return (
    <>
      {experiments.map((assignment) => {
        const endpoint = EXPERIMENT_ENDPOINT_LABELS[assignment.endpoint] ?? assignment.endpoint
        return (
          <Link
            key={`${assignment.experiment_id}:${assignment.arm}:${assignment.endpoint}`}
            to={`/routing/ab-testing?experiment=${encodeURIComponent(assignment.experiment_id)}`}
            title={`Experiment ${assignment.experiment_id} · routed on ${endpoint} at ${formatDateTime(assignment.routed_at_ms)}`}
          >
            <Badge variant={assignment.arm === 'variant' ? 'purple' : 'blue'}>
              A/B · {assignment.arm} · {endpoint}
            </Badge>
          </Link>
        )
      })}
    </>
  )
}

/** The A/B arm(s) of a result row; nothing for a payment outside any experiment, to keep the list quiet. */
function ExperimentArmTag({ experiments }: { experiments?: PaymentAuditExperiment[] }) {
  if (!experiments?.length) return null
  const arms = [...new Set(experiments.map((assignment) => assignment.arm))]
  const title = experiments
    .map((a) => `${a.arm} · ${EXPERIMENT_ENDPOINT_LABELS[a.endpoint] ?? a.endpoint} · ${a.experiment_id}`)
    .join('\n')
  return (
    <span className="shrink-0" title={title}>
      <Badge variant={arms.length === 1 && arms[0] === 'variant' ? 'purple' : 'blue'}>A/B · {arms.join(' / ')}</Badge>
    </span>
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
  const navigationType = useNavigationType()
  const merchantId = useMerchantStore((state) => state.merchantId)

  // Arriving without a search in the URL (e.g. from the sidebar) resumes the merchant's last one.
  const [startupParams] = useState(() =>
    searchParams.toString() ? searchParams : readSavedSearch(merchantId) ?? searchParams,
  )
  useEffect(() => {
    if (startupParams !== searchParams) setSearchParams(startupParams, { replace: true })
    // Once, on arrival: later URL changes come from the page's own searches.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const initialRange = startupParams.get('start_ms') && startupParams.get('end_ms')
    ? 'custom'
    : parseRange(startupParams.get('range'))
  const initialFilters = parseFilters(startupParams)
  const initialPage = Math.max(1, Number(startupParams.get('page') || '1'))
  const initialSelectedKey = startupParams.get('selected') || ''
  const initialStartMs = Number(startupParams.get('start_ms') || '0')
  const initialEndMs = Number(startupParams.get('end_ms') || '0')
  const initialCustomWindow =
    initialStartMs > 0 && initialEndMs > initialStartMs
      ? { start_ms: initialStartMs, end_ms: initialEndMs }
      : presetWindow('1d')

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
  const [pageSize, setPageSize] = useState(() => parsePageSize(startupParams))

  useEffect(() => {
    if (navigationType !== 'POP') return

    const nextRange = searchParams.get('start_ms') && searchParams.get('end_ms')
      ? 'custom'
      : parseRange(searchParams.get('range'))
    const nextFilters = parseFilters(searchParams)
    const nextSelectedKey = searchParams.get('selected') || ''
    const startMs = Number(searchParams.get('start_ms') || '0')
    const endMs = Number(searchParams.get('end_ms') || '0')
    const nextWindow = startMs > 0 && endMs > startMs
      ? { start_ms: startMs, end_ms: endMs }
      : presetWindow(nextRange === 'custom' ? '1d' : nextRange)

    setRange(nextRange)
    setFilters(nextFilters)
    setAppliedFilters(nextFilters)
    setPage(Math.max(1, Number(searchParams.get('page') || '1')))
    setPageSize(parsePageSize(searchParams))
    setSelectedKey(nextSelectedKey)
    setTrailFocused(Boolean(nextSelectedKey))
    setSelectedEventId(null)
    setInspectorTab('summary')
    setCustomStart(toDateTimeInputValue(nextWindow.start_ms))
    setCustomEnd(toDateTimeInputValue(nextWindow.end_ms))
  }, [navigationType, searchParams])

  const customWindow = useMemo(
    () => (range === 'custom' ? customWindowFrom(customStart, customEnd) : undefined),
    [customEnd, customStart, range],
  )

  const isExactSearch = Boolean(appliedFilters.paymentId || appliedFilters.requestId)
  const searchUrl =
    isExactSearch || range !== 'custom' || customWindow
      ? buildAuditUrl(range, page, pageSize, appliedFilters, customWindow)
      : null

  const auditSearch = useSWR<PaymentAuditResponse>(searchUrl, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 5000,
  })

  // The dropdowns' values: what this merchant's payments in the window actually carried. One cheap
  // query per window (cached a minute), which also names every gateway, including those that fall
  // on later pages of the results.
  const filterOptionsUrl =
    range !== 'custom' || customWindow
      ? `${AUDIT_PATH}/filter-options?${queryString({
          range: range === 'custom' ? '1h' : range,
          start_ms: customWindow?.start_ms,
          end_ms: customWindow?.end_ms,
        })}`
      : null
  const filterOptions = useSWR<PaymentAuditFilterOptions>(filterOptionsUrl, fetcher, {
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
    // The trail of a selected payment always shows all of its events, whatever the list filter.
    return { ...EMPTY_FILTERS, paymentId: lookupValue }
  }, [selectedSummary])

  const detailUrl = detailFilters
    ? buildAuditUrl(range, 1, 50, detailFilters, customWindow)
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
  const firstRowNumber = (page - 1) * pageSize + 1
  const lastRowNumber = (page - 1) * pageSize + resultRows.length
  const totalEvents = timeline.length
  const successCount = auditSearch.data?.total_success ?? resultRows.filter((row) => row.outcome === 'success').length
  const failureCount = auditSearch.data?.total_failure ?? resultRows.filter((row) => row.outcome === 'failure').length
  // Payment outcomes: success and failure are terminal; the rest have no outcome reported yet.
  const pendingCount = Math.max(0, totalMatches - successCount - failureCount)
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
  // Error codes the merchant's payments hit in the window, plus any selected from a link.
  const errorCodeOptions = Array.from(
    new Set([...(filterOptions.data?.error_codes || []), ...appliedFilters.errorCode.split(',')].filter(Boolean)),
  ).sort((left, right) => left.localeCompare(right))
  const gatewayOptions = Array.from(
    new Set(
      [
        ...(filterOptions.data?.gateways || []),
        ...activeGatewayList,
        // A gateway supplied by URL stays selectable even if it has gone quiet since.
        ...appliedFilters.gateway.split(','),
      ].filter(Boolean),
    ),
  ).sort((left, right) => left.localeCompare(right))
  const content = {
    title: 'Decision Audit',
    description:
      'Search by payment or request, then inspect every step the payment went through — rule evaluations, gateway decisions, score updates, debit routing, and errors — with the exact payload captured at each step.',
    merchantPrompt: 'Audit data follows your signed-in merchant account.',
    searchTitle: 'Search Decision Trail',
    searchDescription: 'Use payment or request IDs when you have them. Routing type, gateway, status, route, and error code narrow results quickly.',
    matchingLabel: 'Matches',
    matchingDescription: 'Scan the current result set and pick a payment to open its full event trail.',
    summaryLabel: 'Selected Payment Timeline',
    summaryEmpty: 'Pick a payment from the left column to see the full transaction trail.',
    noMatchesTitle: 'No matching payments found',
    noMatchesBody: isExactSearch
      ? 'No retained audit history matches this ID. Check the payment or request ID and try again.'
      : 'Try widening the time range, clearing the routing type filter, or searching by a single payment ID, request ID, or error code.',
  }

  function syncSearch(
    nextRange: AnalyticsRangeValue,
    nextPage: number,
    nextFilters: AuditFilters,
    nextSelectedKey?: string,
    nextCustomWindow?: TimeWindow,
    nextPageSize: number = pageSize,
  ) {
    const normalizedFilters = normalizeAuditFilters(nextFilters)
    const nextQuery = queryString({
      range: nextRange,
      page: nextPage > 1 ? nextPage : undefined,
      page_size: nextPageSize !== DEFAULT_PAGE_SIZE ? nextPageSize : undefined,
      start_ms: nextRange === 'custom' ? nextCustomWindow?.start_ms : undefined,
      end_ms: nextRange === 'custom' ? nextCustomWindow?.end_ms : undefined,
      payment_id: normalizedFilters.paymentId || undefined,
      request_id: normalizedFilters.requestId || undefined,
      gateway: normalizedFilters.gateway || undefined,
      route: normalizedFilters.route || undefined,
      ...paymentFilterParams(normalizedFilters),
      flow_type: normalizedFilters.flowType || undefined,
      routing_kind: normalizedFilters.routingKind || undefined,
      error_code: normalizedFilters.errorCode || undefined,
      selected: nextSelectedKey || undefined,
    })
    setSearchParams(nextQuery)
    saveSearch(merchantId, nextQuery)
  }

  function updateFilter(field: keyof AuditFilters, value: string) {
    setFilters((current) => normalizeAuditFilters({ ...current, [field]: value }))
  }

  function applyFilters() {
    const nextPage = 1
    const normalizedFilters = normalizeAuditFilters(filters)
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    setFilters(normalizedFilters)
    setAppliedFilters(normalizedFilters)
    syncSearch(range, nextPage, normalizedFilters, undefined, customWindow)
  }

  /**
   * Multi-select dropdowns: a tick only updates the pending selection; closing the list searches
   * once, and only if the selection changed. Five ticks cost one search, not five.
   */
  function applyIfChanged() {
    const normalizedFilters = normalizeAuditFilters(filters)
    if (JSON.stringify(normalizedFilters) === JSON.stringify(appliedFilters)) return
    applyFilters()
  }

  /** Dropdown filters apply on change — the redesigned bar has no Search button to press. */
  function applyDropdownFilter(
    field: keyof AuditFilters,
    value: string,
  ) {
    const nextPage = 1
    const normalizedFilters = normalizeAuditFilters({ ...filters, [field]: value })
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    setFilters(normalizedFilters)
    setAppliedFilters(normalizedFilters)
    syncSearch(range, nextPage, normalizedFilters, undefined, customWindow)
  }

  function clearFilters() {
    const nextPage = 1
    const clearedFilters = { ...EMPTY_FILTERS }
    setPage(nextPage)
    setTrailFocused(false)
    setSelectedEventId(null)
    setFilters(clearedFilters)
    setAppliedFilters(clearedFilters)
    syncSearch(range, nextPage, clearedFilters, undefined, customWindow)
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
      nextRange,
      nextPage,
      appliedFilters,
      undefined,
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
      'custom',
      nextPage,
      appliedFilters,
      undefined,
      customWindowFrom(nextStart, nextEnd),
    )
  }

  function selectSummary(lookupKey: string, eventCount?: number) {
    setSelectedKey(lookupKey)
    setSelectedEventId(null)
    // Single-event payments have nothing to choose in the trail — keep the results
    // list visible and let the right panel populate directly from the auto-selected event.
    if (eventCount !== 1) setTrailFocused(true)
    syncSearch(range, page, appliedFilters, lookupKey, customWindow)
  }

  function returnToResults() {
    setTrailFocused(false)
    setSelectedEventId(null)
    syncSearch(range, page, appliedFilters, undefined, customWindow)
  }

  // Full-height column so the two panels fill the shell (top bar, which embed mode hides, plus
  // main's vertical padding — see shellHeightClass)
  // and scroll internally, as they did before the header moved into the page.
  return (
    <div className={`flex min-h-[620px] flex-col gap-5 ${shellHeightClass()}`}>
      {/* The mock draws these controls in an app-wide bar; the shell already owns that strip, so the
          time range lives at the top of the page's own content instead. */}
      <div className="grid grid-cols-1 items-center gap-3 xl:grid-cols-[1fr_auto]">
        <PageHeading title={content.title} />

        <div className="flex items-center gap-2 justify-self-start xl:justify-self-end">
          {isExactSearch ? (
            <span
              className="rounded-full border border-slate-200 px-4 py-2 text-sm font-medium text-slate-600 dark:border-[#2a303a] dark:text-slate-300"
              title="Payment ID and request ID searches include all retained audit history."
            >
              All history
            </span>
          ) : (
            <TimeRangeFilter
              range={range}
              customStart={customStart}
              customEnd={customEnd}
              onRangeChange={updateRange}
              onCustomChange={applyCustomWindow}
            />
          )}
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
            placeholder="Search by payment ID or request ID…"
            aria-label="Payment ID"
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

        <div className="flex min-w-[480px] flex-1 items-center gap-3">
        <div className="flex-1">
          <select
            className={`${fieldSelectClassName()} min-w-[168px]`}
            value={filters.routingKind}
            onChange={(event) => applyDropdownFilter('routingKind', event.target.value)}
            aria-label="Routing type"
          >
            {ROUTING_KIND_OPTIONS.map((option) => (
              <option key={option.value || 'all'} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>

        <div className="flex-1">
          <SearchableMultiSelect
            values={filters.gateway.split(',').filter(Boolean)}
            options={gatewayOptions.map((gateway) => ({ value: gateway, label: gateway }))}
            onChange={(values) => updateFilter('gateway', values.join(','))}
            onClose={applyIfChanged}
            placeholder="Any gateway"
            triggerClassName={fieldClassName()}
            compact
          />
        </div>

        <div className="flex-1">
          <SearchableMultiSelect
            values={filters.status.split(',').filter(Boolean)}
            options={OUTCOME_OPTIONS}
            onChange={(values) => updateFilter('status', values.join(','))}
            onClose={applyIfChanged}
            placeholder="Any outcome"
            triggerClassName={fieldClassName()}
            compact
          />
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
        {showAdvancedFilters ? (
          <div className="grid w-full basis-full grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
            <SearchableMultiSelect
              values={filters.route.split(',').filter(Boolean)}
              options={ROUTE_OPTIONS}
              onChange={(values) => updateFilter('route', values.join(','))}
              onClose={applyIfChanged}
              placeholder="Any route"
              triggerClassName={fieldClassName()}
              compact
            />
            <SearchableMultiSelect
              values={filters.errorCode.split(',').filter(Boolean)}
              options={errorCodeOptions.map((code) => ({ value: code, label: code }))}
              onChange={(values) => updateFilter('errorCode', values.join(','))}
              onClose={applyIfChanged}
              placeholder="Any error code"
              triggerClassName={fieldClassName()}
              compact
            />
            {PAYMENT_ATTRIBUTE_FILTERS.map(({ field, optionsKey, anyLabel, labels }) => {
              // The merchant's own values, lower-cased as the filters match them.
              const values = filterOptions.data?.[optionsKey] ?? []
              const options = values.map((value) => ({ value, label: attributeLabel(field, value, labels) }))
              // Selected values, canonicalised to an option ignoring case. A value from a link that
              // no recent payment carried stays selectable.
              const selected = filters[field]
                .split(',')
                .map((value) => value.trim())
                .filter(Boolean)
                .map((value) => options.find((o) => o.value.toLowerCase() === value.toLowerCase())?.value ?? value)
              const unlisted = selected
                .filter((value) => !options.some((o) => o.value === value))
                .map((value) => ({ value, label: attributeLabel(field, value, labels) }))
              return (
                <SearchableMultiSelect
                  key={field}
                  values={selected}
                  options={[...unlisted, ...options]}
                  onChange={(next) => updateFilter(field, next.join(','))}
                  onClose={applyIfChanged}
                  placeholder={anyLabel}
                  triggerClassName={fieldClassName()}
                  compact
                />
              )
            })}
            <input
              className={fieldClassName()}
              type="number"
              min={0}
              step="any"
              value={filters.minAmount}
              onChange={(event) => updateFilter('minAmount', event.target.value)}
              placeholder="Min amount"
              aria-label="Min amount"
            />
            <input
              className={fieldClassName()}
              type="number"
              min={0}
              step="any"
              value={filters.maxAmount}
              onChange={(event) => updateFilter('maxAmount', event.target.value)}
              placeholder="Max amount"
              aria-label="Max amount"
            />
            <Button type="submit" variant="secondary" className="h-11">
              Apply
            </Button>
          </div>
        ) : null}
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
            <SummaryStat tone="bg-emerald-500" label={`${successCount.toLocaleString()} successful`} />
            <SummaryStat tone="bg-red-500" label={`${failureCount.toLocaleString()} failed`} />
            <SummaryStat tone="bg-slate-400" label={`${pendingCount.toLocaleString()} pending`} />
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
                  {selectedSummary?.experiments ? (
                    <ExperimentAssignment experiments={selectedSummary.experiments} />
                  ) : null}
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
                {resultRows.length > 0
                  ? `${firstRowNumber.toLocaleString()}–${lastRowNumber.toLocaleString()} of ${totalMatches.toLocaleString()}`
                  : `0 of ${totalMatches.toLocaleString()}`}
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
                    <div className="flex min-w-0 flex-1 items-center gap-2">
                      <span
                        className={`h-1.5 w-1.5 shrink-0 rounded-full ${statusDotClass(row.outcome)}`}
                        title={`${humanizeAuditValue(row.outcome)} · last status ${humanizeAuditValue(row.latest_status) || 'unknown'}`}
                      />
                      {row.latest_gateway ? (
                        <span
                          className="min-w-0 truncate text-[13px] text-slate-500 dark:text-[#a7b2c6] leading-[18px]"
                          title={row.latest_gateway}
                        >
                          {row.latest_gateway}
                        </span>
                      ) : null}
                      <span className="shrink-0 whitespace-nowrap text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                        · {row.event_count} event{row.event_count === 1 ? '' : 's'}
                      </span>
                      <ExperimentArmTag experiments={row.experiments} />
                    </div>
                    {gatewayPath ? (
                      <span
                        className="min-w-0 max-w-[45%] truncate rounded-md bg-orange-500/10 px-2 py-0.5 text-[13px] font-medium text-orange-600 ring-1 ring-inset ring-orange-500/20 dark:text-orange-300 leading-[18px]"
                        title={gatewayPath}
                      >
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
                  syncSearch(range, nextPage, appliedFilters, undefined, customWindow)
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
                  syncSearch(range, nextPage, appliedFilters, undefined, customWindow)
                }}
              >
                Next
              </Button>
              <span className="ml-auto text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">Page {page}</span>
              <select
                className="h-8 rounded-lg border border-slate-200 bg-white pl-2.5 pr-8 [background-position:right_0.5rem_center] text-[13px] text-slate-700 outline-none focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7]"
                value={pageSize}
                onChange={(event) => {
                  const nextPageSize = Number(event.target.value)
                  setPageSize(nextPageSize)
                  setPage(1)
                  setTrailFocused(false)
                  syncSearch(range, 1, appliedFilters, undefined, customWindow, nextPageSize)
                }}
                aria-label="Payments per page"
              >
                {PAGE_SIZE_OPTIONS.map((size) => (
                  <option key={size} value={size}>
                    {size} / page
                  </option>
                ))}
              </select>
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
