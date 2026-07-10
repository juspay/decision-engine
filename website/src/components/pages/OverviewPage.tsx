import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import useSWR from 'swr'
import {
  BookOpen,
  CheckCircle2,
  ChevronRight,
  GitBranch,
  Network,
  TrendingUp,
} from 'lucide-react'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import {
  AnalyticsRange,
  AnalyticsOverviewResponse,
  AnalyticsRoutingStatsResponse,
  RoutingAlgorithm,
  RuleConfig,
} from '../../types/api'
import { Badge } from '../ui/Badge'
import { Card as GlassCard, SurfaceLabel } from '../ui/Card'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'

const OVERVIEW_RANGE_OPTIONS: {
  value: AnalyticsRange
  label: string
  detail: string
  badge: string
  summaryLabel: string
}[] = [
  { value: '15m', label: '15m', detail: 'Last 15 mins', badge: 'Live 15m', summaryLabel: 'Errors last 15 mins' },
  { value: '1h', label: '1h', detail: 'Last hour', badge: 'Live 1h', summaryLabel: 'Errors last hour' },
  { value: '12h', label: '12h', detail: 'Last 12 hours', badge: 'Live 12h', summaryLabel: 'Errors last 12 hours' },
  { value: '1d', label: '1 day', detail: 'Last 1 day', badge: 'Live 1d', summaryLabel: 'Errors last 1 day' },
  { value: '1w', label: '1 week', detail: 'Last 1 week', badge: 'Live 1w', summaryLabel: 'Errors last 1 week' },
]

function useHealth() {
  const { data, error } = useSWR<{ message: string }>(
    '/health',
    fetcher,
    { revalidateOnFocus: false, shouldRetryOnError: false },
  )
  if (data) return 'up' as const
  if (error) return 'down' as const
  return 'loading' as const
}

function formatCompactNumber(value: number | undefined) {
  return new Intl.NumberFormat(undefined, {
    notation: 'compact',
    maximumFractionDigits: value && value < 100 ? 1 : 0,
  }).format(value || 0)
}

function formatPercent(value: number | undefined) {
  if (value === undefined || value === null || Number.isNaN(value)) return '0%'
  return `${value.toFixed(value >= 100 ? 0 : 1)}%`
}

function healthLabel(status: 'up' | 'down' | 'loading') {
  if (status === 'up') return 'Live'
  if (status === 'down') return 'Needs attention'
  return 'Checking'
}

function timeAgo(ms: number) {
  const diff = Date.now() - ms
  if (diff < 60_000) return 'just now'
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`
  return `${Math.floor(diff / 86_400_000)}d ago`
}

function scoreColor(score: number) {
  if (score >= 0.85) return { dot: 'bg-emerald-500', text: 'text-emerald-600 dark:text-emerald-400' }
  if (score >= 0.70) return { dot: 'bg-amber-500', text: 'text-amber-600 dark:text-amber-400' }
  return { dot: 'bg-red-500', text: 'text-red-600 dark:text-red-400' }
}

function EmptyWorkspace() {
  return (
    <div className="grid gap-5 lg:grid-cols-[1.1fr_0.9fr]">
      <GlassCard className="p-7">
        <SurfaceLabel>Merchant session required</SurfaceLabel>
        <h2 className="mt-4 max-w-xl text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">
          Sign in with a merchant account to turn this into a live overview.
        </h2>
        <p className="mt-4 max-w-xl text-sm leading-7 text-slate-600 dark:text-[#b2bdd1]">
          Analytics use your signed-in merchant account. After sign-in, the overview shows service
          health, active routing, request count, and gateway activity.
        </p>
      </GlassCard>

      <GlassCard className="p-7">
        <div className="space-y-5">
          {[
            {
              icon: CheckCircle2,
              title: 'System status',
              text: 'Check whether the service is reachable.',
            },
            {
              icon: GitBranch,
              title: 'Routing setup',
              text: 'See whether a strategy is configured.',
            },
            {
              icon: Network,
              title: 'Gateway activity',
              text: 'View recent request distribution by gateway.',
            },
          ].map((item) => (
            <div key={item.title} className="flex items-start gap-4">
              <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
                <item.icon className="h-5 w-5 text-brand-600 dark:text-sky-300" />
              </div>
              <div>
                <p className="text-sm font-semibold text-slate-950 dark:text-white">{item.title}</p>
                <p className="mt-1 text-sm leading-6 text-slate-600 dark:text-[#b2bdd1]">{item.text}</p>
              </div>
            </div>
          ))}
        </div>
      </GlassCard>
    </div>
  )
}


type RuleConfigResponse = {
  merchant_id: string
  config: RuleConfig
}

export function OverviewPage() {
  const { merchantId } = useMerchantStore()
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const effectiveMerchantId = merchantId || authMerchantId
  const health = useHealth()
  const [range, setRange] = useState<AnalyticsRange>('1d')

  const { data: activeAlgorithms } = useSWR<RoutingAlgorithm[]>(
    effectiveMerchantId ? `/routing/list/active/${effectiveMerchantId}` : null,
    () => apiPost<RoutingAlgorithm[]>(`/routing/list/active/${effectiveMerchantId}`),
    { shouldRetryOnError: false },
  )

  const { data: srConfig } = useSWR<RuleConfigResponse>(
    effectiveMerchantId ? ['/rule/get', 'successRate', effectiveMerchantId] : null,
    () => apiPost('/rule/get', { merchant_id: effectiveMerchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false },
  )
  const debitRoutingFlag = useDebitRoutingFlag(effectiveMerchantId)
  const merchantFeatures = useMerchantFeatures(effectiveMerchantId || undefined)

  const analyticsOverviewUrl = `/analytics/overview?range=${range}`
  const analyticsRoutingUrl = `/analytics/routing-stats?range=${range}`

  const analyticsOverview = useSWR<AnalyticsOverviewResponse>(analyticsOverviewUrl, fetcher, {
    shouldRetryOnError: false,
    keepPreviousData: true,
  })
  const analyticsRouting = useSWR<AnalyticsRoutingStatsResponse>(analyticsRoutingUrl, fetcher, {
    shouldRetryOnError: false,
    keepPreviousData: true,
  })

  const activeRouting = activeAlgorithms?.[0] || null
  const hasRuleBasedRouting = (activeAlgorithms || []).some(
    (algorithm) => (algorithm.algorithm_data || algorithm.algorithm)?.type === 'advanced',
  )

  const routeHits = analyticsOverview.data?.route_hits || []
  const decideHits = routeHits.find((item) => item.route === '/decide_gateway')?.count || 0
  const totalErrors =
    analyticsOverview.data?.top_errors?.reduce((sum, item) => sum + item.count, 0) || 0

  const topErrors = analyticsOverview.data?.top_errors || []

  const gatewayScores = useMemo(() => {
    const map = new Map<string, { scoreSum: number; txSum: number }>()
    for (const s of analyticsOverview.data?.top_scores || []) {
      const e = map.get(s.gateway) ?? { scoreSum: 0, txSum: 0 }
      e.scoreSum += s.score_value * s.transaction_count
      e.txSum += s.transaction_count
      map.set(s.gateway, e)
    }
    return Array.from(map.entries())
      .map(([gateway, e]) => ({
        gateway,
        score: e.txSum > 0 ? e.scoreSum / e.txSum : 0,
        txCount: e.txSum,
      }))
      .sort((a, b) => b.txCount - a.txCount)
  }, [analyticsOverview.data])

  const gatewayUsage = useMemo(() => {
    const totals = new Map<string, number>()

    for (const point of analyticsRouting.data?.gateway_share || []) {
      totals.set(point.gateway, (totals.get(point.gateway) || 0) + point.count)
    }

    const totalTraffic = Array.from(totals.values()).reduce((sum, count) => sum + count, 0)

    return Array.from(totals.entries())
      .map(([gateway, count]) => ({
        gateway,
        count,
        share: totalTraffic ? (count / totalTraffic) * 100 : 0,
      }))
      .sort((left, right) => right.count - left.count)
  }, [analyticsRouting.data])

  const topGateway = gatewayUsage[0]?.gateway || analyticsOverview.data?.top_scores?.[0]?.gateway
  const selectedWindow =
    OVERVIEW_RANGE_OPTIONS.find((option) => option.value === range) || OVERVIEW_RANGE_OPTIONS[1]
  // Multi-objective is set up when either the merchant saved a manual successRate
  // config on /routing/sr (a merchant-specific /rule/get response — the same
  // condition that page uses) or turned on Autopilot mode there (the `autopilot`
  // merchant feature flag). A global default config alone counts as neither.
  const hasMultiObjectiveConfig = Boolean(
    srConfig?.config?.type === 'successRate' &&
      srConfig.config.data &&
      srConfig.merchant_id === effectiveMerchantId,
  )
  const autopilotEnabled = merchantFeatures.isEnabled('autopilot')
  const multiObjectiveReady = hasMultiObjectiveConfig || autopilotEnabled
  const hasDebitRouting = debitRoutingFlag.isEnabled
  const configuredBasics = [
    Boolean(activeRouting),
    multiObjectiveReady,
    hasRuleBasedRouting,
    hasDebitRouting,
  ].filter(Boolean).length

  const setupItems = [
    {
      label: 'Routing strategy',
      description: activeRouting ? activeRouting.name : 'None configured',
      state: activeRouting ? 'Configured' : 'Not set',
      icon: GitBranch,
      required: true,
      href: '../routing',
    },
    {
      label: 'Multi-objective config',
      description: autopilotEnabled
        ? 'Autopilot mode enabled'
        : hasMultiObjectiveConfig
          ? 'Configured'
          : 'Not configured',
      state: autopilotEnabled ? 'Auto-pilot' : hasMultiObjectiveConfig ? 'Configured' : 'Not set',
      icon: TrendingUp,
      required: true,
      href: '../routing/sr',
    },
    {
      label: 'Rule-based routing',
      description: hasRuleBasedRouting ? 'Enabled' : 'Not enabled',
      state: hasRuleBasedRouting ? 'Enabled' : 'Optional',
      icon: BookOpen,
      required: false,
      href: '../routing/rules',
    },
    {
      label: 'Debit routing',
      description: debitRoutingFlag.isLoading
        ? 'Checking…'
        : hasDebitRouting
          ? 'Enabled'
          : 'Not enabled',
      state: debitRoutingFlag.isLoading ? 'Checking' : hasDebitRouting ? 'Enabled' : 'Optional',
      icon: Network,
      required: false,
      href: '../routing/debit',
    },
  ]

  const analyticsLoading =
    (!analyticsOverview.data && analyticsOverview.isLoading) ||
    (!analyticsRouting.data && analyticsRouting.isLoading)
  const analyticsRefreshing =
    !analyticsLoading &&
    (analyticsOverview.isValidating || analyticsRouting.isValidating)

  const gatewayColors = ['#38bdf8', '#60a5fa', '#22c55e', '#f59e0b']

  return (
    <div className="space-y-6 px-5 sm:px-6 lg:px-8 xl:px-10">
      <header className="relative flex flex-wrap items-start justify-between gap-4">
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Overview</h1>
          {(analyticsOverview.data?.merchant_id || effectiveMerchantId) ? (
            <Badge variant="blue">{analyticsOverview.data?.merchant_id || effectiveMerchantId}</Badge>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2 md:justify-end">
          <span
            className={`inline-flex flex-shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium ${
              health === 'up'
                ? 'border-emerald-300/35 bg-emerald-500/12 text-emerald-700 dark:border-emerald-400/35 dark:bg-emerald-500/15 dark:text-emerald-200'
                : health === 'down'
                  ? 'border-red-300/35 bg-red-500/12 text-red-700 dark:border-red-400/35 dark:bg-red-500/15 dark:text-red-200'
                  : 'border-amber-300/35 bg-amber-500/12 text-amber-700 dark:border-amber-400/35 dark:bg-amber-500/15 dark:text-amber-200'
            }`}
          >
            <span className="relative inline-flex h-2.5 w-2.5 shrink-0 items-center justify-center">
              <span
                className={`absolute h-2 w-2 rounded-full ${health === 'up' ? 'bg-emerald-500' : health === 'down' ? 'bg-red-500' : 'bg-amber-500'} ${
                  health === 'up' ? 'animate-ping' : ''
                }`}
              />
              <span
                className={`relative h-2 w-2 rounded-full ${health === 'up' ? 'bg-emerald-500' : health === 'down' ? 'bg-red-500' : 'bg-amber-500'}`}
              />
            </span>
            {healthLabel(health)}
          </span>

          <div className="inline-flex rounded-2xl border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d]">
            {OVERVIEW_RANGE_OPTIONS.map((option) => {
              const active = option.value === range
              return (
                <button
                  key={option.value}
                  type="button"
                  onClick={() => setRange(option.value)}
                  className={`rounded-[14px] px-3 py-2 text-xs font-semibold transition ${
                    active
                      ? 'bg-white text-slate-950 shadow-sm dark:bg-[#1a2332] dark:text-white'
                      : 'text-slate-500 hover:text-slate-900 dark:text-[#8ea0bb] dark:hover:text-white'
                  }`}
                >
                  {option.label}
                </button>
              )
            })}
          </div>
        </div>

        {/* loading bar — sits at the bottom edge of the header, no layout shift */}
        <div className={`absolute bottom-0 left-0 right-0 h-[2px] overflow-hidden transition-opacity duration-500 ${analyticsLoading || analyticsRefreshing ? 'opacity-100' : 'opacity-0'}`}>
          <div className="h-full origin-left animate-[analytics-progress_1.8s_ease-in-out_infinite] bg-brand-500" />
        </div>
      </header>

      {!effectiveMerchantId ? (
        <EmptyWorkspace />
      ) : (
        <>

          {/* ── top stat row ─────────────────────────────────────── */}
          <div className={`grid gap-4 sm:grid-cols-3 transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
            <GlassCard className="p-5">
              <SurfaceLabel>Requests</SurfaceLabel>
              <p className="mt-3 text-[2rem] font-semibold leading-none tracking-tight text-slate-950 dark:text-white">
                {formatCompactNumber(decideHits)}
              </p>
              <p className="mt-2 text-xs text-slate-500 dark:text-[#8390a7]">
                /decide-gateway · {selectedWindow.detail.toLowerCase()}
              </p>
            </GlassCard>

            <GlassCard className={`p-5 transition-colors ${totalErrors > 0 ? 'border-red-300/60 dark:border-red-500/30' : ''}`}>
              <SurfaceLabel>Errors</SurfaceLabel>
              <p className={`mt-3 text-[2rem] font-semibold leading-none tracking-tight ${totalErrors > 0 ? 'text-red-600 dark:text-red-400' : 'text-slate-950 dark:text-white'}`}>
                {formatCompactNumber(totalErrors)}
              </p>
              <p className="mt-2 text-xs text-slate-500 dark:text-[#8390a7]">
                {totalErrors > 0 ? 'Issues detected in window' : 'No issues in window'}
              </p>
            </GlassCard>

            <GlassCard className="p-5">
              <SurfaceLabel>Top gateway</SurfaceLabel>
              <p className="mt-3 text-[2rem] font-semibold leading-none tracking-tight text-slate-950 dark:text-white">
                {topGateway?.toUpperCase() || '—'}
              </p>
              <p className="mt-2 text-xs text-slate-500 dark:text-[#8390a7]">
                {gatewayUsage[0] ? `${formatPercent(gatewayUsage[0].share)} of traffic` : 'No activity yet'}
              </p>
            </GlassCard>
          </div>

          {/* ── main content ─────────────────────────────────────── */}
          <div className={`grid gap-6 xl:grid-cols-[1.1fr_0.9fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>

            {/* Gateway activity */}
            <GlassCard className="p-6">
              <div className="flex items-center justify-between gap-4">
                <SurfaceLabel>Gateway activity</SurfaceLabel>
                <Badge variant="blue">{selectedWindow.badge}</Badge>
              </div>

              <div className="mt-6 space-y-3">
                {gatewayUsage.length ? (
                  gatewayUsage.slice(0, 4).map((item, index) => (
                    <div
                      key={item.gateway}
                      className="rounded-[20px] border border-slate-200 bg-slate-50/80 p-4 dark:border-[#2a303a] dark:bg-[#121720]"
                    >
                      <div className="flex items-center justify-between gap-4">
                        <div className="flex items-center gap-3">
                          <span
                            className="h-2.5 w-2.5 flex-shrink-0 rounded-full"
                            style={{ backgroundColor: gatewayColors[index] || gatewayColors[0] }}
                          />
                          <div>
                            <p className="text-sm font-semibold text-slate-950 dark:text-white">
                              {item.gateway.toUpperCase()}
                            </p>
                            <p className="mt-0.5 text-xs text-slate-500 dark:text-[#98a3b8]">
                              {formatCompactNumber(item.count)} requests
                            </p>
                          </div>
                        </div>
                        <p className="text-sm font-semibold tabular-nums text-slate-950 dark:text-white">
                          {formatPercent(item.share)}
                        </p>
                      </div>

                      <div className="mt-3 h-1.5 rounded-full bg-slate-200 dark:bg-[#232933]">
                        <div
                          className="h-full rounded-full transition-all duration-500"
                          style={{
                            width: `${Math.max(4, item.share)}%`,
                            backgroundColor: gatewayColors[index] || gatewayColors[0],
                          }}
                        />
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="rounded-[20px] border border-dashed border-slate-200 px-5 py-12 text-center dark:border-[#2a303a]">
                    <p className="text-sm font-semibold text-slate-950 dark:text-white">
                      No gateway activity yet
                    </p>
                    <p className="mt-2 text-sm leading-6 text-slate-500 dark:text-[#a6b0c3]">
                      Traffic by gateway will appear here once requests flow through.
                    </p>
                  </div>
                )}
              </div>
            </GlassCard>

            {/* Setup */}
            <GlassCard className="p-6">
              <div className="flex items-center justify-between gap-4">
                <SurfaceLabel>Setup</SurfaceLabel>
                <Badge variant={configuredBasics >= 2 ? 'green' : 'orange'}>
                  {configuredBasics}/4 ready
                </Badge>
              </div>

              {/* Progress bar */}
              <div className="mt-4 h-1.5 overflow-hidden rounded-full bg-slate-200 dark:bg-[#232933]">
                <div
                  className="h-full rounded-full bg-emerald-500 transition-all duration-500"
                  style={{ width: `${(configuredBasics / 4) * 100}%` }}
                />
              </div>

              {/* Checklist */}
              <div className="mt-4 divide-y divide-slate-100 dark:divide-[#1e2535]">
                {setupItems.map((item) => {
                  const readyState =
                    item.state === 'Live' ||
                    item.state === 'Configured' ||
                    item.state === 'Enabled' ||
                    item.state === 'Auto-pilot'
                  const iconColor =
                    item.state === 'Issue'
                      ? 'text-red-500'
                      : readyState
                        ? 'text-emerald-500'
                        : 'text-slate-400 dark:text-[#5a6a82]'
                  const badgeVariant = readyState
                    ? 'green'
                    : item.state === 'Issue'
                      ? 'red'
                      : item.state === 'Checking'
                        ? 'gray'
                        : item.required
                          ? 'orange'
                          : 'gray'
                  const inner = (
                    <>
                      <div className="flex items-center gap-3 min-w-0">
                        <item.icon className={`h-4 w-4 flex-shrink-0 ${iconColor}`} />
                        <div className="min-w-0">
                          <p className="text-sm font-medium text-slate-900 dark:text-white">
                            {item.label}
                          </p>
                          <p className="truncate text-xs text-slate-500 dark:text-[#8390a7]">
                            {item.description}
                          </p>
                        </div>
                      </div>
                      <div className="flex flex-shrink-0 items-center gap-1.5">
                        <Badge variant={badgeVariant}>{item.state}</Badge>
                        {item.href && (
                          <ChevronRight className="h-3.5 w-3.5 text-slate-400 dark:text-[#5a6a82]" />
                        )}
                      </div>
                    </>
                  )
                  const rowClass = 'flex items-center justify-between gap-3 py-3.5 rounded-lg -mx-2 px-2'
                  return item.href ? (
                    <Link
                      key={item.label}
                      to={item.href}
                      relative="path"
                      className={`${rowClass} transition-colors hover:bg-slate-50 dark:hover:bg-[#13192a]`}
                    >
                      {inner}
                    </Link>
                  ) : (
                    <div key={item.label} className={rowClass}>
                      {inner}
                    </div>
                  )
                })}
              </div>
            </GlassCard>
          </div>

          {/* ── gateway health + errors ───────────────────────────── */}
          <div className={`grid gap-6 xl:grid-cols-2 transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>

            {/* Gateway health */}
            <GlassCard className="p-6">
              <div className="flex items-center justify-between gap-4">
                <SurfaceLabel>Gateway health</SurfaceLabel>
                <Badge variant="blue">{selectedWindow.badge}</Badge>
              </div>

              <div className="mt-5 space-y-2">
                {gatewayScores.length ? (
                  gatewayScores.slice(0, 5).map((gw) => {
                    const color = scoreColor(gw.score)
                    return (
                      <div
                        key={gw.gateway}
                        className="flex items-center gap-3 rounded-xl border border-slate-200 bg-slate-50/80 px-4 py-3 dark:border-[#2a303a] dark:bg-[#121720]"
                      >
                        <span className={`h-2 w-2 flex-shrink-0 rounded-full ${color.dot}`} />
                        <span className="min-w-0 flex-1 truncate text-sm font-semibold text-slate-950 dark:text-white">
                          {gw.gateway.toUpperCase()}
                        </span>
                        <span className={`text-sm font-semibold tabular-nums ${color.text}`}>
                          {(gw.score * 100).toFixed(1)}%
                        </span>
                      </div>
                    )
                  })
                ) : (
                  <div className="rounded-xl border border-dashed border-slate-200 px-5 py-10 text-center dark:border-[#2a303a]">
                    <p className="text-sm font-semibold text-slate-950 dark:text-white">No score data yet</p>
                    <p className="mt-1.5 text-xs text-slate-500 dark:text-[#a6b0c3]">
                      SR scores appear once gateways handle traffic.
                    </p>
                  </div>
                )}
              </div>
            </GlassCard>

            {/* Recent errors */}
            <GlassCard className={`p-6 transition-colors ${totalErrors > 0 ? 'border-red-300/40 dark:border-red-500/20' : ''}`}>
              <div className="flex items-center justify-between gap-4">
                <SurfaceLabel>Recent errors</SurfaceLabel>
                {totalErrors > 0 ? (
                  <Badge variant="red">{formatCompactNumber(totalErrors)} total</Badge>
                ) : (
                  <Badge variant="green">Clean</Badge>
                )}
              </div>

              {topErrors.length ? (
                <div className="mt-4 divide-y divide-slate-100 dark:divide-[#1e2535]">
                  {topErrors.slice(0, 5).map((err, index) => (
                    <div key={index} className="flex items-start justify-between gap-3 py-3">
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="rounded bg-red-50 px-1.5 py-0.5 font-mono text-[11px] font-medium text-red-700 dark:bg-red-500/10 dark:text-red-400">
                            {err.error_code}
                          </span>
                          <span className="truncate text-xs text-slate-500 dark:text-[#8390a7]">
                            {err.route}
                          </span>
                        </div>
                        <p className="mt-1 truncate text-xs text-slate-500 dark:text-[#8390a7]">
                          {err.error_message}
                        </p>
                      </div>
                      <div className="flex-shrink-0 text-right">
                        <p className="text-sm font-semibold tabular-nums text-slate-950 dark:text-white">
                          {formatCompactNumber(err.count)}
                        </p>
                        <p className="text-[11px] text-slate-400 dark:text-[#5a6a82]">
                          {timeAgo(err.last_seen_ms)}
                        </p>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="mt-5 flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-200 px-5 py-10 text-center dark:border-[#2a303a]">
                  <CheckCircle2 className="h-8 w-8 text-emerald-500" />
                  <p className="mt-3 text-sm font-semibold text-slate-950 dark:text-white">No errors in window</p>
                  <p className="mt-1.5 text-xs text-slate-500 dark:text-[#a6b0c3]">
                    All requests resolved without errors.
                  </p>
                </div>
              )}
            </GlassCard>
          </div>
        </>
      )}
    </div>
  )
}
