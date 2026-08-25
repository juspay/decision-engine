import { useMemo, useState } from 'react'
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Handshake } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import {
  useVolumeCommitment,
  useVolumeCommitmentAudit,
  useVolumeCommitmentSeries,
} from '../../hooks/useVolumeCommitment'
import { CommitmentAuditEvent, CommitmentConnectorSeries } from '../../types/api'

/**
 * Fixed categorical order, assigned by contract position — never re-ordered by rank, so a PSP
 * keeps its color as others come and go. Validated for CVD + contrast in light and dark.
 */
const SERIES_COLORS = ['#0069ED', '#0d9488', '#ea580c', '#8b5cf6']
const ELIMINATED_COLOR = '#94a3b8'

function formatAmount(value: number) {
  if (Math.abs(value) >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (Math.abs(value) >= 1_000) return `${(value / 1_000).toFixed(0)}k`
  return value.toFixed(0)
}

function seriesColor(index: number, eliminated: boolean) {
  return eliminated ? ELIMINATED_COLOR : SERIES_COLORS[index % SERIES_COLORS.length]
}

/** Run picker chips — selected reads as a filled pill, the rest as quiet outlines. */
function runChipClass(selected: boolean) {
  return `rounded-full border px-2.5 py-1 text-xs font-medium tabular-nums transition-colors ${
    selected
      ? 'border-brand-500/50 bg-brand-500/10 text-brand-600 dark:text-brand-400'
      : 'border-slate-200 text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-white/5'
  }`
}

const AUDIT_BADGES: Record<CommitmentAuditEvent['kind'], { label: string; variant: 'blue' | 'orange' | 'red' }> = {
  forecast: { label: 'Forecast', variant: 'blue' },
  steered: { label: 'Steered', variant: 'orange' },
  eliminated: { label: 'Eliminated', variant: 'red' },
}

/** Merge per-PSP day points into chart rows: cumulative delivery plus the straight promise line. */
function buildChartRows(connectors: CommitmentConnectorSeries[]) {
  const daysTotal = Math.max(1, ...connectors.map((c) => c.daysTotal))
  const rows: Record<string, number>[] = []
  const running: Record<string, number> = {}
  const lastSeen: Record<string, number> = {}
  for (const c of connectors) {
    lastSeen[c.connector] = Math.max(0, ...c.points.map((p) => p.dayIndex))
  }
  for (let day = 0; day <= daysTotal; day += 1) {
    const row: Record<string, number> = { day }
    for (const c of connectors) {
      const point = c.points.find((p) => p.dayIndex === day)
      if (point) running[c.connector] = (running[c.connector] ?? 0) + point.total
      // The delivery line stops at the last observed day; the promise runs the whole cycle.
      if (day <= lastSeen[c.connector]) row[c.connector] = running[c.connector] ?? 0
      row[`${c.connector} promise`] = (c.goal * Math.min(day, c.daysTotal)) / c.daysTotal
      if (point && point.steered > 0) row[`${c.connector} steered`] = point.steered
    }
    rows.push(row)
  }
  return rows
}

/**
 * Everything the volume-commitment engine did for this merchant, and why: per-contract pacing
 * cards, the cumulative-volume-vs-promise chart, and the audit trail underneath.
 */
export function VolumeCommitmentAnalytics() {
  const { merchantId } = useMerchantStore()
  // `undefined` = the run in flight; a run id pins the whole tab — cards, chart and trail — to
  // that past execution.
  const [selectedRun, setSelectedRun] = useState<string | undefined>(undefined)
  const pacing = useVolumeCommitment(merchantId)
  const series = useVolumeCommitmentSeries(merchantId, selectedRun)
  const audit = useVolumeCommitmentAudit(merchantId, selectedRun)

  const connectors = series.data?.connectors ?? []
  const rows = useMemo(() => buildChartRows(connectors), [connectors])
  const byConnector = useMemo(
    () => new Map((pacing.data?.psps ?? []).map((p) => [p.connector, p])),
    [pacing.data],
  )

  // A finished run's verdict cannot come from the live plan — that describes the cycle in flight.
  // It comes from the run itself: what its own chart shows delivered, and which connectors its own
  // audit recorded as eliminated.
  const isPastRun =
    selectedRun !== undefined && !audit.runs.find((r) => r.runId === selectedRun)?.isCurrent
  const deliveredInRun = useMemo(() => {
    const totals = new Map<string, number>()
    for (const c of connectors) {
      totals.set(
        c.connector,
        c.points.reduce((sum, p) => sum + p.total, 0),
      )
    }
    return totals
  }, [connectors])
  const eliminatedInRun = useMemo(
    () =>
      new Set(
        audit.events
          .filter((e) => e.kind === 'eliminated' && e.connector)
          .map((e) => e.connector as string),
      ),
    [audit.events],
  )

  if (!merchantId) return <ErrorMessage error="Set a merchant ID to view volume commitments." />
  if (pacing.error || series.error) return <ErrorMessage error="Could not load volume-commitment analytics." />
  if (pacing.isLoading || series.isLoading) {
    return (
      <div className="flex justify-center py-10">
        <Spinner />
      </div>
    )
  }

  if (connectors.length === 0) {
    return (
      <Card>
        <CardBody>
          <div className="flex items-start gap-3">
            <Handshake size={16} className="mt-0.5 text-brand-500" />
            <div>
              <p className="text-sm font-medium text-slate-800 dark:text-white">
                No volume contracts are active for this merchant
              </p>
              <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
                Configure commitments on the Volume Contracts page and activate the document, then
                enable “Volume contracts (meet PSP commitments)” on the Multi Objective page. Pick a
                short test cycle so a full period plays out in minutes, then drive traffic from the
                Decision Simulator and watch it land here.
              </p>
            </div>
          </div>
        </CardBody>
      </Card>
    )
  }

  return (
    <div className="space-y-6">
      {/* One card per contract: the promise, the reward, and where the PSP stands right now. */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {connectors.map((c, i) => {
          const pace = byConnector.get(c.connector)
          const achieved = isPastRun ? (deliveredInRun.get(c.connector) ?? 0) : (pace?.achieved ?? 0)
          const eliminated = isPastRun ? eliminatedInRun.has(c.connector) : c.eliminated
          const met = c.goal > 0 && achieved >= c.goal
          const pct = c.goal > 0 ? Math.min(100, (achieved / c.goal) * 100) : 0
          return (
            <Card key={c.connector}>
              <CardBody>
                <div className="flex items-center justify-between gap-2">
                  <span className="flex items-center gap-2 text-sm font-medium text-slate-800 dark:text-white">
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ backgroundColor: seriesColor(i, c.eliminated) }}
                    />
                    {c.connector}
                  </span>
                  {met ? (
                    <Badge variant="green">Met</Badge>
                  ) : eliminated ? (
                    <Badge variant="red">Eliminated</Badge>
                  ) : isPastRun ? (
                    <Badge variant="gray">Missed</Badge>
                  ) : pace?.steering ? (
                    <Badge variant="orange">Steering</Badge>
                  ) : (
                    <Badge variant="green">On pace</Badge>
                  )}
                </div>
                <p className="mt-2 text-2xl font-semibold tabular-nums text-slate-900 dark:text-white">
                  {formatAmount(c.reward)}
                </p>
                <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                  reward on a {formatAmount(c.goal)} promise
                </p>
                <p className="mt-1 text-xs tabular-nums text-slate-500 dark:text-slate-400">
                  {formatAmount(achieved)} delivered · {pct.toFixed(0)}% · {c.daysTotal}-day cycle
                </p>
              </CardBody>
            </Card>
          )
        })}
      </div>

      <Card>
        <CardHeader>
          <SurfaceLabel>Cumulative volume vs. each promise</SurfaceLabel>
          <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
            Solid lines are delivered volume; dashed lines are the pace each promise needs
          </h2>
          {isPastRun && (
            <p className="mt-2 text-xs text-amber-600 dark:text-amber-500">
              Showing the finished run {selectedRun} — its own delivery across its own cycle, not
              the one currently in flight.
            </p>
          )}
        </CardHeader>
        <CardBody>
          <div className="h-80 w-full">
            <ResponsiveContainer>
              <LineChart data={rows} margin={{ top: 8, right: 16, bottom: 4, left: 8 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="currentColor" opacity={0.12} />
                <XAxis
                  dataKey="day"
                  tickFormatter={(d: number) => `Day ${d}`}
                  tick={{ fontSize: 11 }}
                  stroke="currentColor"
                  opacity={0.5}
                />
                <YAxis
                  tickFormatter={(v: number) => formatAmount(v)}
                  tick={{ fontSize: 11 }}
                  stroke="currentColor"
                  opacity={0.5}
                  width={56}
                />
                <Tooltip
                  formatter={(value: number, name: string) => [formatAmount(value), name]}
                  labelFormatter={(d) => `Day ${d}`}
                  wrapperStyle={{ zIndex: 30, outline: 'none' }}
                  contentStyle={{ fontSize: 12 }}
                />
                <Legend wrapperStyle={{ fontSize: 12 }} />
                {connectors.map((c, i) => (
                  <Line
                    key={c.connector}
                    dataKey={c.connector}
                    stroke={seriesColor(i, c.eliminated)}
                    strokeWidth={2}
                    dot={false}
                    isAnimationActive={false}
                  />
                ))}
                {connectors.map((c, i) => (
                  <Line
                    key={`${c.connector}-promise`}
                    dataKey={`${c.connector} promise`}
                    stroke={seriesColor(i, c.eliminated)}
                    strokeWidth={1.5}
                    strokeDasharray="6 4"
                    opacity={0.55}
                    dot={false}
                    legendType="none"
                    isAnimationActive={false}
                  />
                ))}
              </LineChart>
            </ResponsiveContainer>
          </div>
          <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
            A solid line tracking below its dashed promise is behind pace — the engine steers a
            little extra volume there, in chunks spread through the day. Eliminated contracts turn
            gray: not enough traffic remains to land them, so the engine backs the rest instead.
          </p>
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <SurfaceLabel>Audit trail</SurfaceLabel>
          <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
            Every forecast, steer chunk and elimination — what happened, when, and why
          </h2>
          {audit.runs.length > 0 && (
            <>
              <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                A contract runs once per billing cycle and each pass is judged on its own
                delivery. Picking one re-renders this whole tab — cards, chart and trail — for that
                run.
              </p>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <button
                  onClick={() => setSelectedRun(undefined)}
                  className={runChipClass(selectedRun === undefined)}
                >
                  All runs
                </button>
                {audit.runs.map((run) => (
                  <button
                    key={run.runId}
                    onClick={() => setSelectedRun(run.runId)}
                    title={`${run.runId} · ${run.forecasts} forecasts · ${run.steers} steers · ${run.eliminations} eliminations`}
                    className={runChipClass(selectedRun === run.runId)}
                  >
                    {run.isCurrent ? '● ' : ''}
                    {new Date(run.startedAtEpochMs).toLocaleString(undefined, {
                      month: 'short',
                      day: 'numeric',
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                    <span className="ml-1.5 tabular-nums opacity-60">{run.steers}↗</span>
                  </button>
                ))}
              </div>
            </>
          )}
        </CardHeader>
        <CardBody>
          {audit.events.length === 0 ? (
            <p className="text-sm text-slate-500 dark:text-slate-400">
              Nothing yet. Entries appear when the scheduler runs a forecast or a payment is
              steered.
            </p>
          ) : (
            <div className="max-h-96 space-y-2 overflow-y-auto pr-1">
              {audit.events.slice(0, 200).map((event, i) => {
                const badge = AUDIT_BADGES[event.kind]
                return (
                  <div
                    key={`${event.atEpochMs}-${i}`}
                    className="flex items-start gap-3 rounded-lg border border-slate-100 px-3 py-2 dark:border-slate-800"
                  >
                    <Badge variant={badge.variant}>{badge.label}</Badge>
                    <div className="min-w-0 flex-1">
                      <p className="text-sm text-slate-700 dark:text-slate-300">{event.message}</p>
                      <p className="mt-0.5 text-xs tabular-nums text-slate-400 dark:text-slate-500">
                        {new Date(event.atEpochMs).toLocaleString()}
                        {event.connector ? ` · ${event.connector}` : ''}
                        {event.amount != null ? ` · ${formatAmount(event.amount)}` : ''}
                        {selectedRun === undefined && event.runId ? ` · ${event.runId}` : ''}
                      </p>
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  )
}
