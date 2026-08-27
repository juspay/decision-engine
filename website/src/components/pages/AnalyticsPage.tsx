import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { useLocation, useSearchParams } from 'react-router-dom'
import useSWR from 'swr'
import {
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
import { CheckCircle2, ChevronRight, RefreshCw } from 'lucide-react'
import { fetcher } from '../../lib/api'
import { FEATURE_FLAGS } from '../../lib/featureFlags'
import { CHART_TOOLTIP_ITEM_STYLE, CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import {
  AnalyticsCostSavingsResponse,
  AnalyticsOverviewResponse,
  AnalyticsRange,
  AnalyticsRangeValue,
  AnalyticsRoutingStatsResponse,
  PaymentAuditResponse,
  RoutingFilterOptions,
  SmartRetryStats,
} from '../../types/api'

function formatCurrencyValue(value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      maximumFractionDigits: 2,
    }).format(value)
  } catch {
    return `${value.toFixed(2)} ${currency}`
  }
}

function formatCurrencyCompact(value: number, currency: string): string {
  try {
    return new Intl.NumberFormat(undefined, {
      style: 'currency',
      currency,
      notation: 'compact',
      maximumFractionDigits: 2,
    }).format(value)
  } catch {
    return `${value.toFixed(0)} ${currency}`
  }
}
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { TimeRangeFilter } from '../ui/TimeRangeFilter'
import {
  TimeWindow,
  customWindowFrom,
  presetWindow,
  toDateTimeInputValue,
} from '../../lib/timeRange'

import { PageHeading } from '../ui/PageHeading'
type RoutingFilters = {
  dimensions: Record<string, string>
  gateways: string[]
}

type AnalyticsView = 'transactions' | 'rule_based'
const ANALYTICS_VIEWS: readonly AnalyticsView[] = ['transactions', 'rule_based']
const ANALYTICS_VIEW_LABELS: Record<AnalyticsView, string> = {
  transactions: 'Multi-objective',
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

const CHART_COLORS = ['#0069ED', '#14b8a6', '#f97316', '#e11d48', '#8b5cf6', '#22c55e']
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
    calculation: 'Compares the latest connector success-rate score with the gateways actually selected in the same time window.',
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
    calculation: 'Rule Evaluate counts come from request-hit analytics for `/routing/evaluate`. Gateway coverage counts the unique connectors selected in rule decisions for this window.',
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
    purpose: 'Use this to see which connectors lead selected rule decisions, separate from multi-objective transaction decisions.',
    calculation: 'Returned decision traces are grouped by latest selected connector and displayed as share of selected rule decisions.',
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

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-slate-200 bg-white/60 px-6 py-12 text-center dark:border-[#222227] dark:bg-[#0b0b0d]">
      <p className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{title}</p>
      <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">{body}</p>
    </div>
  )
}

function PendingState({ title, body }: { title: string; body: string }) {
  return (
    <div className="rounded-2xl border border-slate-200 bg-white/60 px-6 py-12 text-center dark:border-[#222227] dark:bg-[#0b0b0d]">
      <div className="flex justify-center">
        <Spinner size={20} />
      </div>
      <p className="mt-5 text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{title}</p>
      <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">{body}</p>
    </div>
  )
}

function controlClassName() {
  return 'h-11 w-full rounded-2xl border border-slate-200 bg-white px-4 text-[13px] text-slate-700 shadow-sm outline-none transition focus:border-brand-500 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#e5e7eb]'
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
        className={`flex h-7 w-7 items-center justify-center rounded-full border text-[13px] font-semibold transition ${
          open
            ? 'border-brand-500/50 bg-brand-500/10 text-brand-700 dark:text-brand-200'
            : 'border-slate-200 bg-white text-slate-500 hover:border-slate-300 hover:text-slate-900 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#8a8a93] dark:hover:text-white'
        } leading-[18px]`}
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
          className="z-[120] rounded-2xl border border-slate-200 bg-white/95 p-4 shadow-2xl backdrop-blur dark:border-[#1d1d23] dark:bg-[#09090d]/95"
        >
          <p className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">{content.title}</p>
          <div className="mt-3 space-y-3 text-[13px] leading-6 text-slate-600 dark:text-[#b3b3bd]">
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">Why it matters</p>
              <p className="mt-2">{content.purpose}</p>
            </div>
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">How it is calculated</p>
              <p className="mt-2">{content.calculation}</p>
            </div>
            <div>
              <p className="font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">Data source</p>
              <p className="mt-2">{content.source}</p>
            </div>
          </div>
        </div>
      ) : null}
    </div>
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
    <Card className="!rounded-2xl overflow-visible">
      <CardHeader className={expanded ? 'px-5 py-3' : 'px-5 py-3 !border-b-0'}>
        <div className={`flex flex-wrap justify-between gap-3 ${expanded ? 'items-start' : 'items-center'}`}>
          {expanded ? (
            <div className="min-w-0">
              <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">
                Routing alignment
              </h2>
              <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                Traffic share: checks if the best-scoring connector is also getting the largest share.
              </p>
            </div>
          ) : (
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <CheckCircle2 size={18} className="shrink-0 text-emerald-700" />
              <span className="min-w-0 text-[13px] text-slate-600 dark:text-[#9aa7bb] leading-[18px]">
                <span className="font-semibold text-slate-800 dark:text-white">Routing alignment</span>
                {' — '}
                {collapsedReadout}
              </span>
            </div>
          )}
          <div className="flex shrink-0 items-center gap-2">
            {expanded ? (
              <>
                <InfoButton content={CARD_INFO.alignment} />
                <Button size="sm" variant="secondary" onClick={onToggle}>
                  Hide details
                </Button>
              </>
            ) : (
              <button
                type="button"
                onClick={onToggle}
                className="inline-flex items-center gap-2 text-[13px] font-semibold text-brand-600 transition hover:text-brand-700 dark:text-brand-300 dark:hover:text-brand-200 leading-[18px]"
              >
                Show details
                <ChevronRight size={16} />
              </button>
            )}
          </div>
        </div>
      </CardHeader>
      {expanded ? (
        <CardBody className="space-y-5 px-5 py-3">
          {/* The verdict reads as one sentence; the connectors it names are detailed in the table
              below, so it does not restate their rates or volumes. */}
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="gray">Best score: {srLeaderName}</Badge>
              <Badge variant={volumeBadgeVariant}>
                Traffic leader: {volumeLeaderName}
              </Badge>
            </div>
            <p className="text-[13px] font-semibold text-slate-900 dark:text-white leading-[18px]">
              {summary.headline}
            </p>
            <p className="text-[13px] leading-[21px] text-slate-500 dark:text-[#8a8a93]">
              {summary.detail}
            </p>
          </div>

          {/* The one figure the card exists to report, and the only one the table cannot show. */}
          <div className="flex flex-wrap items-baseline gap-x-3 gap-y-2 rounded-2xl border border-slate-200 bg-slate-50/80 px-5 py-3 dark:border-[#2a303a] dark:bg-[#0d1118]">
            <span className="text-[21px] font-semibold tabular-nums text-slate-950 dark:text-white leading-7">
              {summary.alignmentPercent === null ? 'Not enough data' : formatPercent(summary.alignmentPercent)}
            </span>
            <span className="text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
              {alignedVolumeText}
            </span>
          </div>

          {comparisonRows.length ? (
            <div className="overflow-x-auto">
              <div className="min-w-[640px]">
                <div className="grid grid-cols-[minmax(0,1.2fr)_0.9fr_0.65fr_0.65fr_1fr] gap-3 border-b border-slate-200 px-3 py-3 text-[13px] font-medium text-slate-500 dark:border-[#2a303a] dark:text-[#8a8a93] leading-[18px]">
                  <span>Connector</span>
                  <span>Success rate</span>
                  <span>Traffic</span>
                  <span>Share</span>
                  <span>Role</span>
                </div>
                {comparisonRows.map((row) => (
                  <div
                    key={row.gateway}
                    className="grid grid-cols-[minmax(0,1.2fr)_0.9fr_0.65fr_0.65fr_1fr] gap-3 border-b border-slate-200 px-3 py-3 text-[13px] last:border-b-0 dark:border-[#2a303a] leading-[18px]"
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
                    <span className="flex flex-wrap items-baseline gap-2 text-slate-600 dark:text-[#cbd5e1]">
                      {row.srValue === null ? '--' : formatPercent(row.srValue)}
                      {/* The winning margin belongs beside the rate it is a margin on — a lead of a
                          tenth of a point is the difference between a verdict and a coin toss. */}
                      {row.isSrLeader && srMargin !== undefined ? (
                        <span className="text-slate-500 dark:text-[#78849a]">
                          {formatPercentPointDelta(srMargin)} vs next
                        </span>
                      ) : null}
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
                        <span className="text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">Secondary</span>
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

function authRateColor(rate: number) {
  if (rate >= 0.85) return 'text-emerald-700 dark:text-emerald-400'
  if (rate >= 0.70) return 'text-amber-700 dark:text-amber-400'
  return 'text-red-600 dark:text-red-400'
}

function srBadgeVariant(valuePercent: number): BadgeVariant {
  if (valuePercent >= 85) return 'green'
  if (valuePercent >= 70) return 'orange'
  return 'red'
}

function SmartRetrySection({ stats }: { stats: SmartRetryStats | null }) {
  if (!stats || stats.retried_count === 0) return null
  const recoveryRate = Math.round((stats.recovered_count / stats.retried_count) * 100)
  const rateColor = recoveryRate >= 70
    ? 'text-emerald-700 dark:text-emerald-400'
    : recoveryRate >= 40 ? 'text-amber-700' : 'text-red-600'

  return (
    <div className="space-y-5">
      {/* Summary row */}
      <Card className="!rounded-2xl">
        <CardHeader>
          <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Smart Retry</h2>
          <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
            Payments retried on a fallback gateway after GSM classified the failure as retryable.
          </p>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-3 gap-5">
            <div className="rounded-lg bg-slate-50 dark:bg-[#111114] px-4 py-3">
              <p className="text-[13px] text-slate-500 dark:text-slate-400 leading-[18px]">Retried</p>
              <p className="mt-2 text-[21px] font-bold text-slate-800 dark:text-white tabular-nums leading-7">{stats.retried_count}</p>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-[#111114] px-4 py-3">
              <p className="text-[13px] text-slate-500 dark:text-slate-400 leading-[18px]">Recovered</p>
              <p className="mt-2 text-[21px] font-bold text-emerald-700 dark:text-emerald-400 tabular-nums leading-7">{stats.recovered_count}</p>
            </div>
            <div className="rounded-lg bg-slate-50 dark:bg-[#111114] px-4 py-3">
              <p className="text-[13px] text-slate-500 dark:text-slate-400 leading-[18px]">Recovery rate</p>
              <p className={`mt-2 text-[21px] font-bold tabular-nums ${rateColor} leading-7`}>{recoveryRate}%</p>
            </div>
          </div>
        </CardBody>
      </Card>

      <div className="grid gap-5 lg:grid-cols-2">
        {/* By trigger — which connectors failed with a retryable error */}
        {stats.by_trigger.length > 0 && (
          <Card className="!rounded-2xl">
            <CardHeader>
              <h3 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Retry triggers by connector</h3>
              <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                Connectors whose failures were classified as retryable by GSM.
              </p>
            </CardHeader>
            <CardBody className="p-0">
              <table className="w-full text-[13px] leading-[18px]">
                <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-[11px] text-slate-500 dark:text-slate-500 border-b border-slate-100 dark:border-[#1c1c24] leading-4">
                  <tr>
                    <th className="text-left px-4 py-2">Connector</th>
                    <th className="text-left px-4 py-2">Error code</th>
                    <th className="text-right px-4 py-2">Count</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100 dark:divide-[#1a1a22]">
                  {stats.by_trigger.map((row, i) => (
                    <tr key={i} className="hover:bg-slate-50 dark:hover:bg-[#0d0d14]">
                      <td className="px-4 py-2 text-[13px] font-medium text-slate-700 dark:text-slate-300 leading-[18px]">{row.gateway}</td>
                      <td className="px-4 py-2">
                        {row.error_code
                          ? <span className="font-mono text-[11px] text-slate-500 dark:text-slate-400 leading-4">{row.error_code}</span>
                          : <span className="text-[11px] text-slate-500 leading-4">—</span>}
                      </td>
                      <td className="px-4 py-2 text-right tabular-nums text-[13px] text-slate-600 dark:text-slate-300 leading-[18px]">{row.count}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </CardBody>
          </Card>
        )}

        {/* By fallback — which connectors were used as fallback and how well they recovered */}
        {stats.by_fallback.length > 0 && (
          <Card className="!rounded-2xl">
            <CardHeader>
              <h3 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Recovery by fallback connector</h3>
              <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                Connectors used as the retry fallback and their recovery outcomes.
              </p>
            </CardHeader>
            <CardBody className="p-0">
              <table className="w-full text-[13px] leading-[18px]">
                <thead className="bg-slate-50 dark:bg-[#0a0a0f] text-[11px] text-slate-500 dark:text-slate-500 border-b border-slate-100 dark:border-[#1c1c24] leading-4">
                  <tr>
                    <th className="text-left px-4 py-2">Connector</th>
                    <th className="text-right px-4 py-2">Retried</th>
                    <th className="text-right px-4 py-2">Recovered</th>
                    <th className="text-right px-4 py-2">Rate</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100 dark:divide-[#1a1a22]">
                  {stats.by_fallback.map((row, i) => {
                    const rate = row.retried > 0 ? Math.round((row.recovered / row.retried) * 100) : 0
                    return (
                      <tr key={i} className="hover:bg-slate-50 dark:hover:bg-[#0d0d14]">
                        <td className="px-4 py-2 text-[13px] font-medium text-slate-700 dark:text-slate-300 leading-[18px]">{row.gateway}</td>
                        <td className="px-4 py-2 text-right tabular-nums text-[13px] text-slate-600 dark:text-slate-300 leading-[18px]">{row.retried}</td>
                        <td className="px-4 py-2 text-right tabular-nums text-[13px] text-emerald-700 dark:text-emerald-400 leading-[18px]">{row.recovered}</td>
                        <td className="px-4 py-2 text-right">
                          <span className={`text-[13px] font-semibold tabular-nums ${rate >= 70 ? 'text-emerald-700 dark:text-emerald-400' : rate >= 40 ? 'text-amber-700' : 'text-red-600'} leading-[18px]`}>
                            {rate}%
                          </span>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </CardBody>
          </Card>
        )}
      </div>
    </div>
  )
}

export function AnalyticsPage() {
  const location = useLocation()
  const [range, setRange] = useState<AnalyticsRangeValue>('1d')
  // View is kept in the URL (?view=…) so a reload or shared/searched link
  // reopens it directly; the default (transactions) is left out of the URL.
  const [searchParams, setSearchParams] = useSearchParams()
  const viewParam = searchParams.get('view')
  const view: AnalyticsView = ANALYTICS_VIEWS.includes(viewParam as AnalyticsView)
    ? (viewParam as AnalyticsView)
    : 'transactions'
  const setView = (nextView: AnalyticsView) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev)
        if (nextView === 'transactions') next.delete('view')
        else next.set('view', nextView)
        return next
      },
      { replace: true },
    )
  }
  // Keep the URL canonical: an unknown or explicitly-default ?view= is rewritten
  // to the canonical form (default omitted) so the URL never disagrees with the
  // rendered view and default links stay shareable/canonical.
  useEffect(() => {
    const canonical = view === 'transactions' ? null : view
    if (viewParam !== canonical) setView(view)
    // setView is stable enough for this purpose; re-run only on the derived state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, viewParam])
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

  const customWindow = useMemo(
    () => (range === 'custom' ? customWindowFrom(customStart, customEnd) : undefined),
    [customEnd, customStart, range],
  )
  const activeQueryWindow = range === 'custom' ? customWindow : presetWindowBounds

  const costCurrency = 'USD'

  const overviewUrl =
    activeQueryWindow
      ? buildAnalyticsUrl('/analytics/overview', range, activeQueryWindow)
      : null
  const routingUrl =
    activeQueryWindow
      ? buildAnalyticsUrl('/analytics/routing-stats', range, activeQueryWindow)
      : null
  const costSavingsUrl = (() => {
    if (!activeQueryWindow) return null
    const base = buildAnalyticsUrl('/analytics/cost-savings', range, activeQueryWindow)
    if (!costCurrency) return base
    const sep = base.includes('?') ? '&' : '?'
    return `${base}${sep}currency=${encodeURIComponent(costCurrency)}`
  })()
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
  const costSavings = useSWR<AnalyticsCostSavingsResponse>(costSavingsUrl, fetcher, routingSwrOptions)
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
  const overallAuthRate = useMemo(() => {
    const scores = overview.data?.top_scores ?? []
    const totalTx = scores.reduce((sum, s) => sum + s.transaction_count, 0)
    if (!totalTx) return null
    const weightedSum = scores.reduce((sum, s) => sum + s.score_value * s.transaction_count, 0)
    return weightedSum / totalTx
  }, [overview.data])
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
  const previewStatusTotal = useMemo(
    () => previewStatusSummary.reduce((sum, item) => sum + item.count, 0),
    [previewStatusSummary],
  )
  const ruleMatchRate = useMemo(() => {
    if (!previewStatusTotal) return null
    const successCount = previewStatusSummary.find((s) => s.status === 'success')?.count ?? 0
    return successCount / previewStatusTotal
  }, [previewStatusSummary, previewStatusTotal])
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
  const previewGatewaysTouched = previewGatewaySummary.filter(
    (item) => item.gateway !== 'No gateway selected',
  ).length
  const previewGatewayMaxCount = previewGatewaySummary[0]?.count || 1
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
    if (value === 'custom') return

    const preset = presetWindow(value)
    setPresetWindowBounds(preset)
    setCustomStart(toDateTimeInputValue(preset.start_ms))
    setCustomEnd(toDateTimeInputValue(preset.end_ms))
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
    <div className="space-y-8 px-5 sm:px-5 lg:px-8 xl:px-8">
      <div className="grid grid-cols-1 items-center gap-3 xl:grid-cols-[1fr_auto_1fr]">
        <div className="space-y-2">
          <PageHeading title="Analytics" description="Real-time multi-gateway routing performance overview." />
        </div>

        <div className="inline-flex max-w-full flex-wrap items-center gap-1 justify-self-start rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d] xl:justify-self-center">
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

        <div className="flex items-center gap-2 justify-self-start xl:justify-self-end">
          <TimeRangeFilter
            range={range}
            customStart={customStart}
            customEnd={customEnd}
            onRangeChange={handleRangeChange}
            onCustomChange={(nextStart, nextEnd) => {
              setCustomStart(nextStart)
              setCustomEnd(nextEnd)
            }}
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

      <ErrorMessage error={error} />

      {loading ? (
        <div className="flex items-center gap-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
          <Spinner size={16} />
          Loading analytics…
        </div>
      ) : null}

      <div className="relative">
      {view === 'transactions' ? (
        <div className="space-y-5">
          <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
            <Card className="!rounded-2xl">
              <CardBody>
                <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                  Overall auth rate
                </p>
                {overallAuthRate !== null ? (
                  <>
                    <p className={`mt-2 text-[34px] font-semibold tabular-nums ${authRateColor(overallAuthRate)} leading-[42px]`}>
                      {formatPercent(overallAuthRate)}
                    </p>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                      Weighted across all gateways
                    </p>
                  </>
                ) : (
                  <>
                    <p className="mt-2 text-[34px] font-semibold text-slate-500 dark:text-slate-400 leading-[42px]">—</p>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">No score data yet</p>
                  </>
                )}
              </CardBody>
            </Card>

            {transactionRouteHits.map((item) => (
              <Card key={item.route} className="!rounded-2xl">
                <CardBody>
                  <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    {analyticsRouteLabel(item.route)}
                  </p>
                  <p className="mt-2 text-[34px] font-semibold tabular-nums text-slate-950 dark:text-white leading-[42px]">
                    {formatNumber(item.count, 0)}
                  </p>
                  <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    {item.route === '/decide_gateway' ? 'routing decisions' : 'score feedback calls'}
                  </p>
                </CardBody>
              </Card>
            ))}

            <Card className="!rounded-2xl">
              <CardBody>
                <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                  Total cost saved
                </p>
                {costSavings.data && costSavings.data.currency && costSavings.data.totals.saved_value > 0 ? (
                  <>
                    <p className="mt-2 text-[34px] font-semibold tabular-nums text-emerald-700 dark:text-emerald-400 leading-[42px]">
                      {formatCurrencyValue(costSavings.data.totals.saved_value, costSavings.data.currency)}
                    </p>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] max-w-[57ch] leading-[18px]">
                      {formatNumber(costSavings.data.totals.cost_won_count, 0)} of {formatNumber(costSavings.data.totals.total_decisions, 0)} cost decisions
                    </p>
                  </>
                ) : (
                  <>
                    <p className="mt-2 text-[34px] font-semibold text-slate-500 dark:text-slate-400 leading-[42px]">—</p>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                      {costSavings.data ? 'No multi-objective wins yet' : 'Loading…'}
                    </p>
                  </>
                )}
              </CardBody>
            </Card>
          </div>

          <RoutingAlignmentCard
            summary={routingAlignmentSummary}
            comparisonRows={connectorComparisonRows}
            expanded={routingAlignmentOpen}
            onToggle={() => setRoutingAlignmentOpen((value) => !value)}
          />

          {FEATURE_FLAGS.SMART_RETRY_IN_ANALYTICS && <SmartRetrySection stats={overview.data?.smart_retry_stats ?? null} />}

          <Card className="!rounded-2xl overflow-visible">
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Selected gateways over time</h2>
              <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
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
                <BarChart data={gatewayShareData.rows} barCategoryGap="35%" barGap={3}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" vertical={false} />
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
                    <Bar
                      key={gateway}
                      dataKey={gateway}
                      fill={CHART_COLORS[index % CHART_COLORS.length]}
                      name={gateway}
                      radius={[3, 3, 0, 0]}
                    />
                  ))}
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyState
              title="No gateway share history yet"
              body="Gateway traffic will appear here after payments are routed in this window."
            />
          )}
        </CardBody>
      </Card>

          <Card className="!rounded-2xl overflow-visible">
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <div>
                  <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Cost saved over time</h2>
                  <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] max-w-[57ch] leading-[18px]">
                    Savings from multi-objective routing promoting the higher-EV PSP within the derived auth band.
                  </p>
                </div>
              </div>
            </CardHeader>
            <CardBody>
              {costSavings.data && costSavings.data.currency && costSavings.data.trend.length > 0 ? (
                <div className="h-72">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={costSavings.data.trend} barCategoryGap="35%">
                      <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" vertical={false} />
                      <XAxis dataKey="bucket_ms" tickFormatter={bucketTickFormatter} tick={{ fontSize: 11 }} />
                      <YAxis
                        tick={{ fontSize: 11 }}
                        tickFormatter={(value: number) => formatCurrencyCompact(value, costSavings.data!.currency!)}
                        width={70}
                      />
                      <Tooltip
                        labelFormatter={(label) => formatDateTime(Number(label))}
                        formatter={(value: number) => [formatCurrencyValue(value, costSavings.data!.currency!), 'Saved']}
                        contentStyle={CHART_TOOLTIP_STYLE}
                        labelStyle={CHART_TOOLTIP_LABEL_STYLE}
                        itemStyle={CHART_TOOLTIP_ITEM_STYLE}
                      />
                      <Bar dataKey="saved_value" fill="#10b981" name="Saved" radius={[3, 3, 0, 0]} />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <EmptyState
                  title="No cost savings yet"
                  body="Cost savings appear once multi-objective routing promotes a higher-EV PSP within the derived auth band."
                />
              )}
            </CardBody>
          </Card>

          <Card className="!rounded-2xl overflow-visible">
        <CardHeader>
          <div className="flex items-start justify-between gap-3">
            <div>
              <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">
                Connector success rate over time
              </h2>
              <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] max-w-[57ch] leading-[18px]">
                Historical connector success rate for the active filters; compare it with selected volume above.
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
        <CardBody className="space-y-5">
          {connectorFiltersOpen ? (
          <div className="space-y-3 border-b border-slate-200 pb-5 dark:border-[#1d1d23]">
            {availableFilters.dimensions.length ? (
              <>
                <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                  {visibleDimensions.map((dimension) => (
                    <label key={dimension.key} className="block">
                      <span className="mb-2 block text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                        {dimension.label}
                      </span>
                      <select
                        value={routingFilters.dimensions[dimension.key] || ''}
                        onChange={(event) => updateDimensionFilter(dimension.key, event.target.value)}
                        className={controlClassName()}
                        disabled={!dimension.values.length}
                      >
                        <option value="">Any {dimension.label.toLowerCase()}</option>
                        {dimension.values.map((value) => (
                          <option key={value} value={value}>
                            {value}
                          </option>
                        ))}
                      </select>
                    </label>
                  ))}
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    Connectors
                  </span>
                  {availableFilters.gateways.length ? (
                    availableFilters.gateways.map((gateway) => {
                      const active = routingFilters.gateways.includes(gateway)
                      return (
                        <button
                          key={gateway}
                          type="button"
                          aria-pressed={active}
                          onClick={() => toggleGatewayFilter(gateway)}
                          className={`rounded-full border px-3 py-2 text-[13px] font-medium transition ${
                            active
                              ? 'border-brand-500/50 bg-brand-500/10 text-brand-700 dark:text-brand-200'
                              : 'border-slate-200 bg-white text-slate-600 hover:border-slate-300 hover:text-slate-900 dark:border-[#27272a] dark:bg-[#121214] dark:text-[#a1a1aa] dark:hover:text-white'
                          } leading-[18px]`}
                        >
                          {gateway}
                        </button>
                      )
                    })
                  ) : (
                    <span className="text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                      No connector history yet for the selected window.
                    </span>
                  )}
                </div>
              </>
            ) : availableFilters.missing_dimensions.length ? (
              <EmptyState
                title="No routing dimension values in this window"
                body="Score history exists, but no routing dimensions have values recorded in the selected time window yet."
              />
            ) : null}

            <div className="flex flex-wrap items-center gap-3">
              {hasExtraDimensions ? (
                <button
                  type="button"
                  onClick={() => setShowAllFilters((value) => !value)}
                  className="text-[13px] font-medium text-brand-600 transition hover:text-brand-600 dark:text-brand-400 leading-[18px]"
                >
                  {showAllFilters
                    ? 'Fewer dimensions'
                    : `${hiddenDimensionCount} more dimension${hiddenDimensionCount === 1 ? '' : 's'}`}
                </button>
              ) : null}

              {availableFilters.missing_dimensions.length ? (
                <span className="text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                  No values yet: {availableFilters.missing_dimensions.map((dimension) => dimension.label).join(', ')}
                </span>
              ) : null}

              {activeFilterChips.map((chip) => (
                <button
                  key={chip.key}
                  type="button"
                  onClick={() => removeRoutingFilterChip(chip.key)}
                  className="inline-flex items-center gap-2 rounded-full border border-brand-500/30 bg-brand-500/10 px-3 py-2 text-[13px] font-medium text-brand-700 transition hover:bg-brand-500/15 dark:text-brand-200 leading-[18px]"
                >
                  <span>{chip.label}</span>
                  <span aria-hidden="true">×</span>
                </button>
              ))}

              {activeFilterChips.length ? (
                <button
                  type="button"
                  onClick={clearRoutingFilters}
                  className="ml-auto text-[13px] font-medium text-slate-500 transition hover:text-slate-900 dark:text-[#8a8a93] dark:hover:text-white leading-[18px]"
                >
                  Clear filters
                </button>
              ) : null}
            </div>
          </div>
          ) : null}

          {latestConnectorSummary.length ? (
            <div className="flex flex-wrap gap-2">
              {latestConnectorSummary.map((item) => (
                <Badge key={item.gateway} variant={srBadgeVariant(item.value)}>
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
              body="Connector history will appear after decide-gateway and update-gateway-score activity in this window."
            />
          )}
        </CardBody>
      </Card>
        </div>
      ) : (
        <div className="space-y-5">
          <Card className="!rounded-2xl">
            <CardBody>
              <div className="grid gap-5 sm:grid-cols-3">
                <div>
                  <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    Rule match rate
                  </p>
                  {ruleMatchRate !== null ? (
                    <>
                      <p className={`mt-2 text-[34px] font-semibold tabular-nums ${authRateColor(ruleMatchRate)} leading-[42px]`}>
                        {formatPercent(ruleMatchRate)}
                      </p>
                      <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                        Decisions matched to a configured rule
                      </p>
                    </>
                  ) : (
                    <>
                      <p className="mt-2 text-[34px] font-semibold text-slate-500 dark:text-slate-400 leading-[42px]">—</p>
                      <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">No decision data yet</p>
                    </>
                  )}
                </div>
                <div>
                  <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    Rule Evaluate
                  </p>
                  <p className="mt-2 text-[21px] font-semibold tabular-nums text-slate-950 dark:text-white leading-7">
                    {formatNumber(ruleEvaluateHits, 0)}
                  </p>
                  <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">routing decisions</p>
                </div>
                <div>
                  <p className="text-[13px] font-medium text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                    Gateways active
                  </p>
                  <p className="mt-2 text-[21px] font-semibold tabular-nums text-slate-950 dark:text-white leading-7">
                    {previewGatewaysTouched}
                  </p>
                  <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">unique connectors selected</p>
                </div>
              </div>
            </CardBody>
          </Card>

          <Card className="!rounded-2xl overflow-visible">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">
                      Connector selections over time
                    </h2>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                      Connector counts over time for selected rule decisions.
                    </p>
                  </div>
                  <InfoButton content={CARD_INFO.preview_activity} />
                </div>
              </CardHeader>
              <CardBody>
                {previewConnectorSeriesData.gateways.length ? (
                  <div className="h-80">
                    <ResponsiveContainer width="100%" height="100%">
                      <BarChart data={previewConnectorSeriesData.rows} barCategoryGap="35%" barGap={3}>
                        <CartesianGrid strokeDasharray="3 3" stroke="#e2e8f0" vertical={false} />
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
                    body="Rule evaluate calls were received. Recent decisions are still being processed, and this view will update automatically."
                  />
                ) : (
                  <EmptyState
                    title="No connector selections yet"
                    body="Rule decision traffic will appear here after /routing/evaluate calls in this window."
                  />
                )}
              </CardBody>
            </Card>

          <div className="grid items-stretch gap-5 xl:grid-cols-2">
            <Card className="!rounded-2xl h-full overflow-visible">
              <CardHeader>
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Recent decisions</h2>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                      Latest decisions from <code>/routing/evaluate</code>.
                    </p>
                  </div>
                  <Badge variant="purple">
                    {latestPreviewActivity ? `Latest ${formatDateTime(latestPreviewActivity)}` : 'No activity'}
                  </Badge>
                </div>
              </CardHeader>
              <CardBody>
                {previewRows.length ? (
                  <div className="space-y-2">
                    {previewRows.slice(0, 10).map((row) => {
                      const statusVariant: BadgeVariant = row.latest_status?.toLowerCase()?.includes('fail') ? 'red' : row.latest_status === 'default_selection' ? 'orange' : 'green'
                      return (
                        <div
                          key={row.lookup_key}
                          className="flex items-center justify-between gap-3 rounded-2xl border border-slate-100 bg-slate-50/60 px-3 py-2.5 dark:border-[#1d1d23] dark:bg-[#0c0c0e]"
                        >
                          <div className="flex min-w-0 items-center gap-2">
                            <Badge variant={statusVariant}>
                              {row.latest_status || 'decision'}
                            </Badge>
                            {row.latest_gateway ? (
                              <span className="truncate text-[13px] font-medium text-slate-700 dark:text-[#c8d3e6] leading-[18px]">
                                {row.latest_gateway}
                              </span>
                            ) : null}
                          </div>
                          <span className="shrink-0 text-[13px] text-slate-500 dark:text-[#78849a] leading-[18px]">
                            {formatDateTime(row.last_seen_ms)}
                          </span>
                        </div>
                      )
                    })}
                  </div>
                ) : previewIngestionPending ? (
                  <PendingState
                    title="Processing recent decisions"
                    body="Rule evaluate calls were received. Decision rows will appear as soon as they are ready."
                  />
                ) : (
                  <EmptyState
                    title="No decisions yet"
                    body="Recent rule decisions will appear after /routing/evaluate calls in this window."
                  />
                )}
              </CardBody>
            </Card>

            <div className="grid h-full gap-5 xl:grid-rows-2">
              <Card className="!rounded-2xl h-full overflow-visible">
                <CardHeader>
                  <div>
                    <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Gateway activity</h2>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
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
                            <p className="text-[13px] font-medium text-slate-900 dark:text-white leading-[18px]">{item.gateway}</p>
                            <p className="text-[13px] font-semibold text-slate-500 dark:text-[#8a8a93] leading-[18px]">{item.count}</p>
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
                      body="Recent rule decisions are still being processed. Gateway activity will appear automatically once rows are available."
                    />
                  ) : (
                    <EmptyState
                      title="No gateway activity yet"
                      body="Once rule decisions are captured, this panel will show which connectors are being selected."
                    />
                  )}
                </CardBody>
              </Card>

              <Card className="!rounded-2xl h-full overflow-visible">
                <CardHeader>
                  <div>
                    <h2 className="text-[13px] font-semibold text-slate-800 dark:text-white leading-[18px]">Decision outcomes</h2>
                    <p className="mt-2 text-[13px] text-slate-500 dark:text-[#8a8a93] leading-[18px]">
                      How decisions resolved — rule match vs. default fallback.
                    </p>
                  </div>
                </CardHeader>
                <CardBody>
                  {previewStatusSummary.length ? (
                    <div className="space-y-5">
                      {previewStatusSummary.map((item) => {
                        const pct = previewStatusTotal ? (item.count / previewStatusTotal) * 100 : 0
                        const variant: BadgeVariant = item.status.toLowerCase().includes('fail') ? 'red' : item.status === 'default_selection' ? 'orange' : 'green'
                        const barColor = variant === 'green' ? '#22c55e' : variant === 'orange' ? '#f97316' : '#ef4444'
                        return (
                          <div key={item.status} className="space-y-2">
                            <div className="flex items-center justify-between gap-3">
                              <Badge variant={variant}>{item.status}</Badge>
                              <span className="text-[13px] font-semibold tabular-nums text-slate-900 dark:text-white leading-[18px]">
                                {item.count} <span className="font-normal text-slate-500 dark:text-[#8a8a93]">({formatPercent(pct)})</span>
                              </span>
                            </div>
                            <div className="h-1.5 overflow-hidden rounded-full bg-slate-100 dark:bg-[#141822]">
                              <div
                                className="h-full rounded-full"
                                style={{ width: `${pct}%`, backgroundColor: barColor }}
                              />
                            </div>
                          </div>
                        )
                      })}
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
