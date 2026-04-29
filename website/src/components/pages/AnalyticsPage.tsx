import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useLocation } from 'react-router-dom'
import useSWR from 'swr'
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  Line,
  LineChart,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { fetcher } from '../../lib/api'
import {
  AnalyticsOverviewResponse,
  AnalyticsRange,
  AnalyticsRangeValue,
  AnalyticsRoutingStatsResponse,
  PaymentAuditResponse,
  RoutingFilterOptions,
} from '../../types/api'
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { DateTimePicker } from '../ui/DateTimePicker'

type TimeWindow = {
  start_ms: number
  end_ms: number
}

type RoutingFilters = {
  dimensions: Record<string, string>
  gateways: string[]
}

type AnalyticsView = 'transactions' | 'rule_based'
const ANALYTICS_VIEW_LABELS: Record<AnalyticsView, string> = {
  transactions: 'Auth-rate based',
  rule_based: 'Rule based / Volume based',
}

type PreviewTraceKey = readonly [
  'preview-trace-analytics',
  AnalyticsRangeValue,
  number | null,
  number | null,
]

type InfoContent = {
  title: string
  purpose: string
  calculation: string
  source: string
}
type BadgeVariant = 'green' | 'gray' | 'blue' | 'red' | 'orange' | 'purple'
type GatewayVolumeSummaryItem = {
  gateway: string
  count: number
  share: number
}
type SrGatewaySummaryItem = {
  gateway: string
  value: number
}
type RoutingAlignmentSummary = {
  srLeader: SrGatewaySummaryItem | null
  srRunnerUp: SrGatewaySummaryItem | null
  volumeLeader: GatewayVolumeSummaryItem | null
  srLeaderVolume: GatewayVolumeSummaryItem | null
  alignmentPercent: number | null
  leaderDecisionCount: number
  comparableDecisionCount: number
  headline: string
  detail: string
}
type ConnectorComparisonRow = {
  gateway: string
  srValue: number | null
  count: number
  share: number
  color: string
  isSrLeader: boolean
  isVolumeLeader: boolean
}

const PRESET_OPTIONS: { value: AnalyticsRangeValue; label: string }[] = [
  { value: '15m', label: 'Last 15 mins' },
  { value: '1h', label: 'Last 1 hour' },
  { value: '12h', label: 'Last 12 hours' },
  { value: '1d', label: 'Last 1 day' },
  { value: '1w', label: 'Last 1 week' },
  { value: 'custom', label: 'Custom window' },
]

const CHART_COLORS = ['#0069ED', '#14b8a6', '#f97316', '#e11d48', '#8b5cf6', '#22c55e']
const CHART_TOOLTIP_STYLE = {
  backgroundColor: '#0d0d12',
  border: '1px solid #1c1c24',
  borderRadius: '14px',
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
  dimensions: {},
  gateways: [],
}
const MAX_VISIBLE_DIMENSIONS = 3
const PREVIEW_TRACE_PAGE_SIZE = 50
const MAX_PREVIEW_TRACE_PAGES = 5
const PREVIEW_LIST_PAGE_SIZE = 10
const CATCH_UP_REFRESH_DELAYS_MS = [750, 2000, 4000]
const CARD_INFO: Record<'hits' | 'share' | 'alignment' | 'sr' | 'preview_hits' | 'preview_activity' | 'preview_share', InfoContent> = {
  hits: {
    title: 'API call counts',
    purpose: 'Use these cards to see how much traffic each major decision-engine API handled in the selected window.',
    calculation: 'Each request records one lightweight API-call event. The cards count those recorded calls for the endpoints surfaced in the current view.',
    source: 'Counts come from ClickHouse-backed API analytics rows ingested from Kafka into `analytics_api_events`.',
  },
  share: {
    title: 'Selected gateways over time',
    purpose: 'Use this to see when traffic shifted from one connector to another for the selected merchant and routing slice.',
    calculation: 'Decision events are grouped by time and chosen connector. The chart shows how many filtered decisions each gateway received over time.',
    source: 'Reads ClickHouse-backed domain analytics rows from `analytics_domain_events`.',
  },
  alignment: {
    title: 'Routing alignment',
    purpose: 'Use this to see whether the best-scoring connector is also leading traffic share.',
    calculation: 'The page compares the latest connector success-rate score with the gateways actually selected in the same time window.',
    source: 'Reads the same ClickHouse-backed routing-stats response used by the gateway share and connector success-rate charts.',
  },
  sr: {
    title: 'Connector success rate over time',
    purpose: 'Use this to explain why a connector won routing at a given time, based on the recorded historical score trail.',
    calculation: 'Stored `score_snapshot` events are grouped over the selected window and averaged per connector. The line values are displayed as percentages.',
    source: 'Reads ClickHouse-backed `score_snapshot` analytics rows from `analytics_domain_events`. The current score state still originates from Redis-backed scoring flows.',
  },
  preview_hits: {
    title: 'Rule-based summary',
    purpose: 'Use these cards to distinguish rule decision volume from the connector coverage produced by rule-based routing.',
    calculation: 'Rule Evaluate counts come from request-hit analytics for `/routing/evaluate`. Gateway coverage counts the unique connectors selected in the fetched decision sample.',
    source: 'Reads request-hit and rule decision analytics associated with rule-based routing activity.',
  },
  preview_activity: {
    title: 'Connector selections over time',
    purpose: 'Use this to see which connectors were selected over the chosen decision window.',
    calculation: 'Returned decision traces are grouped by time using each trace\'s latest activity timestamp, then grouped by latest selected connector. The chart shows connector counts over time.',
    source: 'Reads rule decision activity through `/analytics/preview-trace`.',
  },
  preview_share: {
    title: 'Rule-based gateway selection mix',
    purpose: 'Use this to see which connectors dominate the fetched rule decision sample, separate from auth-rate transaction decisions.',
    calculation: 'Returned decision traces are grouped by latest selected connector and displayed as share of the fetched decision sample.',
    source: 'Reads rule decision activity through `/analytics/preview-trace`.',
  },
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

function buildAnalyticsUrl(
  path: string,
  range: AnalyticsRangeValue,
  customWindow?: TimeWindow,
  routingFilters?: RoutingFilters,
) {
  const params: Record<string, string | number | undefined> = {
    range: range === 'custom' ? '1h' : range,
    start_ms: customWindow?.start_ms,
    end_ms: customWindow?.end_ms,
    gateway: routingFilters?.gateways.length ? routingFilters.gateways.join(',') : undefined,
  }

  Object.entries(routingFilters?.dimensions || {}).forEach(([key, value]) => {
    if (value) {
      params[key] = value
    }
  })

  const qs = queryString(params)
  return qs ? `${path}?${qs}` : path
}

function buildPreviewTraceUrl(
  range: AnalyticsRangeValue,
  page: number,
  pageSize: number,
  customWindow?: TimeWindow,
) {
  const params: Record<string, string | number | undefined> = {
    range: range === 'custom' ? '1h' : range,
    start_ms: customWindow?.start_ms,
    end_ms: customWindow?.end_ms,
    page,
    page_size: pageSize,
  }

  const qs = queryString(params)
  return qs ? `/analytics/preview-trace?${qs}` : '/analytics/preview-trace'
}

async function loadPreviewTraceSample(
  range: AnalyticsRangeValue,
  customWindow?: TimeWindow,
) {
  const firstPage = await fetcher<PaymentAuditResponse>(
    buildPreviewTraceUrl(range, 1, PREVIEW_TRACE_PAGE_SIZE, customWindow),
  )
  const totalPages = Math.min(
    Math.ceil(firstPage.total_results / PREVIEW_TRACE_PAGE_SIZE),
    MAX_PREVIEW_TRACE_PAGES,
  )

  if (totalPages <= 1) {
    return firstPage
  }

  const remainingPages = await Promise.all(
    Array.from({ length: totalPages - 1 }, (_, index) =>
      fetcher<PaymentAuditResponse>(
        buildPreviewTraceUrl(
          range,
          index + 2,
          PREVIEW_TRACE_PAGE_SIZE,
          customWindow,
        ),
      ),
    ),
  )

  return {
    ...firstPage,
    results: [firstPage.results, ...remainingPages.map((page) => page.results)].flat(),
  }
}

function formatNumber(value: number | string | undefined, digits = 2) {
  if (value === undefined || value === null || Number.isNaN(Number(value))) {
    return '0'
  }
  const numericValue = Number(value)
  if (Number.isInteger(numericValue)) return numericValue.toString()
  return numericValue.toFixed(digits)
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

function formatPercentPointDelta(value: number | undefined, digits = 1) {
  if (value === undefined || !Number.isFinite(value)) return 'No runner-up'
  const sign = value > 0 ? '+' : value < 0 ? '-' : ''
  return `${sign}${formatNumber(Math.abs(value), digits)} pp`
}

function readChartValue(row: Record<string, number | null>, key: string) {
  const value = row[key]
  return typeof value === 'number' && Number.isFinite(value) ? value : 0
}

function formatBucketLabel(ms: number, window: TimeWindow) {
  const duration = Math.max(0, window.end_ms - window.start_ms)

  if (duration <= 24 * 60 * 60 * 1000) {
    return new Intl.DateTimeFormat(undefined, {
      hour: '2-digit',
      minute: '2-digit',
    }).format(new Date(ms))
  }

  if (duration <= 7 * 24 * 60 * 60 * 1000) {
    return new Intl.DateTimeFormat(undefined, {
      day: 'numeric',
      month: 'short',
      hour: '2-digit',
    }).format(new Date(ms))
  }

  return new Intl.DateTimeFormat(undefined, {
    day: 'numeric',
    month: 'short',
  }).format(new Date(ms))
}

function formatDateTime(ms: number) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(ms))
}

function bucketSizeForWindow(range: AnalyticsRangeValue, customWindow?: TimeWindow) {
  const windowMs = customWindow
    ? customWindow.end_ms - customWindow.start_ms
    : range === '15m'
      ? 15 * 60 * 1000
      : range === '1h'
        ? 60 * 60 * 1000
        : range === '12h'
          ? 12 * 60 * 60 * 1000
          : range === '1d'
            ? 24 * 60 * 60 * 1000
            : 7 * 24 * 60 * 60 * 1000

  if (windowMs <= 15 * 60 * 1000) return 60 * 1000
  if (windowMs <= 60 * 60 * 1000) return 5 * 60 * 1000
  if (windowMs <= 12 * 60 * 60 * 1000) return 60 * 60 * 1000
  if (windowMs <= 24 * 60 * 60 * 1000) return 60 * 60 * 1000
  return 24 * 60 * 60 * 1000
}

function bucketTimestamp(ms: number, bucketSize: number) {
  return ms - (ms % Math.max(1, bucketSize))
}

function sortedGateways(values: string[]) {
  return Array.from(new Set(values)).sort((left, right) => left.localeCompare(right)).slice(0, 6)
}

function buildBucketTimeline(window: TimeWindow, bucketSize: number) {
  const buckets: number[] = []
  const safeBucketSize = Math.max(1, bucketSize)
  const startBucket = bucketTimestamp(window.start_ms, safeBucketSize)
  const endBucket = bucketTimestamp(window.end_ms, safeBucketSize)

  for (let bucket = startBucket; bucket <= endBucket; bucket += safeBucketSize) {
    buckets.push(bucket)
  }

  return buckets
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

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-[24px] border border-dashed border-slate-200 bg-white/60 px-6 py-12 text-center dark:border-[#222227] dark:bg-[#0b0b0d]">
      <p className="text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#8a8a93]">{body}</p>
    </div>
  )
}

function PendingState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-[24px] border border-slate-200 bg-white/60 px-6 py-12 text-center dark:border-[#222227] dark:bg-[#0b0b0d]">
      <div className="flex justify-center">
        <Spinner size={20} />
      </div>
      <p className="mt-4 text-sm font-semibold text-slate-900 dark:text-white">{title}</p>
      <p className="mt-2 text-sm text-slate-500 dark:text-[#8a8a93]">{body}</p>
    </div>
  )
}

function controlClassName() {
  return 'h-11 w-full rounded-2xl border border-slate-200 bg-white px-4 text-sm text-slate-700 shadow-sm outline-none transition focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#e5e7eb]'
}

function sectionButtonClass(active: boolean) {
  return active
    ? '!border-brand-500/70 !bg-white !text-slate-950 shadow-[0_14px_30px_-24px_rgba(59,130,246,0.55)] ring-2 ring-brand-500/55 dark:!border-brand-500/70 dark:!bg-[#161b24] dark:!text-white dark:ring-brand-500/55'
    : '!border-transparent !bg-slate-100 !text-slate-600 hover:!bg-slate-200 hover:!text-slate-900 dark:!bg-[#161b24] dark:!text-[#a7b2c6] dark:hover:!bg-[#1c2330] dark:hover:!text-white'
}

function InfoButton({ content }: { content: InfoContent }) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [position, setPosition] = useState<{ top: number; left: number; width: number }>({
    top: 0,
    left: 0,
    width: 320,
  })

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

  useLayoutEffect(() => {
    if (!open || !containerRef.current) return

    const POPOVER_WIDTH = 320
    const POPOVER_HEIGHT = 280
    const VIEWPORT_GUTTER = 16
    const GAP = 12

    function updatePosition() {
      if (!containerRef.current) return

      const rect = containerRef.current.getBoundingClientRect()
      const width = Math.min(POPOVER_WIDTH, window.innerWidth - VIEWPORT_GUTTER * 2)
      const left = Math.min(
        Math.max(rect.right - width, VIEWPORT_GUTTER),
        window.innerWidth - width - VIEWPORT_GUTTER,
      )
      const showAbove = rect.bottom + GAP + POPOVER_HEIGHT > window.innerHeight - VIEWPORT_GUTTER
      const top = showAbove
        ? Math.max(rect.top - POPOVER_HEIGHT - GAP, VIEWPORT_GUTTER)
        : rect.bottom + GAP

      setPosition({ top, left, width })
    }

    updatePosition()
    window.addEventListener('resize', updatePosition)
    window.addEventListener('scroll', updatePosition, true)

    return () => {
      window.removeEventListener('resize', updatePosition)
      window.removeEventListener('scroll', updatePosition, true)
    }
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
        <div
          style={{
            position: 'fixed',
            top: position.top,
            left: position.left,
            width: position.width,
          }}
          className="z-[120] rounded-[24px] border border-slate-200 bg-white/95 p-4 shadow-2xl backdrop-blur dark:border-[#1d1d23] dark:bg-[#09090d]/95"
        >
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

function HitsCard({
  label,
  value,
  subtitle,
  eyebrow = 'Endpoint hits',
}: {
  label: string
  value: number
  subtitle: string
  eyebrow?: string
}) {
  return (
    <Card className="h-full overflow-hidden">
      <CardBody className="flex h-full min-h-[150px] flex-col justify-between">
        <div className="space-y-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
            {eyebrow}
          </p>
          <p className="text-lg font-semibold text-slate-900 dark:text-white">{label}</p>
        </div>
        <div className="flex items-end justify-between gap-4">
          <p className="text-5xl font-semibold tracking-tight text-slate-950 dark:text-white">
            {formatNumber(value, 0)}
          </p>
          <Badge variant="blue">{subtitle}</Badge>
        </div>
      </CardBody>
    </Card>
  )
}

function RoutingAlignmentCard({
  summary,
  comparisonRows,
  expanded,
  onToggle,
}: {
  summary: RoutingAlignmentSummary
  comparisonRows: ConnectorComparisonRow[]
  expanded: boolean
  onToggle: () => void
}) {
  const srMargin =
    summary.srLeader && summary.srRunnerUp
      ? summary.srLeader.value - summary.srRunnerUp.value
      : undefined
  const srLeaderVolumeText = summary.srLeaderVolume
    ? `${formatNumber(summary.srLeaderVolume.count, 0)} payments for ${summary.srLeaderVolume.gateway}`
    : 'No traffic share'
  const alignedVolumeText = summary.comparableDecisionCount
    ? `${formatNumber(summary.leaderDecisionCount, 0)} of ${formatNumber(summary.comparableDecisionCount, 0)} payments matched the best score`
    : srLeaderVolumeText
  const alignmentText =
    summary.alignmentPercent === null
      ? 'Not enough data'
      : `${formatPercent(summary.alignmentPercent)} traffic share`
  const srLeaderName = summary.srLeader?.gateway || '--'
  const volumeLeaderName = summary.volumeLeader?.gateway || '--'
  const leadersDiffer =
    Boolean(summary.srLeader && summary.volumeLeader) &&
    summary.srLeader?.gateway !== summary.volumeLeader?.gateway
  const volumeBadgeVariant: BadgeVariant = !summary.volumeLeader
    ? 'gray'
    : leadersDiffer
      ? 'orange'
      : 'green'
  const collapsedReadout = [
    summary.srLeader ? `Best score: ${srLeaderName}` : 'No score',
    summary.volumeLeader ? `Traffic leader: ${volumeLeaderName}` : 'No traffic',
    alignmentText,
  ].join(' · ')

  return (
    <Card className="overflow-visible">
      <CardHeader className={expanded ? 'px-5 py-4' : 'px-5 py-3 !border-b-0'}>
        <div className={`flex flex-wrap justify-between gap-3 ${expanded ? 'items-start' : 'items-center'}`}>
          <div className={expanded ? 'min-w-0' : 'flex min-w-0 flex-wrap items-center gap-2'}>
            <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
              Routing alignment
            </h2>
            {expanded ? (
              <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                Traffic share: checks if the best-scoring connector is also getting the largest share.
              </p>
            ) : (
              <span className="min-w-0 truncate text-xs font-medium text-slate-500 dark:text-[#9aa7bb]">
                {collapsedReadout}
              </span>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <InfoButton content={CARD_INFO.alignment} />
            <Button size="sm" variant="secondary" onClick={onToggle}>
              {expanded ? 'Hide details' : 'Show details'}
            </Button>
          </div>
        </div>
      </CardHeader>
      {expanded ? (
        <CardBody className="space-y-4 px-5 py-4">
          <div className="rounded-2xl border border-slate-200 bg-slate-50/80 px-4 py-3 dark:border-[#2a303a] dark:bg-[#0d1118]">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div>
                <div className="flex flex-wrap items-center gap-2">
                  <Badge variant="gray">Best score: {srLeaderName}</Badge>
                  <Badge variant={volumeBadgeVariant}>
                    Traffic leader: {volumeLeaderName}
                  </Badge>
                </div>
                <p className="mt-3 text-sm font-semibold text-slate-900 dark:text-white">
                  {summary.headline}
                </p>
                <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#8a8a93]">
                  {summary.detail}
                </p>
              </div>
              <p className="text-right text-xs font-medium text-slate-500 dark:text-[#8a8a93]">
                {alignmentText}
              </p>
            </div>
          </div>

          <div className="grid overflow-hidden rounded-2xl border border-slate-200 bg-white/70 dark:border-[#2a303a] dark:bg-[#0c0f15] lg:grid-cols-3">
              <div className="border-b border-slate-200 p-4 dark:border-[#2a303a] lg:border-b-0 lg:border-r">
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  Best score
                </p>
                <p className="mt-3 truncate text-2xl font-semibold text-slate-950 dark:text-white">
                  {summary.srLeader?.gateway || '--'}
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  {summary.srLeader
                    ? `${formatPercent(summary.srLeader.value)} success rate, ${formatPercentPointDelta(srMargin)} vs next`
                    : 'No score in this window'}
                </p>
              </div>

              <div className="border-b border-slate-200 p-4 dark:border-[#2a303a] lg:border-b-0 lg:border-r">
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  Traffic leader
                </p>
                <p className="mt-3 truncate text-2xl font-semibold text-slate-950 dark:text-white">
                  {summary.volumeLeader?.gateway || '--'}
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  {summary.volumeLeader
                    ? `${formatNumber(summary.volumeLeader.count, 0)} payments, ${formatPercent(summary.volumeLeader.share)} traffic share`
                    : 'No payment traffic in this window'}
                </p>
              </div>

              <div className="p-4">
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  Best-score share
                </p>
                <p className="mt-3 text-2xl font-semibold text-slate-950 dark:text-white">
                  {alignmentText}
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  {alignedVolumeText}
                </p>
              </div>
            </div>

            {comparisonRows.length ? (
              <div className="overflow-x-auto rounded-2xl border border-slate-200 dark:border-[#2a303a]">
                <div className="min-w-[640px]">
                  <div className="grid grid-cols-[minmax(0,1.2fr)_0.65fr_0.65fr_0.65fr_1fr] gap-3 border-b border-slate-200 bg-slate-50 px-4 py-3 text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:border-[#2a303a] dark:bg-[#0c0f15] dark:text-[#8a8a93]">
                    <span>Connector</span>
                    <span>Success rate</span>
                    <span>Traffic</span>
                    <span>Share</span>
                    <span>Read</span>
                  </div>
                  {comparisonRows.map((row) => (
                    <div
                      key={row.gateway}
                      className="grid grid-cols-[minmax(0,1.2fr)_0.65fr_0.65fr_0.65fr_1fr] gap-3 border-b border-slate-200 px-4 py-3 text-sm last:border-b-0 dark:border-[#2a303a]"
                    >
                      <div className="flex min-w-0 items-center gap-2">
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: row.color }}
                        />
                        <span className="truncate font-medium text-slate-900 dark:text-white">
                          {row.gateway}
                        </span>
                      </div>
                      <span className="text-slate-600 dark:text-[#cbd5e1]">
                        {row.srValue === null ? '--' : formatPercent(row.srValue)}
                      </span>
                      <span className="text-slate-600 dark:text-[#cbd5e1]">
                        {formatNumber(row.count, 0)}
                      </span>
                      <span className="text-slate-600 dark:text-[#cbd5e1]">
                        {formatPercent(row.share)}
                      </span>
                      <div className="flex flex-wrap gap-2">
                        {row.isSrLeader ? <Badge variant="blue">Best score</Badge> : null}
                        {row.isVolumeLeader ? <Badge variant="green">Traffic leader</Badge> : null}
                        {!row.isSrLeader && !row.isVolumeLeader ? (
                          <span className="text-xs text-slate-500 dark:text-[#8a8a93]">Secondary</span>
                        ) : null}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <EmptyState
                title="No connector comparison yet"
                body="This view needs both payment traffic and connector scores in the selected time window."
              />
            )}
      </CardBody>
      ) : null}
    </Card>
  )
}

function analyticsRouteLabel(route: string) {
  if (route === '/decide_gateway') return 'Decide Gateway'
  if (route === '/update_gateway') return 'Update Gateway'
  if (route === '/rule_evaluate') return 'Rule Evaluate'
  return route
}

export function AnalyticsPage() {
  const location = useLocation()
  const { merchantId } = useMerchantStore()
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const effectiveMerchantId = merchantId || authMerchantId
  const [range, setRange] = useState<AnalyticsRangeValue>('1d')
  const [view, setView] = useState<AnalyticsView>('transactions')
  const [routingFilters, setRoutingFilters] = useState<RoutingFilters>(EMPTY_ROUTING_FILTERS)
  const [connectorFiltersOpen, setConnectorFiltersOpen] = useState(false)
  const [routingAlignmentOpen, setRoutingAlignmentOpen] = useState(false)
  const [showAllFilters, setShowAllFilters] = useState(false)
  const [previewListPage, setPreviewListPage] = useState(1)
  const [customStart, setCustomStart] = useState(() =>
    toDateTimeInputValue(Date.now() - 24 * 60 * 60 * 1000),
  )
  const [customEnd, setCustomEnd] = useState(() => toDateTimeInputValue(Date.now()))
  const [presetWindowBounds, setPresetWindowBounds] = useState<TimeWindow>(() =>
    presetWindow('1d'),
  )

  const customWindow = useMemo(() => {
    if (range !== 'custom') return undefined
    const start_ms = fromDateTimeInputValue(customStart)
    const end_ms = fromDateTimeInputValue(customEnd)
    const now = Date.now()
    if (start_ms === null || end_ms === null || end_ms <= start_ms || start_ms > now || end_ms > now) {
      return undefined
    }
    return { start_ms, end_ms }
  }, [customEnd, customStart, range])
  const activeQueryWindow = range === 'custom' ? customWindow : presetWindowBounds

  const overviewUrl =
    activeQueryWindow
      ? buildAnalyticsUrl('/analytics/overview', range, activeQueryWindow)
      : null
  const routingUrl =
    activeQueryWindow
      ? buildAnalyticsUrl('/analytics/routing-stats', range, activeQueryWindow)
      : null
  const filteredRoutingUrl =
    activeQueryWindow
      ? buildAnalyticsUrl('/analytics/routing-stats', range, activeQueryWindow, routingFilters)
      : null
  const previewTraceKey =
    activeQueryWindow
      ? ([
          'preview-trace-analytics',
          range,
          activeQueryWindow.start_ms,
          activeQueryWindow.end_ms,
        ] as const)
      : null
  const previewListUrl =
    activeQueryWindow
      ? buildPreviewTraceUrl(
          range,
          previewListPage,
          PREVIEW_LIST_PAGE_SIZE,
          activeQueryWindow,
        )
      : null

  const overviewSwrOptions = {
    revalidateOnFocus: false,
    revalidateIfStale: false,
    revalidateOnMount: true,
  } as const
  const routingSwrOptions = {
    revalidateOnFocus: false,
    revalidateIfStale: false,
    revalidateOnMount: true,
  } as const
  const filteredRoutingSwrOptions = {
    ...routingSwrOptions,
    keepPreviousData: true,
  } as const
  const previewListSwrOptions = {
    revalidateOnFocus: false,
    revalidateIfStale: false,
    revalidateOnMount: true,
    keepPreviousData: true,
  } as const

  const overview = useSWR<AnalyticsOverviewResponse>(overviewUrl, fetcher, overviewSwrOptions)
  const routing = useSWR<AnalyticsRoutingStatsResponse>(routingUrl, fetcher, routingSwrOptions)
  const filteredRouting = useSWR<AnalyticsRoutingStatsResponse>(
    filteredRoutingUrl,
    fetcher,
    filteredRoutingSwrOptions,
  )
  const previewTrace = useSWR<PaymentAuditResponse>(
    previewTraceKey,
    async (key) => {
      const [, selectedRange, startMs, endMs] = key as PreviewTraceKey
      return loadPreviewTraceSample(
        selectedRange,
        startMs !== null && endMs !== null
          ? { start_ms: Number(startMs), end_ms: Number(endMs) }
          : undefined,
      )
    },
    {
      revalidateOnFocus: false,
      revalidateIfStale: false,
      revalidateOnMount: true,
    },
  )
  const previewList = useSWR<PaymentAuditResponse>(
    previewListUrl,
    fetcher,
    previewListSwrOptions,
  )

  useEffect(() => {
    const revalidateCurrentView = () => {
      void overview.mutate()
      if (view === 'transactions') {
        void routing.mutate()
        void filteredRouting.mutate()
        return
      }
      void previewTrace.mutate()
      void previewList.mutate()
    }

    revalidateCurrentView()
    const timers = CATCH_UP_REFRESH_DELAYS_MS.map((delay) =>
      window.setTimeout(revalidateCurrentView, delay),
    )

    return () => {
      timers.forEach((timer) => window.clearTimeout(timer))
    }
  }, [location.key, view])

  const transactionLoading =
    (!overview.data && overview.isLoading) ||
    (!routing.data && routing.isLoading) ||
    (!filteredRouting.data && filteredRouting.isLoading)
  const ruleBasedLoading =
    (!overview.data && overview.isLoading) ||
    (!previewTrace.data && previewTrace.isLoading)
  const transactionError =
    overview.error?.message ||
    routing.error?.message ||
    filteredRouting.error?.message ||
    null
  const ruleBasedError =
    overview.error?.message ||
    previewTrace.error?.message ||
    previewList.error?.message ||
    null
  const loading = view === 'transactions' ? transactionLoading : ruleBasedLoading
  const error = view === 'transactions' ? transactionError : ruleBasedError

  const availableFilters: RoutingFilterOptions = {
    dimensions:
      routing.data?.available_filters?.dimensions ||
      filteredRouting.data?.available_filters?.dimensions ||
      [],
    missing_dimensions:
      routing.data?.available_filters?.missing_dimensions ||
      filteredRouting.data?.available_filters?.missing_dimensions ||
      [],
    gateways:
      routing.data?.available_filters?.gateways ||
      filteredRouting.data?.available_filters?.gateways ||
      [],
  }
  const availableFilterMap = useMemo(
    () =>
      new Map(
        availableFilters.dimensions.map((dimension) => [dimension.key, dimension] as const),
      ),
    [availableFilters.dimensions],
  )

  useEffect(() => {
    setRoutingFilters((current) => {
      const nextDimensions = Object.fromEntries(
        Object.entries(current.dimensions).filter(([key, value]) => {
          if (!value) return false
          const dimension = availableFilterMap.get(key)
          return dimension ? dimension.values.includes(value) : false
        }),
      )
      const nextGateways = current.gateways.filter((gateway) =>
        availableFilters.gateways.includes(gateway),
      )

      if (
        Object.keys(nextDimensions).length === Object.keys(current.dimensions).length &&
        Object.entries(nextDimensions).every(
          ([key, value]) => current.dimensions[key] === value,
        ) &&
        nextGateways.length === current.gateways.length &&
        nextGateways.every((gateway, index) => gateway === current.gateways[index])
      ) {
        return current
      }

      return {
        dimensions: nextDimensions,
        gateways: nextGateways,
      }
    })
  }, [availableFilterMap, availableFilters.gateways])

  useEffect(() => {
    if (availableFilters.dimensions.length <= MAX_VISIBLE_DIMENSIONS && showAllFilters) {
      setShowAllFilters(false)
    }
  }, [availableFilters.dimensions.length, showAllFilters])

  useEffect(() => {
    setPreviewListPage(1)
  }, [range, activeQueryWindow?.start_ms, activeQueryWindow?.end_ms])

  const activeWindowLabel = useMemo(() => {
    if (range !== 'custom') {
      return PRESET_OPTIONS.find((option) => option.value === range)?.label || 'Selected window'
    }
    if (!customWindow) return 'Custom window'
    return `${formatDateTime(customWindow.start_ms)} to ${formatDateTime(customWindow.end_ms)}`
  }, [customWindow, range])
  const effectiveWindow = useMemo(() => {
    if (activeQueryWindow) return activeQueryWindow
    return presetWindow(range as AnalyticsRange)
  }, [activeQueryWindow, range])

  const routeHits = useMemo(() => {
    const fallback = [
      { route: '/decide_gateway', count: 0 },
      { route: '/update_gateway', count: 0 },
      { route: '/rule_evaluate', count: 0 },
    ]
    if (!overview.data?.route_hits?.length) return fallback
    return fallback.map((item) => ({
      ...item,
      count: overview.data?.route_hits.find((row) => row.route === item.route)?.count || 0,
    }))
  }, [overview.data])
  const transactionRouteHits = useMemo(
    () => routeHits.filter((item) => item.route !== '/rule_evaluate'),
    [routeHits],
  )
  const ruleEvaluateHits = useMemo(
    () => routeHits.find((item) => item.route === '/rule_evaluate')?.count || 0,
    [routeHits],
  )
  const previewRows = previewTrace.data?.results || []
  const previewListRows = previewList.data?.results || []
  const previewGatewaySummary = useMemo(() => {
    const counts = new Map<string, number>()

    for (const row of previewRows) {
      const gateway = row.latest_gateway || 'No gateway selected'
      counts.set(gateway, (counts.get(gateway) || 0) + 1)
    }

    return Array.from(counts.entries())
      .map(([gateway, count]) => ({ gateway, count }))
      .sort((left, right) => right.count - left.count)
      .slice(0, 6)
  }, [previewRows])
  const previewStatusSummary = useMemo(() => {
    const counts = new Map<string, number>()

    for (const row of previewRows) {
      const status = row.latest_status || 'unknown'
      counts.set(status, (counts.get(status) || 0) + 1)
    }

    return Array.from(counts.entries())
      .map(([status, count]) => ({ status, count }))
      .sort((left, right) => right.count - left.count)
  }, [previewRows])
  const chartBucketSize = useMemo(
    () => bucketSizeForWindow(range, activeQueryWindow),
    [activeQueryWindow, range],
  )
  const bucketTickFormatter = useMemo(
    () => (value: number | string) => formatBucketLabel(Number(value), effectiveWindow),
    [effectiveWindow],
  )
  const previewConnectorSeriesData = useMemo(() => {
    const gateways = previewGatewaySummary.map((item) => item.gateway).slice(0, 6)
    const buckets = new Map<number, Record<string, number>>()

    for (const bucket_ms of buildBucketTimeline(effectiveWindow, chartBucketSize)) {
      buckets.set(
        bucket_ms,
        gateways.reduce<Record<string, number>>(
          (row, gateway) => {
            row[gateway] = 0
            return row
          },
          { bucket_ms },
        ),
      )
    }

    for (const row of previewRows) {
      const gateway = row.latest_gateway || 'No gateway selected'
      if (!gateways.includes(gateway)) continue
      const bucket_ms = bucketTimestamp(row.last_seen_ms, chartBucketSize)
      const bucket =
        buckets.get(bucket_ms) ||
        gateways.reduce<Record<string, number>>(
          (seriesRow, seriesGateway) => {
            seriesRow[seriesGateway] = 0
            return seriesRow
          },
          { bucket_ms },
        )
      bucket[gateway] = (bucket[gateway] || 0) + 1
      buckets.set(bucket_ms, bucket)
    }

    return {
      gateways,
      rows: Array.from(buckets.values()).sort((left, right) => left.bucket_ms - right.bucket_ms),
    }
  }, [chartBucketSize, effectiveWindow, previewRows, previewGatewaySummary])
  const latestPreviewActivity = previewRows[0]?.last_seen_ms
  const previewListTotalResults = previewList.data?.total_results || 0
  const previewListTotalPages = Math.max(
    1,
    Math.ceil(previewListTotalResults / PREVIEW_LIST_PAGE_SIZE),
  )
  const previewListStart = previewListTotalResults
    ? (previewListPage - 1) * PREVIEW_LIST_PAGE_SIZE + 1
    : 0
  const previewListEnd = previewListTotalResults
    ? previewListStart + previewListRows.length - 1
    : 0
  const previewGatewaysTouched = previewGatewaySummary.filter(
    (item) => item.gateway !== 'No gateway selected',
  ).length
  const previewGatewayMaxCount = previewGatewaySummary[0]?.count || 1
  const previewGatewayMixData = useMemo(() => {
    const total = previewGatewaySummary.reduce((sum, item) => sum + item.count, 0)

    return previewGatewaySummary.map((item, index) => ({
      name: item.gateway,
      value: item.count,
      percentage: total ? (item.count / total) * 100 : 0,
      color:
        item.gateway === 'No gateway selected'
          ? '#64748b'
          : CHART_COLORS[index % CHART_COLORS.length],
    }))
  }, [previewGatewaySummary])
  const previewIngestionPending =
    ruleEvaluateHits > 0 &&
    !previewTrace.error &&
    !previewList.error &&
    previewRows.length === 0 &&
    previewListRows.length === 0

  useEffect(() => {
    if (!previewListTotalResults && previewListPage !== 1) {
      setPreviewListPage(1)
      return
    }

    if (previewListPage > previewListTotalPages) {
      setPreviewListPage(previewListTotalPages)
    }
  }, [previewListPage, previewListTotalPages, previewListTotalResults])

  const gatewayShareData = useMemo(() => {
    const gatewaySharePoints = filteredRouting.data?.gateway_share || []
    const gateways = sortedGateways(gatewaySharePoints.map((point) => point.gateway || 'No gateway selected'))
    if (!gateways.length) {
      return {
        gateways,
        rows: [],
      }
    }

    const buckets = new Map<number, Record<string, number>>()

    for (const bucket_ms of buildBucketTimeline(effectiveWindow, chartBucketSize)) {
      buckets.set(
        bucket_ms,
        gateways.reduce<Record<string, number>>(
          (row, gateway) => {
            row[gateway] = 0
            return row
          },
          { bucket_ms },
        ),
      )
    }

    for (const point of gatewaySharePoints) {
      const gateway = point.gateway || 'No gateway selected'
      if (!gateways.includes(gateway)) continue
      const row =
        buckets.get(point.bucket_ms) ||
        gateways.reduce<Record<string, number>>(
          (seriesRow, seriesGateway) => {
            seriesRow[seriesGateway] = 0
            return seriesRow
          },
          { bucket_ms: point.bucket_ms },
        )
      row[gateway] = point.count
      buckets.set(point.bucket_ms, row)
    }

    return {
      gateways,
      rows: Array.from(buckets.values()).sort((left, right) => left.bucket_ms - right.bucket_ms),
    }
  }, [chartBucketSize, effectiveWindow, filteredRouting.data])

  const connectorTrendData = useMemo(() => {
    const gateways = sortedGateways((filteredRouting.data?.sr_trend || []).map((point) => point.gateway))
    if (!gateways.length) {
      return {
        gateways,
        rows: [],
      }
    }

    const buckets = new Map<number, Record<string, number | null>>()

    for (const bucket_ms of buildBucketTimeline(effectiveWindow, chartBucketSize)) {
      buckets.set(
        bucket_ms,
        gateways.reduce<Record<string, number | null>>(
          (row, gateway) => {
            row[gateway] = null
            return row
          },
          { bucket_ms },
        ),
      )
    }

    for (const point of filteredRouting.data?.sr_trend || []) {
      if (!gateways.includes(point.gateway)) continue
      const row =
        buckets.get(point.bucket_ms) ||
        gateways.reduce<Record<string, number | null>>(
          (seriesRow, seriesGateway) => {
            seriesRow[seriesGateway] = null
            return seriesRow
          },
          { bucket_ms: point.bucket_ms },
        )
      row[point.gateway] = toPercent(point.score_value)
      buckets.set(point.bucket_ms, row)
    }

    const rows = Array.from(buckets.values()).sort(
      (left, right) => Number(left.bucket_ms) - Number(right.bucket_ms),
    )

    // Treat score snapshots as state updates: once a connector emits a score,
    // keep that score in effect until a newer snapshot arrives.
    for (const gateway of gateways) {
      let lastKnownValue: number | null = null
      let hasSeenSnapshot = false

      for (const row of rows) {
        if (typeof row[gateway] === 'number') {
          lastKnownValue = row[gateway]
          hasSeenSnapshot = true
          continue
        }

        if (hasSeenSnapshot) {
          row[gateway] = lastKnownValue
        }
      }
    }

    return {
      gateways,
      rows,
    }
  }, [chartBucketSize, effectiveWindow, filteredRouting.data])

  const latestConnectorSummary = useMemo(() => {
    if (!connectorTrendData.rows.length) return []
    const latestRow = [...connectorTrendData.rows].reverse().find((row) =>
      connectorTrendData.gateways.some((gateway) => typeof row[gateway] === 'number'),
    )
    if (!latestRow) return []
    return connectorTrendData.gateways
      .map((gateway) => ({
        gateway,
        value: typeof latestRow[gateway] === 'number' ? latestRow[gateway] : null,
      }))
      .filter((item): item is { gateway: string; value: number } => item.value !== null)
  }, [connectorTrendData])
  const latestSrRanking = useMemo(
    () => [...latestConnectorSummary].sort((left, right) => right.value - left.value),
    [latestConnectorSummary],
  )

  const connectorTrendPointCounts = useMemo(() => {
    return connectorTrendData.gateways.reduce<Record<string, number>>((counts, gateway) => {
      counts[gateway] = connectorTrendData.rows.filter((row) => typeof row[gateway] === 'number').length
      return counts
    }, {})
  }, [connectorTrendData])

  const connectorTrendDomain = useMemo(() => {
    const values = connectorTrendData.rows.flatMap((row) =>
      connectorTrendData.gateways
        .map((gateway) => row[gateway])
        .filter((value): value is number => typeof value === 'number'),
    )

    if (!values.length) return [0, 100] as const

    const min = Math.min(...values)
    const max = Math.max(...values)
    const padding = min === max ? 5 : Math.max(2, (max - min) * 0.35)

    return [
      Math.max(0, Math.floor(min - padding)),
      Math.min(100, Math.ceil(max + padding)),
    ] as const
  }, [connectorTrendData])

  const gatewayVolumeSummary = useMemo<GatewayVolumeSummaryItem[]>(() => {
    const totals = new Map<string, number>()

    for (const row of gatewayShareData.rows) {
      for (const gateway of gatewayShareData.gateways) {
        totals.set(gateway, (totals.get(gateway) || 0) + readChartValue(row, gateway))
      }
    }

    const totalCount = Array.from(totals.values()).reduce((sum, value) => sum + value, 0)

    return gatewayShareData.gateways
      .map((gateway) => {
        const count = totals.get(gateway) || 0
        return {
          gateway,
          count,
          share: totalCount ? (count / totalCount) * 100 : 0,
        }
      })
      .filter((item) => item.count > 0)
      .sort((left, right) => right.count - left.count)
  }, [gatewayShareData])

  const routingAlignmentSummary = useMemo<RoutingAlignmentSummary>(() => {
    const srLeader = latestSrRanking[0] || null
    const srRunnerUp = latestSrRanking[1] || null
    const volumeLeader = gatewayVolumeSummary[0] || null
    const srLeaderVolume = srLeader
      ? gatewayVolumeSummary.find((item) => item.gateway === srLeader.gateway) || {
          gateway: srLeader.gateway,
          count: 0,
          share: 0,
        }
      : null
    const srRowsByBucket = new Map(
      connectorTrendData.rows.map((row) => [Number(row.bucket_ms), row] as const),
    )

    let leaderDecisionCount = 0
    let comparableDecisionCount = 0

    for (const shareRow of gatewayShareData.rows) {
      const decisionsInBucket = gatewayShareData.gateways.reduce(
        (sum, gateway) => sum + readChartValue(shareRow, gateway),
        0,
      )
      if (decisionsInBucket <= 0) continue

      const srRow = srRowsByBucket.get(Number(shareRow.bucket_ms))
      if (!srRow) continue

      const srValues = connectorTrendData.gateways
        .map((gateway) => {
          const value = srRow[gateway]
          return typeof value === 'number' && Number.isFinite(value)
            ? { gateway, value }
            : null
        })
        .filter((item): item is SrGatewaySummaryItem => item !== null)
        .sort((left, right) => right.value - left.value)
      if (!srValues.length) continue

      const topSrValue = srValues[0].value
      const srLeaders = srValues.filter(
        (item) => Math.abs(item.value - topSrValue) < 0.0001,
      )
      const leaderVolume = srLeaders.reduce(
        (sum, item) => sum + readChartValue(shareRow, item.gateway),
        0,
      )

      comparableDecisionCount += decisionsInBucket
      leaderDecisionCount += leaderVolume
    }

    const alignmentPercent = comparableDecisionCount
      ? (leaderDecisionCount / comparableDecisionCount) * 100
      : null

    let headline = 'Waiting for traffic share and connector scores.'
    let detail = 'Run payments and send score updates in this time window to compare routing traffic.'

    if (srLeader && volumeLeader) {
      const leadersDiffer = srLeader.gateway !== volumeLeader.gateway
      const comparisonText =
        comparableDecisionCount > 0
          ? `${srLeader.gateway} handled ${formatNumber(srLeaderVolume?.count || 0, 0)} of ${formatNumber(comparableDecisionCount, 0)} payments (${formatPercent(srLeaderVolume?.share || 0)} traffic share).`
          : 'Traffic and score updates did not happen close enough together to compare.'

      if (leadersDiffer) {
        headline = `${srLeader.gateway} has the better success rate; ${volumeLeader.gateway} still leads traffic share.`
        detail = `${comparisonText} Traffic share is cumulative for this window, so earlier ${volumeLeader.gateway} selections can keep it ahead even after failures reduce its score.`
      } else {
        headline = `${srLeader.gateway} has the better success rate and leads traffic share.`
        detail = comparisonText
      }
    } else if (srLeader) {
      headline = `${srLeader.gateway} has the better success rate, but no traffic share is available.`
      detail = 'Run decide-gateway traffic in this time window to compare scores against actual selections.'
    } else if (volumeLeader) {
      headline = `${volumeLeader.gateway} leads traffic share, but connector success rate is not available.`
      detail = 'Send update-gateway-score traffic in this time window to compare selections against success rates.'
    }

    return {
      srLeader,
      srRunnerUp,
      volumeLeader,
      srLeaderVolume,
      alignmentPercent,
      leaderDecisionCount,
      comparableDecisionCount,
      headline,
      detail,
    }
  }, [connectorTrendData, gatewayShareData, gatewayVolumeSummary, latestSrRanking])

  const connectorComparisonRows = useMemo<ConnectorComparisonRow[]>(() => {
    const srByGateway = new Map(latestSrRanking.map((item) => [item.gateway, item.value] as const))
    const volumeByGateway = new Map(gatewayVolumeSummary.map((item) => [item.gateway, item] as const))
    const gateways = Array.from(
      new Set([...connectorTrendData.gateways, ...gatewayShareData.gateways]),
    )
    const colorByGateway = new Map(
      gateways.map((gateway, index) => [gateway, CHART_COLORS[index % CHART_COLORS.length]] as const),
    )

    return gateways
      .map((gateway) => {
        const volume = volumeByGateway.get(gateway)
        return {
          gateway,
          srValue: srByGateway.get(gateway) ?? null,
          count: volume?.count || 0,
          share: volume?.share || 0,
          color: colorByGateway.get(gateway) || CHART_COLORS[0],
          isSrLeader: routingAlignmentSummary.srLeader?.gateway === gateway,
          isVolumeLeader: routingAlignmentSummary.volumeLeader?.gateway === gateway,
        }
      })
      .filter((item) => item.srValue !== null || item.count > 0)
      .sort((left, right) => {
        const leftSr = left.srValue ?? -1
        const rightSr = right.srValue ?? -1
        if (rightSr !== leftSr) return rightSr - leftSr
        return right.count - left.count
      })
      .slice(0, 6)
  }, [
    connectorTrendData.gateways,
    gatewayShareData.gateways,
    gatewayVolumeSummary,
    latestSrRanking,
    routingAlignmentSummary.srLeader?.gateway,
    routingAlignmentSummary.volumeLeader?.gateway,
  ])

  const activeFilterSummary = useMemo(() => {
    const parts = availableFilters.dimensions.flatMap((dimension) => {
      const value = routingFilters.dimensions[dimension.key]
      return value ? [`${dimension.label}: ${value}`] : []
    })
    if (routingFilters.gateways.length) parts.push(routingFilters.gateways.join(', '))
    return parts.length ? parts.join(' / ') : 'All routing dimensions'
  }, [availableFilters.dimensions, routingFilters])

  const visibleDimensions = useMemo(() => {
    if (showAllFilters || availableFilters.dimensions.length <= MAX_VISIBLE_DIMENSIONS) {
      return availableFilters.dimensions
    }
    return availableFilters.dimensions.slice(0, MAX_VISIBLE_DIMENSIONS)
  }, [availableFilters.dimensions, showAllFilters])

  const hasExtraDimensions = availableFilters.dimensions.length > MAX_VISIBLE_DIMENSIONS
  const hiddenDimensionCount = hasExtraDimensions
    ? availableFilters.dimensions.length - MAX_VISIBLE_DIMENSIONS
    : 0

  const activeFilterChips = useMemo(() => {
    const dimensionChips = availableFilters.dimensions.flatMap((dimension) => {
      const value = routingFilters.dimensions[dimension.key]
      return value
        ? [{ key: `dimension:${dimension.key}`, label: `${dimension.label}: ${value}` }]
        : []
    })
    const gatewayChips = routingFilters.gateways.map((gateway) => ({
      key: `gateway:${gateway}`,
      label: `Connector: ${gateway}`,
    }))
    return [...dimensionChips, ...gatewayChips]
  }, [availableFilters.dimensions, routingFilters])

  function handleRangeChange(value: AnalyticsRangeValue) {
    setRange(value)
    if (value !== 'custom') {
      const preset = presetWindow(value)
      setPresetWindowBounds(preset)
      setCustomStart(toDateTimeInputValue(preset.start_ms))
      setCustomEnd(toDateTimeInputValue(preset.end_ms))
    }
  }

  function refreshAll() {
    if (range !== 'custom') {
      setPresetWindowBounds(presetWindow(range))
      return
    }

    overview.mutate()
    routing.mutate()
    filteredRouting.mutate()
    previewTrace.mutate()
    previewList.mutate()
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

  function removeRoutingFilterChip(chipKey: string) {
    if (chipKey.startsWith('dimension:')) {
      updateDimensionFilter(chipKey.replace('dimension:', ''), '')
      return
    }
    if (chipKey.startsWith('gateway:')) {
      toggleGatewayFilter(chipKey.replace('gateway:', ''))
    }
  }

  function updateDimensionFilter(dimensionKey: string, value: string) {
    setRoutingFilters((current) => {
      const nextDimensions = { ...current.dimensions }
      if (value) {
        nextDimensions[dimensionKey] = value
      } else {
        delete nextDimensions[dimensionKey]
      }

      return {
        ...current,
        dimensions: nextDimensions,
      }
    })
  }

  return (
    <div className="space-y-8 px-5 sm:px-6 lg:px-8 xl:px-10">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Analytics</h1>
            <Badge variant="green">{overview.data?.merchant_id || effectiveMerchantId || 'Signed-in merchant'}</Badge>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 md:justify-end">
          <Button size="sm" variant="ghost" onClick={refreshAll}>
            Refresh
          </Button>
          <div className="flex flex-wrap items-center gap-2 rounded-full border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d]">
            {PRESET_OPTIONS.map((option) => (
              <Button
                key={option.value}
                size="sm"
                variant="secondary"
                className={sectionButtonClass(range === option.value)}
                onClick={() => handleRangeChange(option.value)}
              >
                {option.value === 'custom' ? 'Custom' : option.value}
              </Button>
            ))}
          </div>
        </div>
      </div>

      <div className="inline-flex max-w-full flex-wrap items-center gap-1 rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d]">
        <Button
          size="sm"
          variant="secondary"
          className={sectionButtonClass(view === 'transactions')}
          onClick={() => setView('transactions')}
        >
          {ANALYTICS_VIEW_LABELS.transactions}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          className={sectionButtonClass(view === 'rule_based')}
          onClick={() => setView('rule_based')}
        >
          {ANALYTICS_VIEW_LABELS.rule_based}
        </Button>
      </div>

      {range === 'custom' ? (
        <Card className="overflow-visible p-4">
          <div className="grid gap-3 md:grid-cols-2">
            <label className="space-y-2">
              <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                Start time
              </span>
              <DateTimePicker
                className="w-full"
                value={customStart}
                onChange={setCustomStart}
              />
            </label>

            <label className="space-y-2">
              <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                End time
              </span>
              <DateTimePicker
                className="w-full"
                value={customEnd}
                onChange={setCustomEnd}
              />
            </label>
          </div>
          {!customWindow ? (
            <p className="mt-3 text-xs text-red-500">
              Choose an end time after the start time. Future dates are not available.
            </p>
          ) : null}
        </Card>
      ) : null}

      <ErrorMessage error={error} />

      {loading ? (
        <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
          <Spinner size={16} />
          Loading analytics…
        </div>
      ) : null}

      <div className="relative">
      {view === 'transactions' ? (
        <div className="space-y-6">
          <section className="space-y-5">
            <div className="grid gap-5 lg:grid-cols-2">
              {transactionRouteHits.map((item) => (
                <HitsCard
                  key={item.route}
                  label={analyticsRouteLabel(item.route)}
                  value={item.count}
                  subtitle={range === 'custom' ? 'Custom window' : activeWindowLabel}
                />
              ))}
            </div>
          </section>

          <RoutingAlignmentCard
            summary={routingAlignmentSummary}
            comparisonRows={connectorComparisonRows}
            expanded={routingAlignmentOpen}
            onToggle={() => setRoutingAlignmentOpen((value) => !value)}
          />

          <Card className="overflow-visible">
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Selected gateways over time</h2>
              <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                Connector decision counts for the same active filters used by the success-rate chart.
              </p>
            </div>
            <InfoButton content={CARD_INFO.share} />
          </div>
        </CardHeader>
        <CardBody>
          {gatewayShareData.rows.length ? (
            <div className="h-80">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={gatewayShareData.rows}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                  <XAxis dataKey="bucket_ms" tickFormatter={bucketTickFormatter} tick={{ fontSize: 11 }} />
                  <YAxis tick={{ fontSize: 11 }} />
                  <Tooltip
                    labelFormatter={(label) => formatDateTime(Number(label))}
                    contentStyle={CHART_TOOLTIP_STYLE}
                    labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                    itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                    wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                  />
                  <Legend />
                  {gatewayShareData.gateways.map((gateway, index) => (
                    <Area
                      key={gateway}
                      type="monotone"
                      dataKey={gateway}
                      stroke={CHART_COLORS[index % CHART_COLORS.length]}
                      strokeWidth={3}
                      fill={CHART_COLORS[index % CHART_COLORS.length]}
                      fillOpacity={0.14}
                      name={gateway}
                    />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyState
              title="No gateway share history yet"
              body="Send real decide-gateway traffic in the selected window to populate connector share."
            />
          )}
        </CardBody>
      </Card>

          <Card className="overflow-visible">
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                Connector success rate over time
              </h2>
              <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                Historical connector success rate for the active filters; compare it with selected volume above.
              </p>
              <p className="mt-2 text-xs font-medium text-slate-600 dark:text-[#b3b3bd]">
                Active filters: {activeFilterSummary}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="secondary"
                onClick={() => setConnectorFiltersOpen((value) => !value)}
              >
                {connectorFiltersOpen ? 'Hide filters' : 'Show filters'}
              </Button>
              <InfoButton content={CARD_INFO.sr} />
            </div>
          </div>
        </CardHeader>
        <CardBody className="space-y-4">
          {connectorFiltersOpen ? (
          <div className="rounded-[24px] border border-slate-200 bg-white p-4 dark:border-[#1d1d23] dark:bg-[#0c0c0e]">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  Connector filters
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  Narrow the success-rate line chart by the routing dimensions present for this merchant.
                </p>
              </div>
              <Button
                size="sm"
                variant="secondary"
                onClick={clearRoutingFilters}
                disabled={
                  !Object.values(routingFilters.dimensions).some(Boolean) &&
                  !routingFilters.gateways.length
                }
              >
                Clear filters
              </Button>
            </div>

            {availableFilters.dimensions.length ? (
              <div className="mt-4 space-y-3">
                <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                  {visibleDimensions.map((dimension) => (
                    <label key={dimension.key} className="space-y-2">
                      <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                        {dimension.label}
                      </span>
                      <select
                        value={routingFilters.dimensions[dimension.key] || ''}
                        onChange={(event) => updateDimensionFilter(dimension.key, event.target.value)}
                        className={controlClassName()}
                        disabled={!dimension.values.length}
                      >
                        <option value="">All {dimension.label.toLowerCase()}</option>
                        {dimension.values.map((value) => (
                          <option key={value} value={value}>
                            {value}
                          </option>
                        ))}
                      </select>
                    </label>
                  ))}
                </div>
                {hasExtraDimensions ? (
                  <div className="flex items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-white px-4 py-3 dark:border-[#1d1d23] dark:bg-[#09090b]">
                    <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                      {showAllFilters
                        ? 'Showing all routing dimensions available for this merchant.'
                        : `${hiddenDimensionCount} more routing dimension${hiddenDimensionCount === 1 ? '' : 's'} available for this merchant.`}
                    </p>
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => setShowAllFilters((value) => !value)}
                    >
                      {showAllFilters ? 'Show fewer filters' : 'More filters'}
                    </Button>
                  </div>
                ) : null}
              </div>
            ) : availableFilters.missing_dimensions.length ? (
              <EmptyState
                title="No populated routing dimensions in this window"
                body="The merchant has score history, but none of the dynamic routing dimensions have values recorded in the selected time window yet."
              />
            ) : null}

            {availableFilters.missing_dimensions.length ? (
              <div className="mt-4 rounded-2xl border border-dashed border-slate-200 bg-white px-4 py-3 dark:border-[#1d1d23] dark:bg-[#09090b]">
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  No values in this window yet
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                  {availableFilters.missing_dimensions.map((dimension) => dimension.label).join(', ')}
                </p>
              </div>
            ) : null}

            {activeFilterChips.length ? (
              <div className="mt-4 space-y-2">
                <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                  Active filters
                </p>
                <div className="flex flex-wrap gap-2">
                  {activeFilterChips.map((chip) => (
                    <button
                      key={chip.key}
                      type="button"
                      onClick={() => removeRoutingFilterChip(chip.key)}
                      className="inline-flex items-center gap-2 rounded-full border border-brand-500/30 bg-brand-500/10 px-3 py-1.5 text-xs font-semibold text-brand-700 transition hover:bg-brand-500/15 dark:text-brand-200"
                    >
                      <span>{chip.label}</span>
                      <span aria-hidden="true">×</span>
                    </button>
                  ))}
                </div>
              </div>
            ) : null}

            <div className="mt-4 space-y-2">
              <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                Connectors
              </p>
              <div className="flex flex-wrap gap-2">
                {availableFilters.gateways.length ? (
                  availableFilters.gateways.map((gateway) => {
                    const active = routingFilters.gateways.includes(gateway)
                    return (
                      <button
                        key={gateway}
                        type="button"
                        onClick={() => toggleGatewayFilter(gateway)}
                        className={`rounded-full border px-3 py-1.5 text-xs font-semibold transition ${
                          active
                            ? 'border-brand-500/50 bg-brand-500/10 text-brand-700 dark:text-brand-200'
                            : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300 hover:text-slate-900 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#a1a1aa] dark:hover:text-white'
                        }`}
                      >
                        {gateway}
                      </button>
                    )
                  })
                ) : (
                  <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                    No connector history yet for the selected window.
                  </p>
                )}
              </div>
            </div>
          </div>
          ) : null}

          {latestConnectorSummary.length ? (
            <div className="flex flex-wrap gap-2">
              {latestConnectorSummary.map((item) => (
                <Badge key={item.gateway} variant="blue">
                  {item.gateway}: {formatPercent(item.value)}
                </Badge>
              ))}
            </div>
          ) : null}

          {connectorTrendData.rows.length ? (
            <div className="h-80">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={connectorTrendData.rows}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                  <XAxis dataKey="bucket_ms" tickFormatter={bucketTickFormatter} tick={{ fontSize: 11 }} />
                  <YAxis
                    domain={connectorTrendDomain as [number, number]}
                    tick={{ fontSize: 11 }}
                    tickFormatter={(value) => `${formatNumber(Number(value), 0)}%`}
                  />
                  <Tooltip
                    labelFormatter={(label) => formatDateTime(Number(label))}
                    formatter={(value: unknown, name: string | number) => [formatPercent(value as number), String(name)]}
                    contentStyle={CHART_TOOLTIP_STYLE}
                    labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                    itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                    wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                  />
                  <Legend />
                  {connectorTrendData.gateways.map((gateway, index) => (
                    <Line
                      key={gateway}
                      type="monotone"
                      dataKey={gateway}
                      stroke={CHART_COLORS[index % CHART_COLORS.length]}
                      strokeWidth={3}
                      dot={connectorTrendPointCounts[gateway] <= 1 ? { r: 4, strokeWidth: 2 } : false}
                      activeDot={{ r: 5 }}
                      connectNulls
                      name={gateway}
                    />
                  ))}
                </LineChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyState
              title="No connector score history yet"
              body="Send decide-gateway and update-gateway-score traffic in the selected window to populate connector history."
            />
          )}
        </CardBody>
      </Card>
        </div>
      ) : (
        <div className="space-y-6">
          <section className="space-y-5">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Rule-based activity</h2>
                <p className="mt-1 text-sm text-slate-500 dark:text-[#8a8a93]">
                  Routing decisions from <code>/routing/evaluate</code>, kept separate from auth-rate transaction routing and gateway scoring.
                </p>
              </div>
              <InfoButton content={CARD_INFO.preview_hits} />
            </div>

            <div className="grid gap-5 lg:grid-cols-2">
              <HitsCard
                label="Rule Evaluate"
                value={ruleEvaluateHits}
                subtitle={range === 'custom' ? 'Custom window' : activeWindowLabel}
              />
              <HitsCard
                label="Gateways touched"
                value={previewGatewaysTouched}
                subtitle="Across recent rule decisions"
                eyebrow="Decision coverage"
              />
            </div>
          </section>

          <div className="grid gap-5 xl:grid-cols-2">
            <Card className="overflow-visible">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                      Connector selections over time
                    </h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      Connector counts over time from the fetched rule decision sample.
                    </p>
                  </div>
                  <InfoButton content={CARD_INFO.preview_activity} />
                </div>
              </CardHeader>
              <CardBody>
                {previewConnectorSeriesData.gateways.length ? (
                  <div className="h-80">
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart data={previewConnectorSeriesData.rows} barGap={6}>
                        <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" />
                        <XAxis dataKey="bucket_ms" tickFormatter={bucketTickFormatter} tick={{ fontSize: 11 }} />
                        <YAxis tick={{ fontSize: 11 }} />
                        <Tooltip
                          labelFormatter={(label) => formatDateTime(Number(label))}
                          contentStyle={CHART_TOOLTIP_STYLE}
                          labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                          itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                          wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                        />
                        <Legend />
                        {previewConnectorSeriesData.gateways.map((gateway, index) => (
                          <Bar
                            key={gateway}
                            dataKey={gateway}
                            stackId="preview-connectors"
                            fill={
                              gateway === 'No gateway selected'
                                ? '#64748b'
                                : CHART_COLORS[index % CHART_COLORS.length]
                            }
                            radius={[6, 6, 0, 0]}
                            name={gateway}
                          />
                        ))}
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                ) : previewIngestionPending ? (
                  <PendingState
                    title="Processing recent rule decisions"
                    body="Rule evaluate calls have landed, but the decision sample has not been materialized yet. It will populate once analytics catches up."
                  />
                ) : (
                  <EmptyState
                    title="No connector selections yet"
                    body="Send /routing/evaluate decision traffic in the selected window to populate connector time-series."
                  />
                )}
              </CardBody>
            </Card>

            <Card className="overflow-visible">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                      Gateway selection mix
                    </h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      Connector share across the fetched rule decision sample.
                    </p>
                  </div>
                  <InfoButton content={CARD_INFO.preview_share} />
                </div>
              </CardHeader>
              <CardBody>
                {previewGatewayMixData.length ? (
                  <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_240px]">
                    <div className="relative h-80">
                      <ResponsiveContainer width="100%" height="100%">
                        <PieChart>
                          <Tooltip
                            formatter={(value: unknown, name: string | number, item: { payload?: { percentage?: number } }) => [
                              `${formatNumber(value as number, 0)} decisions`,
                              `${String(name)} (${formatPercent(item.payload?.percentage || 0)})`,
                            ]}
                            contentStyle={CHART_TOOLTIP_STYLE}
                            labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                            itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                            wrapperStyle={CHART_TOOLTIP_WRAPPER_STYLE}
                          />
                          <Legend />
                          <Pie
                            data={previewGatewayMixData}
                            dataKey="value"
                            nameKey="name"
                            innerRadius={72}
                            outerRadius={108}
                            paddingAngle={3}
                          >
                            {previewGatewayMixData.map((entry) => (
                              <Cell key={entry.name} fill={entry.color} />
                            ))}
                          </Pie>
                        </PieChart>
                      </ResponsiveContainer>
                      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center">
                        <p className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8a8a93]">
                          Sample size
                        </p>
                        <p className="mt-2 text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">
                          {previewRows.length}
                        </p>
                        <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                          decisions
                        </p>
                      </div>
                    </div>

                    <div className="space-y-3">
                      {previewGatewayMixData.map((item) => (
                        <div
                          key={item.name}
                          className="rounded-[20px] border border-slate-200 bg-white/80 px-4 py-3 dark:border-[#1d1d23] dark:bg-[#0c0c0e]"
                        >
                          <div className="flex items-center justify-between gap-3">
                            <div className="flex items-center gap-2">
                              <span
                                className="h-2.5 w-2.5 rounded-full"
                                style={{ backgroundColor: item.color }}
                              />
                              <p className="text-sm font-medium text-slate-900 dark:text-white">
                                {item.name}
                              </p>
                            </div>
                            <p className="text-xs font-semibold text-slate-500 dark:text-[#8a8a93]">
                              {item.value}
                            </p>
                          </div>
                          <p className="mt-2 text-xs text-slate-500 dark:text-[#8a8a93]">
                            {formatPercent(item.percentage)} of fetched decisions
                          </p>
                        </div>
                      ))}
                    </div>
                  </div>
                ) : previewIngestionPending ? (
                  <PendingState
                    title="Building decision connector mix"
                    body="Recent rule decision activity is still being folded into the fetched sample. This card will update automatically once the decision rows appear."
                  />
                ) : (
                  <EmptyState
                    title="No decision connector mix yet"
                    body="Rule decisions need to return gateway selections before the mix chart can render."
                  />
                )}
              </CardBody>
            </Card>
          </div>

          <div className="grid items-stretch gap-5 xl:grid-cols-2">
            <Card className="h-full overflow-visible">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Recent rule decisions</h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      Decisions captured from <code>/routing/evaluate</code>. This does not affect transaction scoring.
                    </p>
                  </div>
                  <Badge variant="purple">
                    {latestPreviewActivity ? `Latest ${formatDateTime(latestPreviewActivity)}` : 'No activity'}
                  </Badge>
                </div>
              </CardHeader>
              <CardBody>
                {!previewList.data && previewList.isLoading ? (
                  <div className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#8a8a93]">
                    <Spinner size={16} />
                    Loading rule decisions…
                  </div>
                ) : previewList.error && !previewList.data ? (
                  <ErrorMessage error={previewList.error.message} />
                ) : previewListRows.length ? (
                  <div className="space-y-4">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                        Showing {previewListStart}-{previewListEnd} of {previewListTotalResults}
                      </p>
                      {previewList.isLoading ? (
                        <div className="flex items-center gap-2 text-xs text-slate-500 dark:text-[#8a8a93]">
                          <Spinner size={14} />
                          Loading page…
                        </div>
                      ) : null}
                    </div>
                    <div className="max-h-[520px] space-y-3 overflow-y-auto pr-1">
                      {previewListRows.map((row) => (
                        <div
                          key={row.lookup_key}
                          className="rounded-[22px] border border-slate-200 bg-white/90 px-4 py-4 dark:border-[#1d1d23] dark:bg-[#0c0c0e]"
                        >
                          <div className="flex flex-wrap items-start justify-between gap-3">
                            <div className="min-w-0">
                              <p className="truncate text-sm font-semibold text-slate-900 dark:text-white">
                                {row.payment_id || row.request_id || row.lookup_key}
                              </p>
                              <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                                {(row.merchant_id || 'unknown merchant')} · {formatDateTime(row.last_seen_ms)}
                              </p>
                            </div>
                            <Badge variant="purple">
                              {row.latest_status || 'decision'}
                            </Badge>
                          </div>
                          <div className="mt-3 flex flex-wrap gap-2">
                            <Badge variant="blue">Rule Evaluate</Badge>
                            {row.latest_gateway ? <Badge variant="green">{row.latest_gateway}</Badge> : null}
                            <Badge variant="gray">{row.event_count} events</Badge>
                          </div>
                        </div>
                      ))}
                    </div>
                    {previewListTotalPages > 1 ? (
                      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-slate-200 pt-4 dark:border-[#1d1d23]">
                        <p className="text-xs text-slate-500 dark:text-[#8a8a93]">
                          Page {previewListPage} of {previewListTotalPages}
                        </p>
                        <div className="flex items-center gap-2">
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() =>
                              setPreviewListPage((current) => Math.max(1, current - 1))
                            }
                            disabled={previewListPage === 1 || previewList.isLoading}
                          >
                            Previous
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() =>
                              setPreviewListPage((current) =>
                                Math.min(previewListTotalPages, current + 1),
                              )
                            }
                            disabled={
                              previewListPage >= previewListTotalPages || previewList.isLoading
                            }
                          >
                            Next
                          </Button>
                        </div>
                      </div>
                    ) : null}
                  </div>
                ) : previewIngestionPending ? (
                  <PendingState
                    title="Waiting for decision rows"
                    body="Recent /routing/evaluate calls were recorded, but the detailed rule decision rows are still being flushed."
                  />
                ) : (
                  <EmptyState
                    title="No rule-based activity yet"
                    body="Send /routing/evaluate decision traffic in the selected window to populate rule-based activity."
                  />
                )}
              </CardBody>
            </Card>

            <div className="grid h-full gap-5 xl:grid-rows-2">
              <Card className="h-full overflow-visible">
                <CardHeader>
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Gateway activity</h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      Recent decisions grouped by latest chosen gateway.
                    </p>
                  </div>
                </CardHeader>
                <CardBody>
                  {previewGatewaySummary.length ? (
                    <div className="space-y-3">
                      {previewGatewaySummary.map((item, index) => (
                        <div key={item.gateway} className="space-y-2">
                          <div className="flex items-center justify-between gap-3">
                            <p className="text-sm font-medium text-slate-900 dark:text-white">{item.gateway}</p>
                            <p className="text-xs font-semibold text-slate-500 dark:text-[#8a8a93]">{item.count}</p>
                          </div>
                          <div className="h-2 overflow-hidden rounded-full bg-slate-100 dark:bg-[#141822]">
                            <div
                              className="h-full rounded-full"
                              style={{
                                width: `${(item.count / previewGatewayMaxCount) * 100}%`,
                                backgroundColor: CHART_COLORS[index % CHART_COLORS.length],
                              }}
                            />
                          </div>
                        </div>
                      ))}
                    </div>
                  ) : previewIngestionPending ? (
                    <PendingState
                      title="Waiting for gateway activity"
                      body="The decision sample is still being assembled from recent rule-evaluate calls. Gateway activity will appear here automatically once the rows are available."
                    />
                  ) : (
                    <EmptyState
                      title="No gateway activity yet"
                      body="Once rule decisions are captured, this panel will show which connectors are being selected."
                    />
                  )}
                </CardBody>
              </Card>

              <Card className="h-full overflow-visible">
                <CardHeader>
                  <div>
                    <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Recent decision outcomes</h2>
                    <p className="mt-1 text-xs text-slate-500 dark:text-[#8a8a93]">
                      Status mix from the loaded decision sample.
                    </p>
                  </div>
                </CardHeader>
                <CardBody>
                  {previewStatusSummary.length ? (
                    <div className="flex flex-wrap gap-2">
                      {previewStatusSummary.map((item) => (
                        <Badge key={item.status} variant={item.status.toLowerCase().includes('fail') ? 'red' : item.status === 'default_selection' ? 'orange' : 'purple'}>
                          {item.status} · {item.count}
                        </Badge>
                      ))}
                    </div>
                  ) : previewIngestionPending ? (
                    <PendingState
                      title="Waiting for decision outcomes"
                      body="Recent decision traffic is still being ingested. Outcome summaries will appear here automatically once the decision rows land."
                    />
                  ) : (
                    <EmptyState
                      title="No decision outcomes yet"
                      body="Recent rule decision results will appear here once decision traffic is recorded."
                    />
                  )}
                </CardBody>
              </Card>
            </div>
          </div>
        </div>
      )}
      </div>
    </div>
  )
}
