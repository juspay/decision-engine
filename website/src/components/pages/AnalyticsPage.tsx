import { useEffect, useMemo, useRef, useState } from 'react'
import useSWR from 'swr'
import { useNavigate } from 'react-router-dom'
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { useMerchantStore } from '../../store/merchantStore'
import { fetcher } from '../../lib/api'
import {
  AnalyticsDecisionResponse,
  AnalyticsGatewayScoresResponse,
  AnalyticsLogSummariesResponse,
  AnalyticsOverviewResponse,
  AnalyticsRange,
  AnalyticsRoutingStatsResponse,
  AnalyticsScope,
  GatewayScoreSeriesPoint,
  RoutingFilterOptions,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'

type Section = 'overview' | 'scores' | 'decisions' | 'routing' | 'logs'
type RoutingFilters = {
  paymentMethodType: string
  paymentMethod: string
  gateways: string[]
}
type InfoContent = {
  title: string
  purpose: string
  calculation: string
  source: string
}

const SECTION_LABELS: Record<Section, string> = {
  overview: 'Overview',
  scores: 'Gateway Scoring',
  decisions: 'Decisions',
  routing: 'Routing Stats',
  logs: 'Logs / Summaries',
}

const RANGE_OPTIONS: AnalyticsRange[] = ['15m', '1h', '24h']
const CHART_TOOLTIP_STYLE = {
  backgroundColor: '#0d0d12',
  border: '1px solid #1c1c24',
  borderRadius: '12px',
  color: '#e8e8f4',
  boxShadow: '0 16px 40px rgba(0, 0, 0, 0.35)',
}
const CHART_TOOLTIP_LABEL_STYLE = {
  color: '#f8fafc',
  fontWeight: 600,
  marginBottom: 8,
}
const CHART_TOOLTIP_ITEM_STYLE = {
  color: '#e2e8f0',
}
const CHART_TOOLTIP_WRAPPER_STYLE = {
  zIndex: 30,
  outline: 'none',
}

const EMPTY_ROUTING_FILTERS: RoutingFilters = {
  paymentMethodType: '',
  paymentMethod: '',
  gateways: [],
}

const CARD_INFO: Record<string, InfoContent> = {
  topScores: {
    title: 'Top score snapshots',
    purpose: 'Use this to answer which connector currently looks strongest for a merchant and payment slice without going to Redis or raw tables.',
    calculation: 'Each row is the latest recorded `score_snapshot` for a unique merchant, payment method type, payment method, and connector combination. The sparkline is the stored time-ordered score history for that same slice.',
    source: 'Rendered from persisted `analytics_event` score snapshots in Postgres. Those snapshots are produced from the Redis-backed gateway scoring flow, but this window itself reads the stored analytics history.',
  },
  recentErrors: {
    title: 'Recent errors',
    purpose: 'Use this to see whether routing, score updates, or audit capture are failing in a repeatable pattern.',
    calculation: 'Rows are grouped by route, error code, and error message. Count is the number of matching structured error events in the selected window, and last seen is the newest timestamp among them.',
    source: 'Reads grouped `error` events from the `analytics_event` history in Postgres.',
  },
  gatewayScoring: {
    title: 'Gateway scoring',
    purpose: 'Use this when you need the exact score inputs that explain why one connector beat another at decision time.',
    calculation: 'The table shows the latest `score_snapshot` per merchant, payment method type, payment method, and connector. Score, sigma, average latency, TP99 latency, and transaction count are taken directly from the captured snapshot payload.',
    source: 'Reads persisted `score_snapshot` rows from `analytics_event` in Postgres. Those rows are emitted from the gateway scoring service, which itself uses Redis-backed scoring state.',
  },
  decisionThroughput: {
    title: 'Decision throughput by routing approach',
    purpose: 'Use this to see how much routing traffic is being served and which approach is taking that traffic right now.',
    calculation: 'Each chart point is the number of `decision` events in a time bucket grouped by `routing_approach`. The tiles above it are computed from the same event set: total decisions and failures divided by total decisions for error rate.',
    source: 'Reads persisted `decision` events from `analytics_event` in Postgres. The page complements, but does not directly read, the in-process Prometheus counters.',
  },
  gatewayShare: {
    title: 'Gateway share over time',
    purpose: 'Use this to see whether traffic shifted sharply toward one connector or away from another.',
    calculation: 'Each stacked bar counts `decision` events per time bucket grouped by chosen connector. Taller share for a connector means more payments were routed there in that period.',
    source: 'Reads persisted `decision` events from `analytics_event` in Postgres.',
  },
  topRules: {
    title: 'Top priority logic hits',
    purpose: 'Use this to see which rules are actively steering routing so rule-driven behaviour is obvious without querying storage directly.',
    calculation: 'Every `rule_hit` event increments the count for its rule name. The list is then sorted by descending hit count for the selected window.',
    source: 'Reads persisted `rule_hit` events from `analytics_event` in Postgres.',
  },
  connectorTrend: {
    title: 'Connector success rate over time',
    purpose: 'Use this to explain why a connector won routing at a given time, for example why Stripe was picked because its recorded score or SR trend was higher then.',
    calculation: 'Built from stored `score_snapshot` history. Snapshot points are bucketed by time and connector, and multiple points in the same bucket are averaged. Merchant scope applies payment method filters before bucketing. Global mode intentionally collapses to connector-only trends.',
    source: 'Reads persisted `score_snapshot` events from `analytics_event` in Postgres. Live scoring still comes from Redis-backed scoring flows; this window shows the stored historical trail.',
  },
  errorSummaries: {
    title: 'Error summaries',
    purpose: 'Use this to prioritise the noisiest operational failures first instead of reading raw logs line by line.',
    calculation: 'Structured `error` events are grouped by route, code, and message. Count shows recurrence and the rows are ordered by frequency in the selected window.',
    source: 'Reads grouped `error` events from `analytics_event` in Postgres.',
  },
  recentSamples: {
    title: 'Recent samples',
    purpose: 'Use this as the fastest jump point into audit for a real payment, request, or failure sample.',
    calculation: 'Rows are recent structured analytics events ordered by timestamp. Each card shows the route, latest status or error, and any captured request or payment identifiers that can deep-link into the audit page.',
    source: 'Reads recent events across `decision`, `score_snapshot`, `rule_hit`, and `error` from `analytics_event` in Postgres.',
  },
}

const KPI_INFO: InfoContent[] = [
  {
    title: 'Decisions',
    purpose: 'Use this to understand real routed volume in the selected merchant or window.',
    calculation: 'Every persisted `decision` event counts once. Labels such as Decisions / 1h or Decisions / 24h reflect the active time window only.',
    source: 'Computed from `decision` rows in `analytics_event` in Postgres.',
  },
  {
    title: 'Score snapshots',
    purpose: 'Use this to understand how much score history exists for explaining connector movement.',
    calculation: 'Every persisted `score_snapshot` event counts once.',
    source: 'Computed from `score_snapshot` rows in `analytics_event` in Postgres.',
  },
  {
    title: 'Rule hits',
    purpose: 'Use this to gauge how much explicit routing logic is influencing traffic.',
    calculation: 'Every persisted `rule_hit` event counts once.',
    source: 'Computed from `rule_hit` rows in `analytics_event` in Postgres.',
  },
  {
    title: 'Errors',
    purpose: 'Use this to see how many structured failures were captured in the selected window.',
    calculation: 'Every persisted `error` event counts once.',
    source: 'Computed from `error` rows in `analytics_event` in Postgres.',
  },
  {
    title: 'Error rate',
    purpose: 'Use this to understand what percentage of recorded routing decisions failed, not just the raw number of failures.',
    calculation: 'Computed as failed `decision` events divided by all `decision` events in the selected window, then converted to a percentage.',
    source: 'Computed from `decision` rows and their `status` values in `analytics_event` in Postgres.',
  },
]

function queryString(params: Record<string, string | number | string[] | undefined>) {
  const search = new URLSearchParams()
  Object.entries(params).forEach(([key, value]) => {
    if (Array.isArray(value)) {
      if (value.length) {
        search.set(key, value.join(','))
      }
      return
    }

    if (value !== undefined && value !== '') {
      search.set(key, String(value))
    }
  })
  return search.toString()
}

function buildAnalyticsUrl(
  path: string,
  scope: AnalyticsScope,
  range: AnalyticsRange,
  merchantId: string | undefined,
  page = 1,
  pageSize = 10,
  extraParams: Record<string, string | number | string[] | undefined> = {},
) {
  const params: Record<string, string | number | string[] | undefined> = {
    scope,
    range,
    page,
    page_size: pageSize,
    ...extraParams,
  }
  if (scope === 'current' && merchantId) {
    params.merchant_id = merchantId
  }
  const qs = queryString(params)
  return qs ? `${path}?${qs}` : path
}

function selectClassName(disabled = false) {
  return `h-10 rounded-2xl border border-slate-200 bg-white px-3 text-sm text-slate-700 shadow-sm outline-none transition focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#e5e7eb] ${disabled ? 'cursor-not-allowed opacity-50' : ''}`
}

function filterBadgeClass(active: boolean) {
  return active
    ? 'border-brand-500/40 bg-brand-500/10 text-brand-700 dark:text-brand-200'
    : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300 hover:text-slate-900 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#a1a1aa] dark:hover:border-[#3a3a44] dark:hover:text-white'
}

function formatNumber(value: number | string | undefined, digits = 2) {
  if (value === undefined || value === null || Number.isNaN(Number(value))) {
    return '0'
  }
  const numberValue = Number(value)
  if (Number.isInteger(numberValue)) return numberValue.toString()
  return numberValue.toFixed(digits)
}

function toPercent(value: number) {
  if (!Number.isFinite(value)) return 0
  return value <= 1 ? value * 100 : value
}

function formatPercent(value: number | string | undefined, digits = 1) {
  if (value === undefined || value === null || Number.isNaN(Number(value))) {
    return '0%'
  }
  return `${formatNumber(toPercent(Number(value)), digits)}%`
}

function formatDateTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'short',
    timeStyle: 'short',
  }).format(new Date(ms))
}

function formatBucket(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(ms))
}

function humanizeAuditRoute(route?: string | null) {
  if (!route) return 'Unknown route'
  if (route === 'decision_gateway' || route === 'decide_gateway') return 'Decide Gateway'
  return route
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase()
    .replace(/\b\w/g, (char) => char.toUpperCase())
}

function makeScoreKey(point: Pick<GatewayScoreSeriesPoint, 'merchant_id' | 'payment_method_type' | 'payment_method' | 'gateway'>) {
  return [point.merchant_id, point.payment_method_type, point.payment_method, point.gateway].join('|')
}

function sectionButtonClass(active: boolean) {
  return active ? 'bg-brand-600 text-white' : 'bg-white text-slate-600 border border-slate-200 hover:bg-slate-50 dark:bg-[#121214] dark:text-[#a1a1aa] dark:border-[#27272a]'
}

function infoMatchForMetric(label: string): InfoContent | null {
  const normalized = label.toLowerCase()
  if (normalized.startsWith('decisions /')) return KPI_INFO[0]
  if (normalized === 'decisions') return KPI_INFO[0]
  if (normalized === 'score snapshots') return KPI_INFO[1]
  if (normalized === 'rule hits') return KPI_INFO[2]
  if (normalized === 'errors') return KPI_INFO[3]
  if (normalized === 'error rate') return KPI_INFO[4]
  return null
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-slate-200 dark:border-[#222227] bg-white/60 dark:bg-[#0b0b0d] px-6 py-12 text-center">
      <p className="text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#8a8a93]">{body}</p>
    </div>
  )
}

function MetricCard({ label, value, subtitle }: { label: string; value: string; subtitle?: string | null }) {
  const info = infoMatchForMetric(label)
  return (
    <Card>
      <CardBody>
        <div className="flex items-start justify-between gap-3">
          <p className="text-xs uppercase tracking-[0.16em] text-slate-500">{label}</p>
          {info ? <InfoButton content={info} /> : null}
        </div>
        <p className="mt-2 text-3xl font-semibold text-slate-900 dark:text-white">{value}</p>
        {subtitle && <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">{subtitle}</p>}
      </CardBody>
    </Card>
  )
}

function InfoButton({ content }: { content: InfoContent }) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: MouseEvent) {
      if (!containerRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }

    document.addEventListener('mousedown', handlePointerDown)
    return () => document.removeEventListener('mousedown', handlePointerDown)
  }, [open])

  return (
    <div ref={containerRef} className="relative shrink-0">
      <button
        type="button"
        aria-label={`About ${content.title}`}
        onClick={() => setOpen((value) => !value)}
        className={`flex h-7 w-7 items-center justify-center rounded-full border text-xs font-semibold transition ${
          open
            ? 'border-brand-500/50 bg-brand-500/10 text-brand-700 dark:text-brand-200'
            : 'border-slate-200 bg-white text-slate-500 hover:border-slate-300 hover:text-slate-900 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#8a8a93] dark:hover:text-white'
        }`}
      >
        i
      </button>
      {open ? (
        <div className="absolute right-0 top-10 z-40 w-[320px] rounded-[24px] border border-slate-200 bg-white/95 p-4 shadow-2xl backdrop-blur dark:border-[#1d1d23] dark:bg-[#09090d]/95">
          <p className="text-sm font-semibold text-slate-900 dark:text-white">{content.title}</p>
          <div className="mt-3 space-y-3 text-xs leading-6 text-slate-600 dark:text-[#b3b3bd]">
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">Why it matters</p>
              <p className="mt-1">{content.purpose}</p>
            </div>
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">How it is calculated</p>
              <p className="mt-1">{content.calculation}</p>
            </div>
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">Data source</p>
              <p className="mt-1">{content.source}</p>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  )
}

function Sparkline({ points }: { points: { bucket_ms: number; value: number }[] }) {
  if (points.length === 0) {
    return <span className="text-xs text-slate-400">No history</span>
  }

  return (
    <ResponsiveContainer width="100%" height={48}>
      <LineChart data={points}>
        <Line type="monotone" dataKey="value" stroke="#0069ED" strokeWidth={2} dot={false} />
        <Tooltip
          formatter={(value: unknown) => formatNumber(value as number, 3)}
          labelFormatter={(label: unknown) => formatBucket(Number(label))}
          contentStyle={CHART_TOOLTIP_STYLE}
          labelStyle={CHART_TOOLTIP_LABEL_STYLE}
          itemStyle={CHART_TOOLTIP_ITEM_STYLE}
          wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
        />
      </LineChart>
    </ResponsiveContainer>
  )
}

export function AnalyticsPage() {
  const { merchantId } = useMerchantStore()
  const navigate = useNavigate()
  const [scope, setScope] = useState<AnalyticsScope>('current')
  const [range, setRange] = useState<AnalyticsRange>('1h')
  const [section, setSection] = useState<Section>('overview')
  const [page, setPage] = useState(1)
  const [routingFilters, setRoutingFilters] = useState<RoutingFilters>(EMPTY_ROUTING_FILTERS)
  const pageSize = 10
  const globalConnectorOnly = scope === 'all'

  const canQueryCurrent = scope === 'all' || Boolean(merchantId)
  const effectiveMerchantId = scope === 'current' ? merchantId || undefined : undefined

  const overviewUrl = canQueryCurrent && !globalConnectorOnly
    ? buildAnalyticsUrl('/analytics/overview', scope, range, effectiveMerchantId)
    : null
  const scoresUrl = canQueryCurrent && !globalConnectorOnly
    ? buildAnalyticsUrl('/analytics/gateway-scores', scope, range, effectiveMerchantId)
    : null
  const decisionsUrl = canQueryCurrent && !globalConnectorOnly
    ? buildAnalyticsUrl('/analytics/decisions', scope, range, effectiveMerchantId)
    : null
  const routingUrl = canQueryCurrent
    ? buildAnalyticsUrl('/analytics/routing-stats', scope, range, effectiveMerchantId, 1, 10, {
      payment_method_type: scope === 'current' ? routingFilters.paymentMethodType || undefined : undefined,
      payment_method: scope === 'current' ? routingFilters.paymentMethod || undefined : undefined,
      gateway: routingFilters.gateways,
    })
    : null
  const logsUrl = canQueryCurrent && !globalConnectorOnly
    ? buildAnalyticsUrl('/analytics/log-summaries', scope, range, effectiveMerchantId, page, pageSize)
    : null

  const overview = useSWR<AnalyticsOverviewResponse>(overviewUrl, fetcher, {
    refreshInterval: 8000,
    revalidateOnFocus: true,
  })
  const scores = useSWR<AnalyticsGatewayScoresResponse>(scoresUrl, fetcher, {
    refreshInterval: 8000,
    revalidateOnFocus: true,
  })
  const decisions = useSWR<AnalyticsDecisionResponse>(decisionsUrl, fetcher, {
    refreshInterval: 8000,
    revalidateOnFocus: true,
  })
  const routing = useSWR<AnalyticsRoutingStatsResponse>(routingUrl, fetcher, {
    refreshInterval: 12000,
    revalidateOnFocus: true,
  })
  const logs = useSWR<AnalyticsLogSummariesResponse>(logsUrl, fetcher, {
    refreshInterval: 12000,
    revalidateOnFocus: true,
  })

  useEffect(() => {
    if (scope === 'all') {
      setSection('routing')
      setRoutingFilters((current) => ({
        ...current,
        paymentMethodType: '',
        paymentMethod: '',
      }))
    }
  }, [scope])

  useEffect(() => {
    const options: RoutingFilterOptions | undefined = routing.data?.available_filters
    if (!options) return

    setRoutingFilters((current) => {
      const nextPaymentMethodType = scope === 'current' && (current.paymentMethodType ? options.payment_method_types.includes(current.paymentMethodType) : true)
        ? current.paymentMethodType
        : ''

      const nextPaymentMethod = scope === 'current' && (current.paymentMethod ? options.payment_methods.includes(current.paymentMethod) : true)
        ? current.paymentMethod
        : ''

      const nextGateways = current.gateways.filter((gateway) => options.gateways.includes(gateway))

      if (
        nextPaymentMethodType === current.paymentMethodType &&
        nextPaymentMethod === current.paymentMethod &&
        nextGateways.length === current.gateways.length &&
        nextGateways.every((gateway, index) => gateway === current.gateways[index])
      ) {
        return current
      }

      return {
        paymentMethodType: nextPaymentMethodType,
        paymentMethod: nextPaymentMethod,
        gateways: nextGateways,
      }
    })
  }, [routing.data?.available_filters, scope])

  const loading = [overview, scores, decisions, routing, logs].some((item) => item.isLoading)
  const error = overview.error?.message || scores.error?.message || decisions.error?.message || routing.error?.message || logs.error?.message || null

  const scoreSeriesByKey = useMemo(() => {
    const grouped = new Map<string, { bucket_ms: number; value: number }[]>()
    for (const point of scores.data?.series || []) {
      const key = makeScoreKey(point)
      const entry = grouped.get(key) || []
      entry.push({ bucket_ms: point.bucket_ms, value: point.score_value })
      grouped.set(key, entry)
    }
    return grouped
  }, [scores.data])

  const decisionChartRows = useMemo(() => {
    const buckets = new Map<number, Record<string, number>>()
    for (const point of decisions.data?.series || []) {
      const row = buckets.get(point.bucket_ms) || { bucket_ms: point.bucket_ms }
      row[point.routing_approach] = point.count
      buckets.set(point.bucket_ms, row)
    }
    return Array.from(buckets.values()).sort((left, right) => left.bucket_ms - right.bucket_ms)
  }, [decisions.data])

  const routingChartRows = useMemo(() => {
    const buckets = new Map<number, Record<string, number>>()
    for (const point of routing.data?.gateway_share || []) {
      const row = buckets.get(point.bucket_ms) || { bucket_ms: point.bucket_ms }
      row[point.gateway] = point.count
      buckets.set(point.bucket_ms, row)
    }
    return Array.from(buckets.values()).sort((left, right) => left.bucket_ms - right.bucket_ms)
  }, [routing.data])

  const srTrendRows = useMemo(() => {
    const gateways = Array.from(new Set((routing.data?.sr_trend || []).map((point) => point.gateway))).slice(0, 5)
    const buckets = new Map<number, Record<string, number>>()
    for (const point of routing.data?.sr_trend || []) {
      if (!gateways.includes(point.gateway)) continue
      const row = buckets.get(point.bucket_ms) || { bucket_ms: point.bucket_ms }
      row[point.gateway] = toPercent(point.score_value)
      buckets.set(point.bucket_ms, row)
    }
    return {
      gateways,
      rows: Array.from(buckets.values()).sort((left, right) => left.bucket_ms - right.bucket_ms),
    }
  }, [routing.data])

  const connectorTrendSummary = useMemo(() => {
    if (!srTrendRows.rows.length) return []
    const latestRow = srTrendRows.rows[srTrendRows.rows.length - 1]
    return srTrendRows.gateways
      .map((gateway) => ({
        gateway,
        value: typeof latestRow[gateway] === 'number' ? latestRow[gateway] : null,
      }))
      .filter((item): item is { gateway: string; value: number } => item.value !== null)
  }, [srTrendRows])

  const availableRoutingFilters = routing.data?.available_filters || {
    payment_method_types: [],
    payment_methods: [],
    gateways: [],
  }

  const activeRoutingFilterBadges = useMemo(() => {
    const badges: string[] = []
    if (scope === 'current' && routingFilters.paymentMethodType) {
      badges.push(routingFilters.paymentMethodType)
    }
    if (scope === 'current' && routingFilters.paymentMethod) {
      badges.push(routingFilters.paymentMethod)
    }
    if (routingFilters.gateways.length) {
      badges.push(...routingFilters.gateways)
    }
    return badges
  }, [routingFilters, scope])

  const routingSubtitle = useMemo(() => {
    if (scope === 'all') {
      return 'Global success rate trend by connector across all merchants.'
    }
    if (!activeRoutingFilterBadges.length) {
      return 'Success rate trend by connector for the selected merchant.'
    }
    return `Success rate trend by connector filtered by ${activeRoutingFilterBadges.join(' / ')}.`
  }, [activeRoutingFilterBadges, scope])

  function updateRoutingFilter<K extends keyof RoutingFilters>(key: K, value: RoutingFilters[K]) {
    setRoutingFilters((current) => ({ ...current, [key]: value }))
  }

  function toggleGatewayFilter(gateway: string) {
    setRoutingFilters((current) => {
      const exists = current.gateways.includes(gateway)
      return {
        ...current,
        gateways: exists
          ? current.gateways.filter((value) => value !== gateway)
          : [...current.gateways, gateway],
      }
    })
  }

  function clearRoutingFilters() {
    setRoutingFilters(EMPTY_ROUTING_FILTERS)
  }

  function refreshAll() {
    overview.mutate()
    scores.mutate()
    decisions.mutate()
    routing.mutate()
    logs.mutate()
  }

  function openAudit(extraParams: Record<string, string | undefined>) {
    const search = queryString({
      scope,
      range,
      merchant_id: scope === 'current' ? effectiveMerchantId : undefined,
      ...extraParams,
    })
    navigate(search ? `/audit?${search}` : '/audit')
  }

  if (!canQueryCurrent) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Analytics</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8a8a93]">
            Set a Merchant ID in the top bar, or switch to the all-merchants view.
          </p>
        </div>
        <EmptyState
          title="Select a merchant to load analytics"
          body="The current-scope view is tied to the selected merchant. You can also use the all-merchants toggle for an operator-wide view."
        />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Analytics</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8a8a93]">
            Live routing metrics, score snapshots, rule hits, and operational summaries.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" variant={scope === 'current' ? 'primary' : 'secondary'} onClick={() => setScope('current')}>
            Merchant
          </Button>
          <Button size="sm" variant={scope === 'all' ? 'primary' : 'secondary'} onClick={() => setScope('all')}>
            All merchants
          </Button>
          <Button size="sm" variant="ghost" onClick={refreshAll}>
            Refresh
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {(globalConnectorOnly ? (['routing'] as Section[]) : (Object.keys(SECTION_LABELS) as Section[])).map((value) => (
          <Button
            key={value}
            size="sm"
            className={sectionButtonClass(section === value)}
            variant="secondary"
            onClick={() => setSection(value)}
          >
            {SECTION_LABELS[value]}
          </Button>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {RANGE_OPTIONS.map((value) => (
          <Button
            key={value}
            size="sm"
            className={sectionButtonClass(range === value)}
            variant="secondary"
            onClick={() => setRange(value)}
          >
            {value}
          </Button>
        ))}
        <Badge variant={scope === 'all' ? 'blue' : 'green'}>
          {scope === 'all' ? 'All merchants' : merchantId || 'Current merchant'}
        </Badge>
      </div>

      <ErrorMessage error={error} />

      {loading && (
        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
          <Spinner size={16} />
          Loading analytics…
        </div>
      )}

      {!globalConnectorOnly ? (
      <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {(overview.data?.kpis || [
          { label: 'Decisions', value: '0', subtitle: 'Waiting for data' },
          { label: 'Score snapshots', value: '0', subtitle: 'Waiting for data' },
          { label: 'Rule hits', value: '0', subtitle: 'Waiting for data' },
          { label: 'Errors', value: '0', subtitle: 'Waiting for data' },
        ]).map((kpi) => (
          <MetricCard key={kpi.label} label={kpi.label} value={kpi.value} subtitle={kpi.subtitle} />
        ))}
      </section>
      ) : (
      <Card>
        <CardBody className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <p className="text-sm font-semibold text-slate-900 dark:text-white">Global connector performance</p>
            <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
              All-merchants mode is restricted to connector-level success-rate summaries only.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="blue">Connector-only global view</Badge>
            <InfoButton content={CARD_INFO.connectorTrend} />
          </div>
        </CardBody>
      </Card>
      )}

      {section === 'overview' && (
        <div className="grid gap-4 xl:grid-cols-2">
          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Top score snapshots</h2>
                <InfoButton content={CARD_INFO.topScores} />
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              {overview.data?.top_scores?.length ? overview.data.top_scores.slice(0, 5).map((snapshot) => {
                const key = [snapshot.merchant_id, snapshot.payment_method_type, snapshot.payment_method, snapshot.gateway].join('|')
                const spark = scoreSeriesByKey.get(key) || []
                return (
                  <div key={key} className="rounded-2xl border border-slate-200 dark:border-[#1d1d23] p-4">
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div>
                        <p className="text-sm font-semibold text-slate-900 dark:text-white">{snapshot.gateway}</p>
                        <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                          {snapshot.merchant_id} · {snapshot.payment_method_type} · {snapshot.payment_method}
                        </p>
                      </div>
                      <Badge variant="blue">{formatNumber(snapshot.score_value, 3)}</Badge>
                    </div>
                    <div className="mt-3 grid grid-cols-2 gap-3 text-xs text-slate-500 dark:text-[#8a8a93] md:grid-cols-4">
                      <span>sigma {formatNumber(snapshot.sigma_factor, 3)}</span>
                      <span>avg {formatNumber(snapshot.average_latency, 2)}</span>
                      <span>tp99 {formatNumber(snapshot.tp99_latency, 2)}</span>
                      <span>count {formatNumber(snapshot.transaction_count, 0)}</span>
                    </div>
                    <div className="mt-3">
                      <Sparkline points={spark} />
                    </div>
                  </div>
                )
              }) : (
                <EmptyState title="No score snapshots yet" body="Score events will appear here after the first routing or score update cycle." />
              )}
            </CardBody>
          </Card>

          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Recent errors</h2>
                <InfoButton content={CARD_INFO.recentErrors} />
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              {overview.data?.top_errors?.length ? overview.data.top_errors.slice(0, 5).map((errorRow) => (
                <button
                  key={`${errorRow.route}-${errorRow.error_code}-${errorRow.error_message}`}
                  type="button"
                  onClick={() => openAudit({
                    route: errorRow.route,
                    error_code: errorRow.error_code,
                    event_type: 'error',
                    status: 'failure',
                  })}
                  className="w-full rounded-2xl border border-slate-200 p-4 text-left transition hover:border-slate-300 dark:border-[#1d1d23] dark:hover:border-[#2a2a33]"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">{errorRow.error_code}</p>
                      <p className="text-xs text-slate-500 dark:text-[#8a8a93]">{humanizeAuditRoute(errorRow.route)}</p>
                    </div>
                    <Badge variant="red">{errorRow.count}</Badge>
                  </div>
                  <p className="mt-2 text-xs text-slate-500 dark:text-[#8a8a93]">{errorRow.error_message}</p>
                  <p className="mt-2 text-[11px] text-slate-400 dark:text-[#66666e]">
                    Last seen {formatDateTime(errorRow.last_seen_ms)}
                  </p>
                </button>
              )) : (
                <EmptyState title="No errors captured" body="Structured failure summaries will appear here when requests fail." />
              )}
            </CardBody>
          </Card>
        </div>
      )}

      {section === 'scores' && (
        <Card>
          <CardHeader>
            <div className="flex items-start justify-between gap-3">
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Gateway scoring</h2>
              <InfoButton content={CARD_INFO.gatewayScoring} />
            </div>
          </CardHeader>
          <CardBody>
            {scores.data?.snapshots?.length ? (
              <div className="space-y-4">
                <div className="overflow-x-auto">
                  <table className="min-w-full text-left text-sm">
                    <thead className="text-xs uppercase tracking-wide text-slate-500 dark:text-[#8a8a93]">
                      <tr>
                        <th className="py-3 pr-4">Merchant</th>
                        <th className="py-3 pr-4">PMT</th>
                        <th className="py-3 pr-4">Gateway</th>
                        <th className="py-3 pr-4">Score</th>
                        <th className="py-3 pr-4">Sigma</th>
                        <th className="py-3 pr-4">Avg latency</th>
                        <th className="py-3 pr-4">TP99</th>
                        <th className="py-3 pr-4">Updated</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-200 dark:divide-[#1d1d23]">
                      {scores.data.snapshots.map((snapshot) => {
                        const key = [snapshot.merchant_id, snapshot.payment_method_type, snapshot.payment_method, snapshot.gateway].join('|')
                        const spark = scoreSeriesByKey.get(key) || []
                        return (
                          <tr key={key} className="align-top">
                            <td className="py-3 pr-4 font-medium text-slate-900 dark:text-white">{snapshot.merchant_id}</td>
                            <td className="py-3 pr-4 text-slate-500 dark:text-[#8a8a93]">{snapshot.payment_method_type}</td>
                            <td className="py-3 pr-4 font-medium text-slate-900 dark:text-white">{snapshot.gateway}</td>
                            <td className="py-3 pr-4"><Badge variant="blue">{formatNumber(snapshot.score_value, 3)}</Badge></td>
                            <td className="py-3 pr-4 text-slate-600 dark:text-[#a6a6b0]">{formatNumber(snapshot.sigma_factor, 3)}</td>
                            <td className="py-3 pr-4 text-slate-600 dark:text-[#a6a6b0]">{formatNumber(snapshot.average_latency, 2)}</td>
                            <td className="py-3 pr-4 text-slate-600 dark:text-[#a6a6b0]">{formatNumber(snapshot.tp99_latency, 2)}</td>
                            <td className="py-3 pr-4 text-slate-500 dark:text-[#8a8a93]">
                              <div className="w-40">
                                <Sparkline points={spark} />
                              </div>
                              <div className="mt-1 text-[11px]">{formatDateTime(snapshot.last_updated_ms)}</div>
                            </td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            ) : (
              <EmptyState title="No gateway scores yet" body="Once score updates are recorded, the live snapshot table and sparklines will populate here." />
            )}
          </CardBody>
        </Card>
      )}

      {section === 'decisions' && (
        <div className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            {decisions.data?.tiles?.map((tile) => (
              <MetricCard key={tile.label} label={tile.label} value={tile.value} subtitle={tile.subtitle} />
            )) || null}
          </div>

          <Card className="overflow-visible">
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Decision throughput by routing approach</h2>
                <InfoButton content={CARD_INFO.decisionThroughput} />
              </div>
            </CardHeader>
            <CardBody>
              {decisionChartRows.length ? (
                <div className="h-80">
                  <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={decisionChartRows}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                      <XAxis dataKey="bucket_ms" tickFormatter={formatBucket} tick={{ fontSize: 11 }} />
                      <YAxis tick={{ fontSize: 11 }} />
                      <Tooltip
                        labelFormatter={(label) => formatDateTime(Number(label))}
                        contentStyle={CHART_TOOLTIP_STYLE}
                        labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                        itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                        wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                      />
                      <Legend />
                      {(decisions.data?.approaches || []).slice(0, 5).map((approach, index) => (
                        <Area
                          key={approach.rule_name}
                          type="monotone"
                          dataKey={approach.rule_name}
                          stackId="1"
                          stroke={['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][index % 5]}
                          fill={['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][index % 5]}
                          fillOpacity={0.25}
                        />
                      ))}
                    </AreaChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <EmptyState title="No decision history yet" body="Decision events will populate this chart as routing traffic flows through the service." />
              )}
            </CardBody>
          </Card>
        </div>
      )}

      {section === 'routing' && (
        <div className="grid gap-4 xl:grid-cols-2">
          {!globalConnectorOnly ? (
          <>
          <Card className="overflow-visible">
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Gateway share over time</h2>
                <InfoButton content={CARD_INFO.gatewayShare} />
              </div>
            </CardHeader>
            <CardBody>
              {routingChartRows.length ? (
                <div className="h-80">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={routingChartRows}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                      <XAxis dataKey="bucket_ms" tickFormatter={formatBucket} tick={{ fontSize: 11 }} />
                      <YAxis tick={{ fontSize: 11 }} />
                      <Tooltip
                        labelFormatter={(label) => formatDateTime(Number(label))}
                        contentStyle={CHART_TOOLTIP_STYLE}
                        labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                        itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                        wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                      />
                      <Legend />
                      {(routing.data?.gateway_share || []).reduce<string[]>((acc, point) => {
                        if (!acc.includes(point.gateway)) acc.push(point.gateway)
                        return acc
                      }, []).slice(0, 5).map((gateway, index) => (
                        <Bar key={gateway} dataKey={gateway} stackId="1" fill={['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][index % 5]} />
                      ))}
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <EmptyState title="No routing share data" body="Decision events will drive this chart once traffic is flowing." />
              )}
            </CardBody>
          </Card>

          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Top priority logic hits</h2>
                <InfoButton content={CARD_INFO.topRules} />
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              {routing.data?.top_rules?.length ? routing.data.top_rules.map((rule) => (
                <div key={rule.rule_name} className="flex items-center justify-between rounded-2xl border border-slate-200 dark:border-[#1d1d23] px-4 py-3">
                  <span className="text-sm font-medium text-slate-900 dark:text-white">{rule.rule_name}</span>
                  <Badge variant="purple">{rule.count}</Badge>
                </div>
              )) : (
                <EmptyState title="No rule hits yet" body="Priority-logic hits will show here once routing is exercised." />
              )}
            </CardBody>
          </Card>
          </>
          ) : null}

          <Card className={`${globalConnectorOnly ? 'xl:col-span-2' : 'xl:col-span-2'} overflow-visible`}>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                    {globalConnectorOnly ? 'Global connector success rate' : 'Connector success rate over time'}
                  </h2>
                  <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                    {routingSubtitle}
                  </p>
                </div>
                <InfoButton content={CARD_INFO.connectorTrend} />
              </div>
            </CardHeader>
            <CardBody>
              <div className="mb-5 rounded-[24px] border border-slate-200 bg-slate-50/80 p-4 dark:border-[#1d1d23] dark:bg-[#0c0c0e]">
                <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
                  <div className="grid gap-3 md:grid-cols-2">
                    <label className="space-y-2">
                      <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                        Payment method type
                      </span>
                      <select
                        value={scope === 'current' ? routingFilters.paymentMethodType : ''}
                        onChange={(event) => updateRoutingFilter('paymentMethodType', event.target.value)}
                        disabled={scope === 'all' || !availableRoutingFilters.payment_method_types.length}
                        className={selectClassName(scope === 'all' || !availableRoutingFilters.payment_method_types.length)}
                      >
                        <option value="">All payment method types</option>
                        {availableRoutingFilters.payment_method_types.map((value) => (
                          <option key={value} value={value}>{value}</option>
                        ))}
                      </select>
                    </label>

                    <label className="space-y-2">
                      <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                        Payment method
                      </span>
                      <select
                        value={scope === 'current' ? routingFilters.paymentMethod : ''}
                        onChange={(event) => updateRoutingFilter('paymentMethod', event.target.value)}
                        disabled={scope === 'all' || !availableRoutingFilters.payment_methods.length}
                        className={selectClassName(scope === 'all' || !availableRoutingFilters.payment_methods.length)}
                      >
                        <option value="">All payment methods</option>
                        {availableRoutingFilters.payment_methods.map((value) => (
                          <option key={value} value={value}>{value}</option>
                        ))}
                      </select>
                    </label>
                  </div>

                  <div className="space-y-2">
                    <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                      Connectors
                    </span>
                    <div className="flex min-h-10 flex-wrap gap-2 rounded-2xl border border-slate-200 bg-white p-2 dark:border-[#27272a] dark:bg-[#121214]">
                      {availableRoutingFilters.gateways.length ? availableRoutingFilters.gateways.map((gateway) => (
                        <button
                          key={gateway}
                          type="button"
                          onClick={() => toggleGatewayFilter(gateway)}
                          className={`rounded-full border px-3 py-1 text-xs font-semibold transition ${filterBadgeClass(routingFilters.gateways.includes(gateway))}`}
                        >
                          {gateway}
                        </button>
                      )) : (
                        <span className="px-2 py-1 text-xs text-slate-500 dark:text-[#8a8a93]">No connector options in this window</span>
                      )}
                    </div>
                  </div>

                  <div className="flex items-end">
                    <Button size="sm" variant="secondary" onClick={clearRoutingFilters}>
                      Clear filters
                    </Button>
                  </div>
                </div>

                <div className="mt-3 flex flex-wrap gap-2">
                  {activeRoutingFilterBadges.length ? activeRoutingFilterBadges.map((value) => (
                    <Badge key={value} variant="blue">{value}</Badge>
                  )) : (
                    <Badge variant="gray">No narrowing filters</Badge>
                  )}
                </div>

                {scope === 'all' ? (
                  <p className="mt-3 text-xs text-slate-500 dark:text-[#8a8a93]">
                    Global mode shows only connector-wise score badges and connector-wise success-rate trends. Merchant-level metrics, logs, and audits are hidden.
                  </p>
                ) : null}
              </div>

              {srTrendRows.rows.length ? (
                <div className="space-y-4">
                  {connectorTrendSummary.length ? (
                    <div className="flex flex-wrap gap-2">
                      {connectorTrendSummary.map((item) => (
                        <Badge key={item.gateway} variant="blue">
                          {item.gateway}: {formatPercent(item.value)}
                        </Badge>
                      ))}
                    </div>
                  ) : null}

                  <div className="h-80">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={srTrendRows.rows}>
                        <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                        <XAxis dataKey="bucket_ms" tickFormatter={formatBucket} tick={{ fontSize: 11 }} />
                        <YAxis tick={{ fontSize: 11 }} tickFormatter={(value) => `${formatNumber(Number(value), 0)}%`} />
                        <Tooltip
                          labelFormatter={(label) => formatDateTime(Number(label))}
                          formatter={(value: unknown, name: string | number) => [formatPercent(value as number), String(name)]}
                          contentStyle={CHART_TOOLTIP_STYLE}
                          labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                          itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                          wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                        />
                        <Legend />
                        {srTrendRows.gateways.map((gateway, index) => (
                          <Line
                            key={gateway}
                            type="monotone"
                            dataKey={gateway}
                            stroke={['#0069ED', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'][index % 5]}
                            strokeWidth={2}
                            dot={false}
                            name={gateway}
                          />
                        ))}
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                </div>
              ) : (
                <EmptyState title="No connector score history for this filter set yet" body="Generate traffic or widen the time range to populate the connector success-rate trend." />
              )}
            </CardBody>
          </Card>
        </div>
      )}

      {section === 'logs' && (
        <div className="grid gap-4 xl:grid-cols-2">
          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Error summaries</h2>
                <InfoButton content={CARD_INFO.errorSummaries} />
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              {logs.data?.errors?.length ? logs.data.errors.map((item) => (
                <button
                  key={`${item.route}-${item.error_code}-${item.error_message}`}
                  type="button"
                  onClick={() => openAudit({
                    route: item.route,
                    error_code: item.error_code,
                    event_type: 'error',
                    status: 'failure',
                  })}
                  className="w-full rounded-2xl border border-slate-200 px-4 py-3 text-left transition hover:border-slate-300 dark:border-[#1d1d23] dark:hover:border-[#2a2a33]"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">{item.error_code}</p>
                      <p className="text-xs text-slate-500 dark:text-[#8a8a93]">{humanizeAuditRoute(item.route)}</p>
                    </div>
                    <Badge variant="red">{item.count}</Badge>
                  </div>
                  <p className="mt-2 text-xs text-slate-500 dark:text-[#8a8a93]">{item.error_message}</p>
                </button>
              )) : (
                <EmptyState title="No log summaries yet" body="Structured error summaries will appear once requests fail." />
              )}
            </CardBody>
          </Card>

          <Card>
            <CardHeader>
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-3">
                  <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Recent samples</h2>
                  <InfoButton content={CARD_INFO.recentSamples} />
                </div>
                <div className="flex items-center gap-2">
                  <Button size="sm" variant="secondary" onClick={() => setPage((value) => Math.max(1, value - 1))} disabled={page <= 1}>
                    Prev
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => setPage((value) => value + 1)}>
                    Next
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardBody className="space-y-3">
              {logs.data?.samples?.length ? logs.data.samples.map((sample) => (
                <button
                  key={`${sample.route}-${sample.created_at_ms}-${sample.error_code || 'ok'}`}
                  type="button"
                  onClick={() => openAudit({
                    payment_id: sample.payment_id || undefined,
                    request_id: sample.request_id || undefined,
                    gateway: sample.gateway || undefined,
                    route: sample.route,
                    status: sample.status || undefined,
                    event_type: sample.event_type || undefined,
                    error_code: sample.error_code || undefined,
                  })}
                  className="w-full rounded-2xl border border-slate-200 px-4 py-3 text-left transition hover:border-slate-300 dark:border-[#1d1d23] dark:hover:border-[#2a2a33]"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <p className="text-sm font-semibold text-slate-900 dark:text-white">{humanizeAuditRoute(sample.route)}</p>
                      <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                        {sample.merchant_id || 'all merchants'} · {formatDateTime(sample.created_at_ms)}
                      </p>
                    </div>
                    {sample.error_code ? <Badge variant="red">{sample.error_code}</Badge> : <Badge variant="green">{sample.status || 'ok'}</Badge>}
                  </div>
                  <p className="mt-2 text-xs text-slate-500 dark:text-[#8a8a93]">
                    {sample.error_message || sample.routing_approach || sample.gateway || 'No additional detail'}
                  </p>
                </button>
              )) : (
                <EmptyState title="No samples yet" body="Recent structured samples will appear here after the first failures or snapshots." />
              )}
            </CardBody>
          </Card>
        </div>
      )}
    </div>
  )
}
