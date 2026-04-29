import type { ElementType, ReactNode } from 'react'
import { useEffect, useMemo, useState } from 'react'
import useSWR from 'swr'
import {
  Activity,
  AlertCircle,
  BarChart3,
  CheckCircle2,
  Clock3,
  GitBranch,
  Network,
  ShieldCheck,
  Sparkles,
  XCircle,
} from 'lucide-react'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiFetch, apiPost, fetcher } from '../../lib/api'
import {
  AnalyticsRange,
  AnalyticsOverviewResponse,
  AnalyticsRoutingStatsResponse,
  RoutingAlgorithm,
  RuleConfig,
} from '../../types/api'
import { Badge } from '../ui/Badge'
import { Card as GlassCard, SurfaceLabel } from '../ui/Card'
import { Spinner } from '../ui/Spinner'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'

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
  const [status, setStatus] = useState<'up' | 'down' | 'loading'>('loading')

  useEffect(() => {
    apiFetch<{ message: string }>('/health')
      .then(() => setStatus('up'))
      .catch(() => setStatus('down'))
  }, [])

  return status
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

function HeroStat({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail: string
}) {
  return (
    <div className="rounded-[22px] border border-slate-200 bg-white px-4 py-4 dark:border-[#2a303a] dark:bg-[#161b24]">
      <SurfaceLabel>{label}</SurfaceLabel>
      <p className="mt-3 text-2xl font-semibold tracking-tight text-slate-950 dark:text-white">
        {value}
      </p>
      <p className="mt-1 text-sm text-slate-500 dark:text-[#b2bdd1]">{detail}</p>
    </div>
  )
}

function MetricCard({
  icon: Icon,
  label,
  value,
  detail,
}: {
  icon: ElementType
  label: string
  value: ReactNode
  detail: string
}) {
  return (
    <GlassCard className="p-5">
      <div className="flex items-start justify-between gap-4">
        <div>
          <SurfaceLabel>{label}</SurfaceLabel>
          <p className="mt-4 text-3xl font-semibold tracking-tight text-slate-950 dark:text-white">
            {value}
          </p>
          <p className="mt-2 text-sm text-slate-500 dark:text-[#b2bdd1]">{detail}</p>
        </div>
        <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
          <Icon className="h-5 w-5 text-brand-600 dark:text-sky-300" />
        </div>
      </div>
    </GlassCard>
  )
}

function formatRouteLabel(route: string) {
  return route
    .replace(/_/g, '-')
    .replace(/^\/routing-evaluate$/, '/routing/evaluate')
    .replace(/^\/decide-gateway$/, '/decide-gateway')
    .replace(/^\/update-gateway$/, '/update-gateway')
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
          Analytics now derive merchant scope from your authenticated session. Once you are signed in,
          this page shows service health, active routing, request count, and gateway activity without
          needing analytics query params for merchant selection.
        </p>
      </GlassCard>

      <GlassCard className="p-7">
        <div className="space-y-5">
          {[
            {
              icon: Activity,
              title: 'System status',
              text: 'Check whether the service is reachable.',
            },
            {
              icon: GitBranch,
              title: 'Routing setup',
              text: 'See whether a strategy is configured.',
            },
            {
              icon: BarChart3,
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

function RefreshingState({ label }: { label: string }) {
  return (
    <div className="overflow-hidden rounded-[22px] border border-brand-500/20 bg-white shadow-[0_10px_30px_-24px_rgba(0,105,237,0.9)] dark:bg-[#0c0c0e]">
      <div className="h-2 w-full bg-brand-500/15">
        <div className="h-full origin-left animate-[analytics-progress_1.8s_ease-in-out_infinite] rounded-r-full bg-brand-500" />
      </div>
      <div className="flex items-center justify-between gap-3 px-4 py-3">
        <div className="flex items-center gap-2">
          <Spinner size={14} />
          <p className="text-sm font-medium text-slate-900 dark:text-white">{label}</p>
        </div>
        <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-brand-600 dark:text-brand-300">
          Loading
        </span>
      </div>
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
  const hasAuthRateConfig = Boolean(srConfig?.config?.data)
  const hasDebitRouting = debitRoutingFlag.isEnabled
  const totalRouteHits = routeHits.reduce((sum, item) => sum + item.count, 0)
  const topRouteHit = [...routeHits].sort((left, right) => right.count - left.count)[0] || null
  const topRouteShare = topRouteHit && totalRouteHits ? (topRouteHit.count / totalRouteHits) * 100 : 0
  const configuredBasics = [
    health === 'up',
    Boolean(activeRouting),
    hasAuthRateConfig,
    hasRuleBasedRouting,
    hasDebitRouting,
  ].filter(Boolean).length

  const setupItems = [
    {
      label: 'Service health',
      description: health === 'up' ? 'Service is reachable.' : 'Please verify service health.',
      state: health === 'up' ? 'Live' : health === 'down' ? 'Issue' : 'Checking',
      icon: health === 'up' ? CheckCircle2 : health === 'down' ? XCircle : AlertCircle,
    },
    {
      label: 'Routing strategy',
      description: activeRouting ? activeRouting.name : 'No active routing configured.',
      state: activeRouting ? 'Configured' : 'Not set',
      icon: GitBranch,
    },
    {
      label: 'Auth-rate config',
      description: hasAuthRateConfig ? 'Configured and available.' : 'Not configured yet.',
      state: hasAuthRateConfig ? 'Configured' : 'Not set',
      icon: ShieldCheck,
    },
    {
      label: 'Rule-based routing',
      description: hasRuleBasedRouting ? 'Enabled for this merchant.' : 'Not enabled.',
      state: hasRuleBasedRouting ? 'Enabled' : 'Optional',
      icon: Sparkles,
    },
    {
      label: 'Debit routing',
      description: debitRoutingFlag.isLoading
        ? 'Checking merchant debit-routing flag.'
        : hasDebitRouting
          ? 'Enabled for this merchant.'
          : 'Not enabled yet.',
      state: debitRoutingFlag.isLoading ? 'Checking' : hasDebitRouting ? 'Enabled' : 'Not set',
      icon: Network,
    },
  ]

  const analyticsLoading =
    (!analyticsOverview.data && analyticsOverview.isLoading) ||
    (!analyticsRouting.data && analyticsRouting.isLoading)
  const analyticsRefreshing =
    !analyticsLoading &&
    (analyticsOverview.isValidating || analyticsRouting.isValidating)

  return (
    <div className="space-y-6 px-5 sm:px-6 lg:px-8 xl:px-10">
      <header className="flex flex-wrap items-start justify-between gap-4">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Overview</h1>
            {(analyticsOverview.data?.merchant_id || effectiveMerchantId) ? (
              <Badge variant="blue">{analyticsOverview.data?.merchant_id || effectiveMerchantId}</Badge>
            ) : null}
          </div>
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
      </header>

      {!effectiveMerchantId ? (
        <EmptyWorkspace />
      ) : (
        <>
          {analyticsLoading ? (
            <div>
              <RefreshingState label={`Loading overview analytics for ${selectedWindow.detail.toLowerCase()}`} />
            </div>
          ) : null}

          {analyticsRefreshing ? (
            <div>
              <RefreshingState label={`Refreshing overview analytics for ${selectedWindow.detail.toLowerCase()}`} />
            </div>
          ) : null}

          <div className={`grid gap-5 xl:grid-cols-[1.15fr_0.85fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
              <GlassCard className="p-6 md:p-7">
                <div className="flex h-full flex-col justify-between">
                  <div>
                    <SurfaceLabel>Traffic leader</SurfaceLabel>
                    <div className="mt-5 flex flex-wrap items-end gap-4">
                      <h2 className="text-[2.5rem] font-semibold tracking-[-0.05em] text-slate-950 md:text-[3rem] dark:text-white">
                        {topGateway?.toUpperCase() || '--'}
                      </h2>
                      <div className="pb-2">
                        <p className="text-lg font-medium text-slate-700 dark:text-[#d5dded]">
                          {gatewayUsage[0] ? formatPercent(gatewayUsage[0].share) : '0%'}
                        </p>
                        <p className="mt-1 text-xs uppercase tracking-[0.16em] text-slate-500 dark:text-[#8390a7]">
                          Share in selected window
                        </p>
                      </div>
                    </div>
                  </div>

                  <div className="mt-8 grid gap-3 sm:grid-cols-3">
                    <HeroStat
                      label="Requests"
                      value={formatCompactNumber(decideHits)}
                      detail={selectedWindow.detail}
                    />
                    <HeroStat label="Setup ready" value={`${configuredBasics}/5`} detail="Core basics configured" />
                    <HeroStat label="Window" value={selectedWindow.label} detail={selectedWindow.detail} />
                  </div>
                </div>
              </GlassCard>

              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-1">
                <MetricCard
                  icon={GitBranch}
                  label="Primary traffic path"
                  value={topRouteHit ? formatRouteLabel(topRouteHit.route) : 'No traffic yet'}
                  detail={
                    topRouteHit
                      ? `${formatCompactNumber(topRouteHit.count)} requests · ${topRouteShare.toFixed(topRouteShare >= 100 ? 0 : 1)}% of tracked routes`
                      : 'No routing endpoint activity in this window'
                  }
                />
                <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-2">
                  <MetricCard
                    icon={Clock3}
                    label="Requests"
                    value={formatCompactNumber(decideHits)}
                    detail={selectedWindow.detail}
                  />
                  <MetricCard
                    icon={BarChart3}
                    label="Errors"
                    value={formatCompactNumber(totalErrors)}
                    detail={totalErrors ? 'Issues in selected window' : 'No issues in selected window'}
                  />
                </div>
              </div>
            </div>

            <div className={`grid gap-6 xl:grid-cols-[1.02fr_0.98fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
              <GlassCard className="p-6">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <SurfaceLabel>Current setup</SurfaceLabel>
                    <p className="mt-2 text-sm text-slate-600 dark:text-[#a6b0c3]">
                      The status cards you can explain in a demo without technical jargon.
                    </p>
                  </div>
                  <Badge variant={configuredBasics >= 3 ? 'green' : 'orange'}>
                    {configuredBasics}/5 ready
                  </Badge>
                </div>

                <div className="mt-5 grid gap-4 md:grid-cols-2">
                  {setupItems.map((item) => (
                    <GlassCard
                      key={item.label}
                      className="min-h-[158px] p-5"
                    >
                      <div className="flex h-full flex-col justify-between">
                        <div className="flex items-start justify-between gap-4">
                          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
                            <item.icon className="h-5 w-5 text-brand-600 dark:text-sky-300" />
                          </div>
                          <Badge
                            variant={
                              item.state === 'Live' || item.state === 'Configured' || item.state === 'Enabled'
                                ? 'green'
                                : item.state === 'Issue'
                                  ? 'red'
                                  : item.state === 'Checking' || item.state === 'Optional'
                                    ? 'gray'
                                    : 'orange'
                            }
                          >
                            {item.state}
                          </Badge>
                        </div>

                        <div className="mt-6">
                          <p className="text-sm font-semibold text-slate-950 dark:text-white">{item.label}</p>
                          <p className="mt-2 text-sm leading-6 text-slate-600 dark:text-[#a6b0c3]">
                            {item.description}
                          </p>
                        </div>
                      </div>
                    </GlassCard>
                  ))}
                </div>
              </GlassCard>

              <GlassCard className="p-6">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <SurfaceLabel>Gateway activity</SurfaceLabel>
                    <p className="mt-2 text-sm text-slate-600 dark:text-[#a6b0c3]">
                      Request distribution by gateway for the selected window.
                    </p>
                  </div>
                  <Badge variant="blue">{selectedWindow.badge}</Badge>
                </div>

                <div className="mt-6 space-y-4">
                  {gatewayUsage.length ? (
                    gatewayUsage.slice(0, 4).map((item, index) => (
                      <div
                        key={item.gateway}
                        className="rounded-[24px] border border-slate-200 bg-slate-50/80 p-4 dark:border-[#2a303a] dark:bg-[#121720]"
                      >
                        <div className="flex items-center justify-between gap-4">
                          <div className="flex items-center gap-3">
                            <span
                              className="h-2.5 w-2.5 rounded-full"
                              style={{
                                backgroundColor:
                                  ['#38bdf8', '#60a5fa', '#22c55e', '#f59e0b'][index] || '#38bdf8',
                              }}
                            />
                            <div>
                              <p className="text-sm font-semibold text-slate-950 dark:text-white">
                                {item.gateway.toUpperCase()}
                              </p>
                              <p className="mt-1 text-xs text-slate-500 dark:text-[#98a3b8]">
                                {formatCompactNumber(item.count)} requests
                              </p>
                            </div>
                          </div>
                          <p className="text-sm font-medium text-slate-950 dark:text-white">
                            {formatPercent(item.share)}
                          </p>
                        </div>

                        <div className="mt-4 h-2 rounded-full bg-slate-200 dark:bg-[#232933]">
                          <div
                            className="h-full rounded-full bg-gradient-to-r from-sky-400 via-blue-500 to-cyan-300"
                            style={{ width: `${Math.max(10, item.share)}%` }}
                          />
                        </div>
                      </div>
                    ))
                  ) : (
                    <div className="rounded-[24px] border border-dashed border-white/10 px-5 py-10 text-center">
                      <p className="text-sm font-semibold text-slate-950 dark:text-white">
                        No gateway activity yet
                      </p>
                      <p className="mt-2 text-sm leading-6 text-slate-600 dark:text-[#a6b0c3]">
                        Once requests start flowing, this section will show traffic by gateway.
                      </p>
                    </div>
                  )}
                </div>
              </GlassCard>
            </div>

            <div className={`transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
              <GlassCard className="p-6">
                <SurfaceLabel>Quick summary</SurfaceLabel>
                <div className="mt-5 space-y-4">
                  {[
                    { label: 'Signed-in merchant', value: analyticsOverview.data?.merchant_id || effectiveMerchantId || '--' },
                    { label: 'Time window', value: selectedWindow.detail },
                    { label: selectedWindow.summaryLabel, value: formatCompactNumber(totalErrors) },
                    { label: 'Top gateway', value: topGateway?.toUpperCase() || 'No activity' },
                  ].map((item) => (
                    <div
                      key={item.label}
                      className="flex items-center justify-between gap-4 rounded-[20px] border border-slate-200 bg-slate-50/80 px-4 py-3 dark:border-[#2a303a] dark:bg-[#121720]"
                    >
                      <span className="text-sm text-slate-600 dark:text-[#a6b0c3]">{item.label}</span>
                      <span className="text-sm font-semibold text-slate-950 dark:text-white">
                        {item.value}
                      </span>
                    </div>
                  ))}
                </div>
              </GlassCard>
            </div>
          </>
        )}
    </div>
  )
}
