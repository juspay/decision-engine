import type { ElementType } from 'react'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import useSWR from 'swr'
import {
  Activity,
  AlertCircle,
  ArrowRight,
  BarChart3,
  CheckCircle2,
  Clock3,
  GitBranch,
  ShieldCheck,
  Sparkles,
  XCircle,
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
import { Spinner } from '../ui/Spinner'

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
    fetch('/health')
      .then((response) => setStatus(response.ok ? 'up' : 'down'))
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
  if (status === 'up') return 'Healthy'
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
  value: string
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

function EmptyWorkspace() {
  return (
    <div className="grid gap-5 pt-8 lg:grid-cols-[1.1fr_0.9fr]">
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

export function OverviewPage() {
  const navigate = useNavigate()
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

  const { data: srConfig } = useSWR<RuleConfig>(
    effectiveMerchantId ? ['/rule/get', 'successRate', effectiveMerchantId] : null,
    () => apiPost('/rule/get', { merchant_id: effectiveMerchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false },
  )

  const analyticsOverviewUrl = `/analytics/overview?range=${range}`
  const analyticsRoutingUrl = `/analytics/routing-stats?range=${range}`

  const analyticsOverview = useSWR<AnalyticsOverviewResponse>(analyticsOverviewUrl, fetcher, {
    refreshInterval: 15000,
    revalidateOnFocus: true,
    shouldRetryOnError: false,
    keepPreviousData: true,
  })
  const analyticsRouting = useSWR<AnalyticsRoutingStatsResponse>(analyticsRoutingUrl, fetcher, {
    refreshInterval: 15000,
    revalidateOnFocus: true,
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
  const configuredBasics = [
    health === 'up',
    Boolean(activeRouting),
    Boolean(srConfig?.data),
    hasRuleBasedRouting,
  ].filter(Boolean).length

  const setupItems = [
    {
      label: 'Service health',
      description: health === 'up' ? 'Service is reachable.' : 'Please verify service health.',
      state: health === 'up' ? 'Healthy' : health === 'down' ? 'Issue' : 'Checking',
      icon: health === 'up' ? CheckCircle2 : health === 'down' ? XCircle : AlertCircle,
      route: undefined,
    },
    {
      label: 'Routing strategy',
      description: activeRouting ? activeRouting.name : 'No active routing configured.',
      state: activeRouting ? 'Configured' : 'Not set',
      icon: GitBranch,
      route: '/routing',
    },
    {
      label: 'Auth-rate config',
      description: srConfig?.data ? 'Configured and available.' : 'Not configured yet.',
      state: srConfig?.data ? 'Configured' : 'Not set',
      icon: ShieldCheck,
      route: '/routing/sr',
    },
    {
      label: 'Rule-based routing',
      description: hasRuleBasedRouting ? 'Enabled for this merchant.' : 'Not enabled.',
      state: hasRuleBasedRouting ? 'Enabled' : 'Optional',
      icon: Sparkles,
      route: '/routing/rules',
    },
  ]

  const workspaceBadge = !effectiveMerchantId
    ? { label: 'Merchant not selected', variant: 'orange' as const }
    : health === 'up'
      ? { label: 'System live', variant: 'green' as const }
      : health === 'down'
        ? { label: 'Attention needed', variant: 'red' as const }
        : { label: 'Checking status', variant: 'gray' as const }
  const analyticsLoading =
    (!analyticsOverview.data && analyticsOverview.isLoading) ||
    (!analyticsRouting.data && analyticsRouting.isLoading)
  const analyticsRefreshing =
    !analyticsLoading &&
    (analyticsOverview.isValidating || analyticsRouting.isValidating)

  return (
    <div className="relative mx-auto max-w-[1380px]">
      <div className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
        <div className="absolute -left-16 top-0 h-72 w-72 rounded-full bg-sky-500/10 blur-3xl dark:bg-sky-500/8" />
        <div className="absolute right-0 top-12 h-80 w-80 rounded-full bg-brand-500/10 blur-3xl dark:bg-brand-500/10" />
      </div>

      <section className="relative overflow-hidden rounded-[40px] border border-slate-200 bg-white px-5 py-5 shadow-[0_28px_90px_-56px_rgba(15,23,42,0.16)] md:px-6 md:py-6 dark:border-[#232933] dark:bg-[#090c12] dark:shadow-[0_28px_90px_-56px_rgba(0,0,0,0.72)]">
        <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-[#3b82f6]/25 to-transparent dark:via-[#3b82f6]/35" />

        <header className="relative flex flex-col gap-4 border-b border-slate-200 pb-5 dark:border-[#232933]">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={workspaceBadge.variant}>{workspaceBadge.label}</Badge>
            {(analyticsOverview.data?.merchant_id || effectiveMerchantId) ? (
              <Badge variant="blue">{analyticsOverview.data?.merchant_id || effectiveMerchantId}</Badge>
            ) : null}
          </div>
          <div>
            <h1 className="text-4xl font-semibold tracking-tight text-slate-950 md:text-[4rem] dark:text-white">
              Overview
            </h1>
            <p className="mt-2 max-w-2xl text-sm leading-7 text-slate-600 dark:text-[#a6b0c3]">
              Basic business-facing view of system status, setup, request volume, and gateway activity.
            </p>
            <div className="mt-4 inline-flex rounded-2xl border border-slate-200 bg-slate-50 p-1 dark:border-[#2a303a] dark:bg-[#121720]">
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
              <div className="pt-8">
                <RefreshingState label={`Loading overview analytics for ${selectedWindow.detail.toLowerCase()}`} />
              </div>
            ) : null}

            {analyticsRefreshing ? (
              <div className="pt-8">
                <RefreshingState label={`Refreshing overview analytics for ${selectedWindow.detail.toLowerCase()}`} />
              </div>
            ) : null}

            <div className={`grid gap-5 pt-8 xl:grid-cols-[1.15fr_0.85fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
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
                    <p className="mt-4 max-w-xl text-sm leading-7 text-slate-600 dark:text-[#a6b0c3]">
                      {activeRouting
                        ? `${activeRouting.name} is the current routing strategy for this merchant.`
                        : 'No active routing strategy is configured for this merchant yet.'}
                    </p>
                  </div>

                  <div className="mt-8 grid gap-3 sm:grid-cols-3">
                    <HeroStat
                      label="Requests"
                      value={formatCompactNumber(decideHits)}
                      detail={selectedWindow.detail}
                    />
                    <HeroStat label="Setup ready" value={`${configuredBasics}/4`} detail="Core basics configured" />
                    <HeroStat label="Window" value={selectedWindow.label} detail={selectedWindow.detail} />
                  </div>
                </div>
              </GlassCard>

              <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-1">
                <MetricCard
                  icon={Activity}
                  label="System status"
                  value={healthLabel(health)}
                  detail={health === 'up' ? 'Service is reachable' : 'Please verify service health'}
                />
                <MetricCard
                  icon={GitBranch}
                  label="Active routing"
                  value={activeRouting?.name || 'Not set'}
                  detail={activeRouting ? 'Currently selected strategy' : 'No routing configured yet'}
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
                    label="Top gateway"
                    value={topGateway?.toUpperCase() || '--'}
                    detail={gatewayUsage[0] ? `${formatPercent(gatewayUsage[0].share)} of traffic` : 'No activity yet'}
                  />
                </div>
              </div>
            </div>

            <div className={`mt-6 grid gap-6 xl:grid-cols-[1.02fr_0.98fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
              <GlassCard className="p-6">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <SurfaceLabel>Current setup</SurfaceLabel>
                    <p className="mt-2 text-sm text-slate-600 dark:text-[#a6b0c3]">
                      The status cards you can explain in a demo without technical jargon.
                    </p>
                  </div>
                  <Badge variant={configuredBasics >= 3 ? 'green' : 'orange'}>
                    {configuredBasics}/4 ready
                  </Badge>
                </div>

                <div className="mt-5 grid gap-4 md:grid-cols-2">
                  {setupItems.map((item) => (
                    <GlassCard
                      key={item.label}
                      className="min-h-[158px] p-5"
                      onClick={item.route ? () => navigate(item.route) : undefined}
                    >
                      <div className="flex h-full flex-col justify-between">
                        <div className="flex items-start justify-between gap-4">
                          <div className="rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
                            <item.icon className="h-5 w-5 text-brand-600 dark:text-sky-300" />
                          </div>
                          <Badge
                            variant={
                              item.state === 'Healthy' || item.state === 'Configured' || item.state === 'Enabled'
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

            <div className={`mt-6 grid gap-6 xl:grid-cols-[0.86fr_1.14fr] transition-opacity duration-200 ${analyticsRefreshing ? 'opacity-60' : 'opacity-100'}`}>
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

              <div className="grid gap-4 md:grid-cols-3">
                {[
                  {
                    label: 'Routing Hub',
                    text: 'Configure routing strategies.',
                    icon: GitBranch,
                    route: '/routing',
                  },
                  {
                    label: 'Analytics',
                    text: 'Inspect request and gateway trends.',
                    icon: BarChart3,
                    route: '/analytics',
                  },
                  {
                    label: 'Audit Trail',
                    text: 'Review individual decision records.',
                    icon: Clock3,
                    route: '/audit',
                  },
                ].map((item) => (
                  <GlassCard key={item.label} className="p-5" onClick={() => navigate(item.route)}>
                    <div className="flex h-full flex-col justify-between">
                      <div className="inline-flex w-fit rounded-2xl border border-slate-200 bg-slate-50 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
                        <item.icon className="h-5 w-5 text-brand-600 dark:text-sky-300" />
                      </div>
                      <div className="mt-10">
                        <p className="text-sm font-semibold text-slate-950 dark:text-white">
                          {item.label}
                        </p>
                        <p className="mt-2 text-sm leading-6 text-slate-600 dark:text-[#a6b0c3]">
                          {item.text}
                        </p>
                        <div className="mt-4 inline-flex items-center gap-2 text-sm font-medium text-brand-600 dark:text-sky-300">
                          <span>Open</span>
                          <ArrowRight className="h-4 w-4" />
                        </div>
                      </div>
                    </div>
                  </GlassCard>
                ))}
              </div>
            </div>
          </>
        )}
      </section>
    </div>
  )
}
