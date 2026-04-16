import { useState, useMemo, useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'
import useSWR from 'swr'
import {
  LineChart, Line, AreaChart, Area, XAxis, YAxis, CartesianGrid,
  Tooltip, Legend, ResponsiveContainer,
} from 'recharts'
import {
  Activity, TrendingUp, Zap, AlertTriangle, ArrowUpDown,
  ChevronDown, RefreshCw,
} from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useMerchantStore } from '../../store/merchantStore'
import { fetcher } from '../../lib/api'
import {
  USE_MOCK_DATA,
  mockKPI, mockGatewayScores, mockSRTimeSeries, mockDecisionsByApproach,
  mockGatewayShare, mockFeedbackDecisions, mockPriorityRules, mockFeedbackErrors,
  GATEWAY_COLORS, APPROACH_COLORS,
  type AnalyticsKPI, type GatewayScore, type TimeSeriesPoint,
  type DecisionSeries, type GatewaySharePoint, type FeedbackDecisionPoint,
  type PriorityRule, type FeedbackError,
} from '../../lib/mockAnalyticsData'

// ─── Constants ──────────────────────────────────────────────────────────────

const TIME_RANGES = [
  { label: '15m', value: '15m' },
  { label: '1h', value: '1h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
  { label: '7d', value: '7d' },
]

const GRANULARITIES = [
  { label: '10s', value: '10s' },
  { label: '1m', value: '1m' },
  { label: '5m', value: '5m' },
  { label: '1h', value: '1h' },
]

const PMT_OPTIONS = ['CARD', 'UPI', 'WALLET', 'NET_BANKING', 'PAY_LATER', 'BANK_TRANSFER']

const GATEWAY_OPTIONS = ['stripe', 'adyen', 'braintree', 'checkout_com', 'razorpay', 'worldpay']

const GATEWAYS = ['stripe', 'adyen', 'braintree', 'checkout_com', 'razorpay', 'worldpay']

const GRANULARITY_CLAMP: Record<string, string[]> = {
  '15m': ['10s', '1m'],
  '1h': ['10s', '1m', '5m'],
  '6h': ['1m', '5m', '1h'],
  '24h': ['5m', '1h'],
  '7d': ['1h'],
}

// ─── Helpers ────────────────────────────────────────────────────────────────

function formatTimestamp(ts: string): string {
  const d = new Date(ts)
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toFixed(n % 1 === 0 ? 0 : 1)
}

// ─── Filter Bar ─────────────────────────────────────────────────────────────

function MultiSelect({
  label, options, selected, onChange,
}: {
  label: string
  options: string[]
  selected: string[]
  onChange: (v: string[]) => void
}) {
  const [open, setOpen] = useState(false)

  const toggle = (v: string) => {
    onChange(selected.includes(v) ? selected.filter((s) => s !== v) : [...selected, v])
  }

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-medium
          bg-slate-100 dark:bg-[#151518] text-slate-700 dark:text-slate-300
          border border-slate-200 dark:border-[#1c1c1f] hover:border-brand-400 transition-colors"
      >
        {label}
        {selected.length > 0 && (
          <Badge variant="blue">{selected.length}</Badge>
        )}
        <ChevronDown size={12} />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute z-20 mt-1 w-52 rounded-xl bg-white dark:bg-[#0f0f11] border border-slate-200 dark:border-[#1c1c1f] shadow-lg p-2 space-y-0.5">
            {options.map((opt) => (
              <label
                key={opt}
                className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs cursor-pointer
                  hover:bg-slate-50 dark:hover:bg-[#151518] text-slate-700 dark:text-slate-300"
              >
                <input
                  type="checkbox"
                  checked={selected.includes(opt)}
                  onChange={() => toggle(opt)}
                  className="rounded border-slate-300 dark:border-[#2a2a2e] text-brand-500"
                />
                {opt}
              </label>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

function FilterBar({
  params, setParam,
}: {
  params: URLSearchParams
  setParam: (key: string, value: string) => void
}) {
  const range = params.get('range') || '1h'
  const granularity = params.get('granularity') || '1m'
  const merchants = (params.get('merchant') || '').split(',').filter(Boolean)
  const pmts = (params.get('pmt') || '').split(',').filter(Boolean)
  const gateways = (params.get('gateway') || '').split(',').filter(Boolean)
  const allowedGranularities = GRANULARITY_CLAMP[range] || ['1m']

  return (
    <div className="flex flex-wrap items-center gap-3">
      <MultiSelect
        label="Merchant"
        options={['merchant_001', 'merchant_002', 'merchant_003']}
        selected={merchants}
        onChange={(v) => setParam('merchant', v.join(','))}
      />
      <MultiSelect
        label="Payment Method"
        options={PMT_OPTIONS}
        selected={pmts}
        onChange={(v) => setParam('pmt', v.join(','))}
      />
      <MultiSelect
        label="Gateway"
        options={GATEWAY_OPTIONS}
        selected={gateways}
        onChange={(v) => setParam('gateway', v.join(','))}
      />

      <div className="h-6 w-px bg-slate-200 dark:bg-[#1c1c1f]" />

      {/* Time Range */}
      <div className="flex rounded-xl overflow-hidden border border-slate-200 dark:border-[#1c1c1f]">
        {TIME_RANGES.map((tr) => (
          <button
            key={tr.value}
            onClick={() => {
              setParam('range', tr.value)
              // auto-clamp granularity
              const allowed = GRANULARITY_CLAMP[tr.value] || ['1m']
              if (!allowed.includes(granularity)) {
                setParam('granularity', allowed[allowed.length - 1])
              }
            }}
            className={`px-3 py-1.5 text-xs font-medium transition-colors ${
              range === tr.value
                ? 'bg-brand-500 text-white'
                : 'bg-slate-50 dark:bg-[#0c0c0e] text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-[#151518]'
            }`}
          >
            {tr.label}
          </button>
        ))}
      </div>

      {/* Granularity */}
      <div className="flex rounded-xl overflow-hidden border border-slate-200 dark:border-[#1c1c1f]">
        {GRANULARITIES.map((g) => (
          <button
            key={g.value}
            disabled={!allowedGranularities.includes(g.value)}
            onClick={() => setParam('granularity', g.value)}
            className={`px-3 py-1.5 text-xs font-medium transition-colors ${
              granularity === g.value
                ? 'bg-brand-500 text-white'
                : !allowedGranularities.includes(g.value)
                  ? 'bg-slate-50 dark:bg-[#0c0c0e] text-slate-300 dark:text-[#333] cursor-not-allowed'
                  : 'bg-slate-50 dark:bg-[#0c0c0e] text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-[#151518]'
            }`}
          >
            {g.label}
          </button>
        ))}
      </div>
    </div>
  )
}

// ─── KPI Tile ───────────────────────────────────────────────────────────────

function KPITile({
  label, value, suffix, sparkline, icon: Icon, color,
}: {
  label: string
  value: string
  suffix?: string
  sparkline: number[]
  icon: React.ElementType
  color: string
}) {
  const max = Math.max(...sparkline)
  const min = Math.min(...sparkline)
  const range = max - min || 1
  const points = sparkline
    .map((v, i) => `${(i / (sparkline.length - 1)) * 60},${20 - ((v - min) / range) * 18}`)
    .join(' ')

  return (
    <Card>
      <CardBody className="flex items-start justify-between gap-3">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <Icon size={14} className={color} />
            <span className="text-[11px] font-medium text-slate-500 dark:text-[#66666e] uppercase tracking-wider">
              {label}
            </span>
          </div>
          <p className="text-2xl font-semibold text-slate-900 dark:text-white">
            {value}
            {suffix && <span className="text-sm font-normal text-slate-400 ml-1">{suffix}</span>}
          </p>
        </div>
        <svg width="60" height="22" className="mt-1 shrink-0">
          <polyline
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            className={color}
            points={points}
          />
        </svg>
      </CardBody>
    </Card>
  )
}

// ─── Sortable Table ─────────────────────────────────────────────────────────

type SortDir = 'asc' | 'desc'

function useSortable<T>(data: T[], defaultKey: keyof T) {
  const [sortKey, setSortKey] = useState<keyof T>(defaultKey)
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const sorted = useMemo(() => {
    return [...data].sort((a, b) => {
      const av = a[sortKey]
      const bv = b[sortKey]
      if (typeof av === 'number' && typeof bv === 'number') {
        return sortDir === 'asc' ? av - bv : bv - av
      }
      return sortDir === 'asc'
        ? String(av).localeCompare(String(bv))
        : String(bv).localeCompare(String(av))
    })
  }, [data, sortKey, sortDir])

  const onSort = useCallback((key: keyof T) => {
    if (key === sortKey) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }, [sortKey])

  return { sorted, sortKey, sortDir, onSort }
}

function SortHeader<T>({
  label, field, sortKey, sortDir, onSort,
}: {
  label: string
  field: keyof T
  sortKey: keyof T
  sortDir: SortDir
  onSort: (k: keyof T) => void
}) {
  return (
    <th
      className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e] cursor-pointer hover:text-slate-700 dark:hover:text-slate-300 select-none"
      onClick={() => onSort(field)}
    >
      <span className="inline-flex items-center gap-1">
        {label}
        {sortKey === field && (
          <ArrowUpDown size={10} className={sortDir === 'asc' ? 'rotate-180' : ''} />
        )}
      </span>
    </th>
  )
}

// ─── Scoreboard ─────────────────────────────────────────────────────────────

function PSPScoreboard({ data }: { data: GatewayScore[] }) {
  const { sorted, sortKey, sortDir, onSort } = useSortable(data, 'sr_score')

  const sparkForGateway = (gw: string) => {
    const base = (gw.charCodeAt(0) % 20) + 60
    return Array.from({ length: 12 }, () => Math.round(base + (Math.random() - 0.5) * 20))
  }

  return (
    <Card>
      <CardHeader>
        <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Live PSP Scoreboard</h3>
      </CardHeader>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-100 dark:border-[#1c1c1f]">
              <SortHeader<GatewayScore> label="Gateway" field="gateway" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<GatewayScore> label="SR Score" field="sr_score" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<GatewayScore> label="Elim Score" field="elimination_score" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<GatewayScore> label="Latency" field="latency_score" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<GatewayScore> label="Decisions" field="decisions" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<GatewayScore> label="Feedbacks" field="feedbacks" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Last Updated</th>
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Trend</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((row) => {
              const spark = sparkForGateway(row.gateway)
              const max = Math.max(...spark)
              const min = Math.min(...spark)
              const range = max - min || 1
              const pts = spark
                .map((v, i) => `${(i / (spark.length - 1)) * 50},${16 - ((v - min) / range) * 14}`)
                .join(' ')
              return (
                <tr key={row.gateway} className="border-b border-slate-50 dark:border-[#111] hover:bg-slate-50 dark:hover:bg-[#0c0c0e] transition-colors">
                  <td className="px-4 py-3 font-medium text-slate-900 dark:text-white">{row.gateway}</td>
                  <td className="px-4 py-3">
                    <Badge variant={row.sr_score >= 0.8 ? 'green' : row.sr_score >= 0.6 ? 'orange' : 'red'}>
                      {(row.sr_score * 100).toFixed(1)}%
                    </Badge>
                  </td>
                  <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{row.elimination_score.toFixed(2)}</td>
                  <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{row.latency_score.toFixed(2)}</td>
                  <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{formatNumber(row.decisions)}</td>
                  <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{formatNumber(row.feedbacks)}</td>
                  <td className="px-4 py-3 text-xs text-slate-400">{formatTimestamp(row.last_updated)}</td>
                  <td className="px-4 py-3">
                    <svg width="50" height="18">
                      <polyline fill="none" stroke={GATEWAY_COLORS[row.gateway] || '#6b7280'} strokeWidth="1.5" points={pts} />
                    </svg>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </Card>
  )
}

// ─── Priority Rules Table ───────────────────────────────────────────────────

function PriorityRulesTable({ data }: { data: PriorityRule[] }) {
  const { sorted, sortKey, sortDir, onSort } = useSortable(data, 'hits')

  return (
    <Card>
      <CardHeader>
        <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Top Priority-Logic Rules</h3>
      </CardHeader>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-100 dark:border-[#1c1c1f]">
              <SortHeader<PriorityRule> label="Rule" field="rule_name" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<PriorityRule> label="Hits" field="hits" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<PriorityRule> label="Last Hit" field="last_hit" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
              <SortHeader<PriorityRule> label="Gateway" field="gateway" sortKey={sortKey} sortDir={sortDir} onSort={onSort} />
            </tr>
          </thead>
          <tbody>
            {sorted.map((row) => (
              <tr key={row.rule_name} className="border-b border-slate-50 dark:border-[#111] hover:bg-slate-50 dark:hover:bg-[#0c0c0e] transition-colors">
                <td className="px-4 py-3 font-mono text-xs text-slate-900 dark:text-white">{row.rule_name}</td>
                <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{formatNumber(row.hits)}</td>
                <td className="px-4 py-3 text-xs text-slate-400">{formatTimestamp(row.last_hit)}</td>
                <td className="px-4 py-3"><Badge variant="blue">{row.gateway}</Badge></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  )
}

// ─── Feedback Errors Table ──────────────────────────────────────────────────

function FeedbackErrorsTable({ data }: { data: FeedbackError[] }) {
  return (
    <Card>
      <CardHeader>
        <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Recent Feedback Errors</h3>
      </CardHeader>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-100 dark:border-[#1c1c1f]">
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Time</th>
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Type</th>
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Message</th>
              <th className="px-4 py-3 text-left text-[11px] font-semibold uppercase tracking-wider text-slate-500 dark:text-[#66666e]">Gateway</th>
            </tr>
          </thead>
          <tbody>
            {data.map((row) => (
              <tr key={row.id} className="border-b border-slate-50 dark:border-[#111] hover:bg-slate-50 dark:hover:bg-[#0c0c0e] transition-colors">
                <td className="px-4 py-3 text-xs text-slate-400 whitespace-nowrap">{formatTimestamp(row.timestamp)}</td>
                <td className="px-4 py-3">
                  <Badge variant={row.error_type === 'DEAD_LETTER' ? 'red' : row.error_type === 'TIMEOUT' ? 'orange' : 'purple'}>
                    {row.error_type}
                  </Badge>
                </td>
                <td className="px-4 py-3 text-slate-600 dark:text-slate-400 max-w-md truncate">{row.message}</td>
                <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{row.gateway}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  )
}

// ─── Chart Wrapper ──────────────────────────────────────────────────────────

const chartAxisStyle = {
  fontSize: 10,
  fill: '#66666e',
}

// ─── Main Page ──────────────────────────────────────────────────────────────

export function AnalyticsPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const { merchantId } = useMerchantStore()

  const range = searchParams.get('range') || '1h'
  const granularity = searchParams.get('granularity') || '1m'

  const setParam = useCallback((key: string, value: string) => {
    setSearchParams((prev) => {
      const next = new URLSearchParams(prev)
      if (value) next.set(key, value)
      else next.delete(key)
      return next
    })
  }, [setSearchParams])

  const queryString = searchParams.toString()

  // ─── Data Fetching (SWR with 5s refresh) ────────────────────────────────

  const swrOpts = { refreshInterval: 5000, shouldRetryOnError: false }

  const { data: kpi } = useSWR<AnalyticsKPI>(
    USE_MOCK_DATA ? 'mock:kpi' : `/analytics/gateway-scores?${queryString}`,
    USE_MOCK_DATA ? mockKPI : fetcher,
    swrOpts,
  )

  const { data: gatewayScores } = useSWR<GatewayScore[]>(
    USE_MOCK_DATA ? 'mock:scores' : `/analytics/gateway-scores?${queryString}`,
    USE_MOCK_DATA ? mockGatewayScores : fetcher,
    swrOpts,
  )

  const { data: srSeries } = useSWR<TimeSeriesPoint[]>(
    USE_MOCK_DATA ? 'mock:sr' : `/analytics/gateway-scores?${queryString}&series=sr`,
    USE_MOCK_DATA ? mockSRTimeSeries : fetcher,
    swrOpts,
  )

  const { data: decisionSeries } = useSWR<DecisionSeries[]>(
    USE_MOCK_DATA ? 'mock:decisions' : `/analytics/decisions?range=${range}&granularity=${granularity}&group_by=approach`,
    USE_MOCK_DATA ? mockDecisionsByApproach : fetcher,
    swrOpts,
  )

  const { data: gatewayShareData } = useSWR<GatewaySharePoint[]>(
    USE_MOCK_DATA ? 'mock:gw-share' : `/analytics/decisions?range=${range}&granularity=${granularity}&group_by=gateway`,
    USE_MOCK_DATA ? mockGatewayShare : fetcher,
    swrOpts,
  )

  const { data: fbDecisionData } = useSWR<FeedbackDecisionPoint[]>(
    USE_MOCK_DATA ? 'mock:fb' : `/analytics/feedbacks?range=${range}&granularity=${granularity}`,
    USE_MOCK_DATA ? mockFeedbackDecisions : fetcher,
    swrOpts,
  )

  const { data: priorityRules } = useSWR<PriorityRule[]>(
    USE_MOCK_DATA ? 'mock:rules' : `/analytics/routing-stats?range=${range}`,
    USE_MOCK_DATA ? mockPriorityRules : fetcher,
    swrOpts,
  )

  const { data: feedbackErrors } = useSWR<FeedbackError[]>(
    USE_MOCK_DATA ? 'mock:errors' : `/analytics/feedbacks?range=${range}&errors=true`,
    USE_MOCK_DATA ? mockFeedbackErrors : fetcher,
    swrOpts,
  )

  // ─── Empty state ────────────────────────────────────────────────────────

  const hasNoData = !kpi && !gatewayScores

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Analytics</h1>
          <p className="text-sm text-slate-500 dark:text-[#66666e] mt-1">
            Real-time routing performance and decision metrics
          </p>
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-400 dark:text-[#55555e]">
          <RefreshCw size={12} className="animate-spin" style={{ animationDuration: '3s' }} />
          Auto-refresh 5s
          {USE_MOCK_DATA && <Badge variant="orange">Mock</Badge>}
        </div>
      </div>

      {/* Filters */}
      <FilterBar params={searchParams} setParam={setParam} />

      {/* Merchant warning */}
      {!merchantId && !USE_MOCK_DATA && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 dark:bg-yellow-900/10 dark:border-yellow-800/30 px-4 py-3 flex items-center gap-2 text-sm text-yellow-800 dark:text-yellow-400">
          <AlertTriangle size={16} />
          Set your Merchant ID in the top bar to load analytics data.
        </div>
      )}

      {/* Empty state */}
      {hasNoData && !USE_MOCK_DATA && merchantId && (
        <Card>
          <CardBody className="py-16 text-center">
            <Activity size={40} className="mx-auto text-slate-300 dark:text-[#333] mb-4" />
            <p className="text-slate-500 dark:text-[#66666e] text-sm">
              No analytics data available yet. Decisions and feedbacks will appear here once traffic starts flowing.
            </p>
          </CardBody>
        </Card>
      )}

      {/* KPI Tiles */}
      {kpi && (
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4">
          <KPITile label="Decisions/sec" value={formatNumber(kpi.decisions_per_sec)} suffix="/s" sparkline={kpi.sparkline} icon={Zap} color="text-brand-500" />
          <KPITile label="Decisions 5m" value={formatNumber(kpi.decisions_5m)} sparkline={kpi.sparkline} icon={Activity} color="text-blue-400" />
          <KPITile label="Decisions 1h" value={formatNumber(kpi.decisions_1h)} sparkline={kpi.sparkline} icon={Activity} color="text-blue-400" />
          <KPITile label="Feedbacks/sec" value={formatNumber(kpi.feedbacks_per_sec)} suffix="/s" sparkline={kpi.sparkline} icon={TrendingUp} color="text-emerald-400" />
          <KPITile label="Avg SR" value={`${(kpi.avg_sr * 100).toFixed(1)}%`} sparkline={kpi.sparkline} icon={TrendingUp} color="text-emerald-400" />
          <KPITile label="Error Rate" value={`${kpi.error_rate.toFixed(1)}%`} sparkline={kpi.sparkline} icon={AlertTriangle} color={kpi.error_rate > 2 ? 'text-red-400' : 'text-slate-400'} />
        </div>
      )}

      {/* Charts Row 1 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Realtime SR per PSP */}
        <Card>
          <CardHeader>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Realtime SR per PSP</h3>
          </CardHeader>
          <CardBody>
            {srSeries && srSeries.length > 0 ? (
              <ResponsiveContainer width="100%" height={260}>
                <LineChart data={srSeries}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1c1c1f" />
                  <XAxis dataKey="timestamp" tickFormatter={formatTimestamp} tick={chartAxisStyle} />
                  <YAxis domain={[0.5, 1]} tickFormatter={(v: number) => `${(v * 100).toFixed(0)}%`} tick={chartAxisStyle} />
                  <Tooltip
                    contentStyle={{ backgroundColor: '#0f0f11', border: '1px solid #1c1c1f', borderRadius: 12, fontSize: 12 }}
                    labelFormatter={formatTimestamp}
                    formatter={(value: number) => [`${(value * 100).toFixed(1)}%`]}
                  />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  {GATEWAYS.map((gw) => (
                    <Line
                      key={gw}
                      type="monotone"
                      dataKey={gw}
                      stroke={GATEWAY_COLORS[gw]}
                      strokeWidth={2}
                      dot={false}
                    />
                  ))}
                </LineChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[260px] flex items-center justify-center text-sm text-slate-400 dark:text-[#55555e]">
                No data
              </div>
            )}
          </CardBody>
        </Card>

        {/* Decision throughput by approach */}
        <Card>
          <CardHeader>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Decision Throughput by Approach</h3>
          </CardHeader>
          <CardBody>
            {decisionSeries && decisionSeries.length > 0 ? (
              <ResponsiveContainer width="100%" height={260}>
                <AreaChart data={decisionSeries}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1c1c1f" />
                  <XAxis dataKey="timestamp" tickFormatter={formatTimestamp} tick={chartAxisStyle} />
                  <YAxis tick={chartAxisStyle} />
                  <Tooltip
                    contentStyle={{ backgroundColor: '#0f0f11', border: '1px solid #1c1c1f', borderRadius: 12, fontSize: 12 }}
                    labelFormatter={formatTimestamp}
                  />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  {Object.entries(APPROACH_COLORS).map(([key, color]) => (
                    <Area
                      key={key}
                      type="monotone"
                      dataKey={key}
                      stackId="1"
                      stroke={color}
                      fill={color}
                      fillOpacity={0.3}
                    />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[260px] flex items-center justify-center text-sm text-slate-400 dark:text-[#55555e]">
                No data
              </div>
            )}
          </CardBody>
        </Card>
      </div>

      {/* Charts Row 2 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Gateway share */}
        <Card>
          <CardHeader>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Gateway Share of Decisions</h3>
          </CardHeader>
          <CardBody>
            {gatewayShareData && gatewayShareData.length > 0 ? (
              <ResponsiveContainer width="100%" height={260}>
                <AreaChart data={gatewayShareData} stackOffset="expand">
                  <CartesianGrid strokeDasharray="3 3" stroke="#1c1c1f" />
                  <XAxis dataKey="timestamp" tickFormatter={formatTimestamp} tick={chartAxisStyle} />
                  <YAxis tickFormatter={(v: number) => `${(v * 100).toFixed(0)}%`} tick={chartAxisStyle} />
                  <Tooltip
                    contentStyle={{ backgroundColor: '#0f0f11', border: '1px solid #1c1c1f', borderRadius: 12, fontSize: 12 }}
                    labelFormatter={formatTimestamp}
                    formatter={(value: number) => [`${(value * 100).toFixed(1)}%`]}
                  />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  {GATEWAYS.map((gw) => (
                    <Area
                      key={gw}
                      type="monotone"
                      dataKey={gw}
                      stackId="1"
                      stroke={GATEWAY_COLORS[gw]}
                      fill={GATEWAY_COLORS[gw]}
                      fillOpacity={0.4}
                    />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[260px] flex items-center justify-center text-sm text-slate-400 dark:text-[#55555e]">
                No data
              </div>
            )}
          </CardBody>
        </Card>

        {/* Feedback vs Decision throughput */}
        <Card>
          <CardHeader>
            <h3 className="text-sm font-semibold text-slate-800 dark:text-white">Feedback vs Decision Throughput</h3>
          </CardHeader>
          <CardBody>
            {fbDecisionData && fbDecisionData.length > 0 ? (
              <ResponsiveContainer width="100%" height={260}>
                <LineChart data={fbDecisionData}>
                  <CartesianGrid strokeDasharray="3 3" stroke="#1c1c1f" />
                  <XAxis dataKey="timestamp" tickFormatter={formatTimestamp} tick={chartAxisStyle} />
                  <YAxis yAxisId="left" tick={chartAxisStyle} />
                  <YAxis yAxisId="right" orientation="right" tick={chartAxisStyle} />
                  <Tooltip
                    contentStyle={{ backgroundColor: '#0f0f11', border: '1px solid #1c1c1f', borderRadius: 12, fontSize: 12 }}
                    labelFormatter={formatTimestamp}
                  />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  <Line yAxisId="left" type="monotone" dataKey="decisions" stroke="#3b82f6" strokeWidth={2} dot={false} />
                  <Line yAxisId="right" type="monotone" dataKey="feedbacks" stroke="#10b981" strokeWidth={2} dot={false} />
                </LineChart>
              </ResponsiveContainer>
            ) : (
              <div className="h-[260px] flex items-center justify-center text-sm text-slate-400 dark:text-[#55555e]">
                No data
              </div>
            )}
          </CardBody>
        </Card>
      </div>

      {/* Tables */}
      {gatewayScores && <PSPScoreboard data={gatewayScores} />}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {priorityRules && <PriorityRulesTable data={priorityRules} />}
        {feedbackErrors && <FeedbackErrorsTable data={feedbackErrors} />}
      </div>
    </div>
  )
}
