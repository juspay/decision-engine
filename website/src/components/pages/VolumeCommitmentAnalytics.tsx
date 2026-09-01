import { useCallback, useMemo, useState } from 'react'
import {
  Bar,
  BarChart,
  CartesianGrid,
  Customized,
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
  useVolumeCommitmentImpact,
  useVolumeCommitmentSeries,
} from '../../hooks/useVolumeCommitment'
import { CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import { CommitmentAuditEvent, CommitmentConnectorImpact, CommitmentConnectorSeries } from '../../types/api'
import { CommitmentContractCards, CommitmentPacingChart, PacingStatus } from './CommitmentPacingChart'
import {
  COMMITMENT_SERIES_COLORS,
  DashSwatch,
  HatchDefs,
  HatchSwatch,
  SECS_PER_DAY,
  SolidSwatch,
  bucketsPerDay,
  compactAmount,
  dayUnit,
  firstEliminationByConnector,
  formatMoney,
  hatchId, pctOfGoal} from './volumeCommitmentChartBits'

/** Stable empty fallbacks, so memos keyed on "no data yet" do not recompute every render. */
const NO_SERIES: CommitmentConnectorSeries[] = []
const NO_IMPACT: CommitmentConnectorImpact[] = []

/** Color by contract position — a PSP keeps it whatever its standing. */
function seriesColor(index: number) {
  return COMMITMENT_SERIES_COLORS[index % COMMITMENT_SERIES_COLORS.length]
}

/** Picker chips — selected reads as a filled pill, the rest as quiet outlines. */
function chipClass(selected: boolean) {
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

/** Runs offered in the picker — the newest few; older cycles stay reachable through the API. */
const RUNS_SHOWN = 10
/** Audit entries rendered, out of the window the backend returns. */
const AUDIT_EVENTS_SHOWN = 200

function formatWhen(ms: number) {
  return new Date(ms).toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatCount(value: number) {
  return value.toLocaleString()
}

function pct(part: number, whole: number) {
  return whole > 0 ? `${((part / whole) * 100).toFixed(0)}%` : '—'
}

type Metric = 'volume' | 'payments'

/** The formatter a metric's numbers read in. */
function fmtFor(metric: Metric) {
  return metric === 'volume' ? compactAmount : formatCount
}

/** One row of the before / with-contract charts. */
type ImpactRow = {
  name: string
  color: string
  hatch: string
  eliminated: boolean
  goal: number
  before: number
  unaided: number
  steered: number
  beforePayments: number
  unaidedPayments: number
  steeredPayments: number
  cededPayments: number
  ceded: number
}

/** One contract day on the day-wise charts: a label plus `<psp>__unaided` / `<psp>__steered` pairs,
 *  and `<psp>__pace` — what that promise needed *that* day given what had landed before it. */
type DayRow = { day: string } & Record<string, number | string | undefined>

type BandScale = ((value: string) => number | undefined) & { bandwidth?: () => number }

/** A dashed segment per PSP across each day's band at that day's required pace; mounted through
 *  `<Customized>` so it can read the chart's band and value scales. */
export function PaceMarkers(props: {
  rows: DayRow[]
  psps: ImpactRow[]
  xAxisMap?: Record<string, { scale: BandScale }>
  yAxisMap?: Record<string, { scale: (value: number) => number | undefined }>
}) {
  const { rows, psps, xAxisMap, yAxisMap } = props
  const xAxis = xAxisMap ? Object.values(xAxisMap)[0] : undefined
  const yAxis = yAxisMap ? Object.values(yAxisMap)[0] : undefined
  if (!xAxis || !yAxis) return null
  const band = xAxis.scale.bandwidth?.() ?? 0
  if (band <= 0) return null
  const inset = band * 0.08
  return (
    <g>
      {rows.flatMap((row) =>
        psps.map((r) => {
          const value = row[`${r.name}__pace`]
          if (typeof value !== 'number') return null
          const x0 = xAxis.scale(row.day)
          const y = yAxis.scale(value)
          if (x0 == null || y == null) return null
          return (
            <line
              key={`${row.day}-${r.name}`}
              x1={x0 + inset}
              x2={x0 + band - inset}
              y1={y}
              y2={y}
              stroke={r.color}
              strokeWidth={1.5}
              strokeDasharray="5 3"
              opacity={0.75}
            />
          )
        }),
      )}
    </g>
  )
}

/** Per-day stacked columns: solid unaided, hatched steered; dashed = what each promise needed that day. */
function DayWiseChart({
  rows,
  psps,
  metric,
  yMax,
  hatchScope,
}: {
  rows: DayRow[]
  psps: ImpactRow[]
  metric: Metric
  yMax: number
  hatchScope: string
}) {
  const fmt = fmtFor(metric)
  const hatch = (name: string) => hatchId(hatchScope, name)
  return (
    <div className="h-72 w-full">
      <ResponsiveContainer>
        <BarChart data={rows} margin={{ top: 12, right: 12, bottom: 4, left: 4 }} barCategoryGap="28%" barGap={3}>
          <Customized component={<HatchDefs entries={psps.map((r) => ({ id: hatch(r.name), color: r.color }))} />} />
          <CartesianGrid vertical={false} stroke="currentColor" opacity={0.08} />
          <XAxis
            dataKey="day"
            tickFormatter={(d: string) => `Day ${d}`}
            tick={{ fontSize: 11 }}
            stroke="currentColor"
            opacity={0.5}
            tickLine={false}
            interval={rows.length > 16 ? Math.ceil(rows.length / 8) - 1 : 0}
          />
          <YAxis domain={[0, yMax]} tickFormatter={fmt} tick={{ fontSize: 11 }} stroke="currentColor" opacity={0.5} width={56} tickLine={false} />
          <Tooltip
            cursor={{ fill: 'currentColor', opacity: 0.04 }}
            wrapperStyle={{ zIndex: 30, outline: 'none' }}
            content={({ active, payload, label }) => {
              if (!active || !payload?.length) return null
              const row = payload[0]?.payload as DayRow | undefined
              if (!row) return null
              return (
                <div style={{ ...CHART_TOOLTIP_STYLE, fontSize: 12, lineHeight: 1.5, minWidth: 230 }}>
                  <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>
                    Day {label}
                  </p>
                  {psps.map((r) => {
                    const unaided = Number(row[`${r.name}__unaided`] ?? 0)
                    const steered = Number(row[`${r.name}__steered`] ?? 0)
                    const needed = row[`${r.name}__pace`]
                    return (
                      <div key={r.name} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                        <span style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: r.color, flexShrink: 0 }} />
                        <span style={{ fontWeight: 600 }}>{r.name}</span>
                        <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>
                          {fmt(unaided + steered)}
                          {steered > 0 && <span style={{ opacity: 0.7 }}> · {fmt(steered)} steered</span>}
                          {typeof needed === 'number' && <span style={{ opacity: 0.7 }}> · needed {fmt(needed)}</span>}
                        </span>
                      </div>
                    )
                  })}
                </div>
              )
            }}
          />
          {psps.map((r) => (
            <Bar key={`${r.name}-unaided`} dataKey={`${r.name}__unaided`} stackId={r.name} fill={r.color} barSize={26} isAnimationActive={false} />
          ))}
          {psps.map((r) => (
            <Bar key={`${r.name}-steered`} dataKey={`${r.name}__steered`} stackId={r.name} fill={`url(#${hatch(r.name)})`} barSize={26} radius={[4, 4, 0, 0]} isAnimationActive={false} />
          ))}
          {metric === 'volume' && <Customized component={<PaceMarkers rows={rows} psps={psps} />} />}
        </BarChart>
      </ResponsiveContainer>
    </div>
  )
}

function StatTile({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <Card>
      <CardBody>
        <SurfaceLabel>{label}</SurfaceLabel>
        <p className="mt-2 text-2xl font-semibold text-slate-900 dark:text-white">{value}</p>
        {hint && <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">{hint}</p>}
      </CardBody>
    </Card>
  )
}

/** Analytics tab: before/after per PSP, pacing chart, and audit trail for one run. */
export function VolumeCommitmentAnalytics() {
  const { merchantId } = useMerchantStore()
  // `undefined` = the run in flight; a run id pins the whole tab to that past execution.
  const [selectedRun, setSelectedRun] = useState<string | undefined>(undefined)
  const [metric, setMetric] = useState<Metric>('volume')

  const pacing = useVolumeCommitment(merchantId)
  // Sub-day buckets so running totals curve within a day.
  const perDay = bucketsPerDay(pacing.data?.daySecs)
  const series = useVolumeCommitmentSeries(merchantId, selectedRun, { perDay })
  const audit = useVolumeCommitmentAudit(merchantId, selectedRun)
  const impact = useVolumeCommitmentImpact(merchantId, selectedRun)

  const connectors = series.data?.connectors ?? NO_SERIES
  const currency = series.data?.currency
  // Color by contract position, shared by every chart and table on the page.
  const colorIndex = useMemo(
    () => new Map(connectors.map((c, i) => [c.connector, i] as const)),
    [connectors],
  )
  const byConnector = useMemo(
    () => new Map((pacing.data?.psps ?? []).map((p) => [p.connector, p])),
    [pacing.data],
  )
  const eliminatedReasons = useMemo(
    () => new Map((pacing.data?.eliminated ?? []).map((e) => [e.connector, e.reason])),
    [pacing.data],
  )

  // A past run's verdict comes from its own series and audit, not the live plan.
  const isPastRun =
    selectedRun !== undefined && !audit.runs.find((r) => r.runId === selectedRun)?.isCurrent
  const eliminatedInRun = useMemo(
    () =>
      new Set(
        audit.events
          .filter((e) => e.kind === 'eliminated' && e.connector && (!selectedRun || e.runId === selectedRun))
          .map((e) => e.connector as string),
      ),
    [audit.events, selectedRun],
  )
  // First elimination per PSP within the shown run only, so an old cycle's drop does not pin day 0.
  const shownRunId = selectedRun ?? audit.runs.find((r) => r.isCurrent)?.runId
  const eliminatedAtMs = useMemo(
    () => firstEliminationByConnector(audit.events, shownRunId),
    [audit.events, shownRunId],
  )
  const deliveredInRun = useMemo(() => {
    const totals = new Map<string, number>()
    for (const c of connectors) totals.set(c.connector, c.points.reduce((sum, p) => sum + p.total, 0))
    return totals
  }, [connectors])
  const achievedFor = useCallback(
    (connector: string) =>
      isPastRun ? (deliveredInRun.get(connector) ?? 0) : (byConnector.get(connector)?.achieved ?? deliveredInRun.get(connector) ?? 0),
    [isPastRun, deliveredInRun, byConnector],
  )
  const statusFor = useCallback(
    (connector: string): PacingStatus => {
      const c = connectors.find((x) => x.connector === connector)
      const achieved = achievedFor(connector)
      const met = (c?.goal ?? 0) > 0 && achieved >= (c?.goal ?? 0)
      if (met) return 'met'
      const eliminated = isPastRun ? eliminatedInRun.has(connector) : (c?.eliminated ?? false)
      if (eliminated) return isPastRun ? 'missed' : 'eliminated'
      if (isPastRun) return 'missed'
      const live = byConnector.get(connector)
      if (!live) return 'pending'
      return live.steering ? 'steering' : 'on_pace'
    },
    [connectors, achievedFor, isPastRun, eliminatedInRun, byConnector],
  )
  const reasonFor = useCallback((connector: string) => eliminatedReasons.get(connector), [eliminatedReasons])
  const pacingColorFor = useCallback(
    (connector: string, index: number) => seriesColor(colorIndex.get(connector) ?? index),
    [colorIndex],
  )

  const impactConnectors = impact.data?.connectors ?? NO_IMPACT
  const impactRows: ImpactRow[] = useMemo(
    () =>
      impactConnectors.map((c, i) => {
        const idx = colorIndex.get(c.connector) ?? i
        const eliminated = isPastRun ? eliminatedInRun.has(c.connector) : c.eliminated
        return {
          name: c.connector,
          color: seriesColor(idx),
          hatch: hatchId('analytics', c.connector),
          eliminated,
          goal: c.goal,
          before: c.before.volume,
          unaided: c.unaided.volume,
          steered: c.steered.volume,
          ceded: c.ceded.volume,
          beforePayments: c.before.payments,
          unaidedPayments: c.unaided.payments,
          steeredPayments: c.steered.payments,
          cededPayments: c.ceded.payments,
        }
      }),
    [impactConnectors, colorIndex, isPastRun, eliminatedInRun],
  )
  const dayWord = dayUnit(impact.data?.daySecs).word
  // Day-by-day rows for both cycles, one row per contract day with a pair of keys per PSP.
  const { beforeRows, withRows, beforeYMax, withYMax } = useMemo(() => {
    const daysTotal = Math.max(1, impact.data?.daysTotal ?? 1)
    const dayMs = Math.max(1, (impact.data?.daySecs ?? SECS_PER_DAY) * 1000)
    const baselineDays = impact.data?.baselineDays ?? []
    const cycleDays = impact.data?.cycleDays ?? []
    const baselineSpan = impact.data ? Math.max(1, Math.round((impact.data.baseline.endMs - impact.data.baseline.startMs) / dayMs)) : daysTotal
    const pick = (p: { total: number; steered: number; payments: number; steeredPayments: number }) =>
      metric === 'volume'
        ? { unaided: Math.max(0, p.total - p.steered), steered: p.steered }
        : { unaided: Math.max(0, p.payments - p.steeredPayments), steered: p.steeredPayments }
    // `markThrough`: the last day that gets a pace marker. Days the cycle has not reached yet
    // would only show "everything still owed, spread over what is left", which is not a pace
    // anyone has failed or met yet.
    const build = (points: typeof baselineDays, count: number, markThrough: number, droppedOn?: Map<string, number>) => {
      const byKey = new Map(points.map((p) => [`${p.connector}:${p.dayIndex}`, p]))
      const rows: DayRow[] = []
      const deliveredSoFar: Record<string, number> = {}
      for (let i = 0; i < count; i += 1) {
        const row: DayRow = { day: String(i) }
        for (const r of impactRows) {
          const p = byKey.get(`${r.name}:${i}`)
          const v = p ? pick(p) : { unaided: 0, steered: 0 }
          row[`${r.name}__unaided`] = v.unaided
          row[`${r.name}__steered`] = v.steered
          // What this day had to bring: the volume still owed when it opened, spread over the
          // days that were left — so a short day raises the bar for the next, a strong one lowers it.
          // Volume-based regardless of the metric shown; payments have no goal.
          // Nothing is needed of a promise once the engine has given it up: its marker would
          // only climb toward the impossible and squash every bar under it.
          const dropDay = droppedOn?.get(r.name)
          const remaining = r.goal - (deliveredSoFar[r.name] ?? 0)
          if (metric === 'volume' && r.goal > 0 && remaining > 0 && i <= markThrough && (dropDay == null || i < dropDay)) {
            row[`${r.name}__pace`] = remaining / (count - i)
          }
          deliveredSoFar[r.name] = (deliveredSoFar[r.name] ?? 0) + (p ? p.total : 0)
        }
        rows.push(row)
      }
      return rows
    }
    const cycleStartMs = impact.data?.cycle.startMs
    const currentDay =
      isPastRun || cycleStartMs == null ? daysTotal - 1 : Math.min(daysTotal - 1, Math.floor((Date.now() - cycleStartMs) / dayMs))
    const beforeSpan = Math.min(daysTotal, baselineSpan)
    // The previous cycle is marked only as far as it had traffic; past that, the markers would
    // just climb toward "everything, on the last day" over an empty chart.
    const lastBaselineDay = Math.max(-1, ...baselineDays.map((p) => p.dayIndex))
    const beforeRows = build(baselineDays, beforeSpan, lastBaselineDay)
    // A drop that did not stick — the PSP was written off (often on lagging numbers in a cycle's
    // last seconds) but landed its goal anyway — does not erase what it needed on the way there.
    const droppedOn = new Map<string, number>()
    if (cycleStartMs != null) {
      for (const [name, at] of eliminatedAtMs) {
        const r = impactRows.find((x) => x.name === name)
        const met = r != null && r.goal > 0 && r.unaided + r.steered >= r.goal
        if (!met) droppedOn.set(name, Math.max(0, Math.floor((at - cycleStartMs) / dayMs)))
      }
    }
    const withRows = build(cycleDays, daysTotal, currentDay, droppedOn)
    // Each chart scales to its own bars and markers, with headroom, so a quiet cycle beside a
    // busy one does not squash the bars of either.
    const yMaxOf = (rows: DayRow[]) => {
      const max = Math.max(
        0,
        ...rows.flatMap((row) =>
          impactRows.flatMap((r) => [
            Number(row[`${r.name}__unaided`] ?? 0) + Number(row[`${r.name}__steered`] ?? 0),
            Number(row[`${r.name}__pace`] ?? 0),
          ]),
        ),
      )
      return max > 0 ? max * 1.12 : 1
    }
    return { beforeRows, withRows, beforeYMax: yMaxOf(beforeRows), withYMax: yMaxOf(withRows) }
  }, [impact.data, impactRows, metric, isPastRun, eliminatedAtMs])

  // Headline figures for the cycle.
  const totals = useMemo(() => {
    const steeredPayments = impactRows.reduce((n, r) => n + r.steeredPayments, 0)
    const cyclePayments = impactRows.reduce((n, r) => n + r.unaidedPayments + r.steeredPayments, 0)
    const steeredVolume = impactRows.reduce((n, r) => n + r.steered, 0)
    const cycleVolume = impactRows.reduce((n, r) => n + r.unaided + r.steered, 0)
    const met = impactRows.filter((r) => r.goal > 0 && r.unaided + r.steered >= r.goal)
    const rewardSecured = impactConnectors
      .filter((c) => c.goal > 0 && c.withContract.volume >= c.goal)
      .reduce((n, c) => n + c.reward, 0)
    const rewardAtStake = impactConnectors
      .filter((c) => !(isPastRun ? eliminatedInRun.has(c.connector) : c.eliminated))
      .reduce((n, c) => n + c.reward, 0)
    return { steeredPayments, cyclePayments, steeredVolume, cycleVolume, met: met.length, rewardSecured, rewardAtStake }
  }, [impactRows, impactConnectors, isPastRun, eliminatedInRun])

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
                Configure commitments on the Multi Objective page&apos;s Volume Contracts tab and
                activate the document, then enable “Volume contracts (meet PSP commitments)” under
                its Feature Flags. Pick a
                short test cycle so a full period plays out in minutes, then drive traffic from the
                Decision Simulator and watch it land here.
              </p>
            </div>
          </div>
        </CardBody>
      </Card>
    )
  }

  const cycle = impact.data?.cycle
  const baseline = impact.data?.baseline
  const valueOf = (r: ImpactRow, key: 'before' | 'unaided' | 'steered') =>
    metric === 'volume' ? r[key] : r[`${key}Payments` as const]

  return (
    <div className="space-y-6">
      {/* ── Chapter picker: which run, how far back the "before" reaches, which measure ── */}
      <Card>
        <CardBody className="space-y-3">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <p className="text-sm font-medium text-slate-800 dark:text-white">
                What the volume contract did
              </p>
              <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
                {impact.data ? (
                  <>
                    Contract live since <strong>{formatWhen(impact.data.contractSinceMs)}</strong>
                    {cycle && (
                      <>
                        {' '}· this cycle {formatWhen(cycle.startMs)} → {formatWhen(cycle.endMs)} (
                        {impact.data.daysTotal} {dayUnit(impact.data.daySecs).short.toLowerCase()} cycle)
                      </>
                    )}
                    {baseline && (
                      <>
                        {' '}· compared with the previous cycle, {formatWhen(baseline.startMs)} →{' '}
                        {formatWhen(baseline.endMs)}
                      </>
                    )}
                    {shownRunId && (
                      <>
                        {' '}· run <span className="font-mono">{shownRunId}</span>
                      </>
                    )}
                  </>
                ) : (
                  'Waiting for the first measurements of this contract.'
                )}
              </p>
            </div>
            <div className="inline-flex rounded-md border border-slate-200 p-0.5 dark:border-slate-700">
              {(['volume', 'payments'] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => setMetric(m)}
                  className={`rounded px-2.5 py-1 text-[11px] font-medium transition-colors ${
                    metric === m
                      ? 'bg-brand-500 text-white'
                      : 'text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200'
                  }`}
                >
                  {m === 'volume' ? 'Volume' : 'Payments'}
                </button>
              ))}
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
            {audit.runs.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-xs text-slate-500 dark:text-slate-400">Run</span>
                <button onClick={() => setSelectedRun(undefined)} className={chipClass(selectedRun === undefined)}>
                  Current
                </button>
                {audit.runs.slice(0, RUNS_SHOWN).map((run) => (
                  <button
                    key={run.runId}
                    onClick={() => setSelectedRun(run.runId)}
                    title={`${run.runId} · ${run.forecasts} forecasts · ${run.steers} steers · ${run.eliminations} eliminations`}
                    className={chipClass(selectedRun === run.runId)}
                  >
                    {run.isCurrent ? '● ' : ''}
                    {formatWhen(run.startedAtEpochMs)}
                    <span className="ml-1.5 font-mono text-[10px] opacity-50">{run.runId}</span>
                    <span className="ml-1.5 tabular-nums opacity-60">{run.steers}↗</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </CardBody>
      </Card>

      {/* ── The contract at a glance: promise, reward terms, standing ── */}
      <CommitmentContractCards
        connectors={connectors}
        currency={currency}
        colorFor={pacingColorFor}
        statusFor={statusFor}
        achievedFor={achievedFor}
        reasonFor={reasonFor}
      />

      {/* ── Headline: what the contract is worth and how much routing had to move ── */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatTile
          label="Reward secured"
          value={formatMoney(totals.rewardSecured, currency)}
          hint={`of ${formatMoney(totals.rewardAtStake, currency)} still reachable · ${totals.met}/${impactRows.length} commitments met`}
        />
        <StatTile
          label="Payments steered"
          value={formatCount(totals.steeredPayments)}
          hint={`${pct(totals.steeredPayments, totals.cyclePayments)} of ${formatCount(totals.cyclePayments)} payments this cycle`}
        />
        <StatTile
          label="Volume steered"
          value={formatMoney(totals.steeredVolume, currency)}
          hint={`${pct(totals.steeredVolume, totals.cycleVolume)} of ${formatMoney(totals.cycleVolume, currency)} delivered this cycle`}
        />
        <StatTile
          label="Approval-rate tolerance"
          value={pacing.data?.tolerance != null ? `${(pacing.data.tolerance * 100).toFixed(1)} pp` : '—'}
          hint="the most a steered payment may give up versus the best-approving PSP"
        />
      </div>

      {/* ── Chapters 1 & 2: day by day, previous cycle and this one — same axis, same PSP colors ── */}
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <SurfaceLabel>1 · Previous cycle</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              What each PSP received per {dayWord} in the previous cycle
            </h2>
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              {baseline ? `${formatWhen(baseline.startMs)} → ${formatWhen(baseline.endMs)}` : 'The cycle before this one'}
              {metric === 'volume' ? ' · dashed = what each promise needed that ' + dayWord + ', after what had already landed' : ''}
            </p>
          </CardHeader>
          <CardBody>
            <DayWiseChart rows={beforeRows} psps={impactRows} metric={metric} yMax={beforeYMax} hatchScope="before" />
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <SurfaceLabel>2 · This cycle</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              What each PSP received per {dayWord} in this cycle
            </h2>
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              Solid is what approval-rate routing sent on its own; hatched is what the engine steered in.
            </p>
          </CardHeader>
          <CardBody>
            <DayWiseChart rows={withRows} psps={impactRows} metric={metric} yMax={withYMax} hatchScope="with" />
          </CardBody>
        </Card>
      </div>

      {/* ── Legend + table twin: every value on the bars, and each PSP's standing ── */}
      <Card>
        <CardBody>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-slate-600 dark:text-slate-300">
            {impactRows.map((r) => (
              <span key={r.name} className="inline-flex items-center gap-1.5">
                <SolidSwatch color={r.color} />
                {r.name}
              </span>
            ))}
            <span className="inline-flex items-center gap-1.5">
              <HatchSwatch color={impactRows[0]?.color ?? COMMITMENT_SERIES_COLORS[0]} />
              Steered in
            </span>
            {metric === 'volume' && (
              <span className="inline-flex items-center gap-1.5">
                <DashSwatch />
                Pace each promise needs per {dayWord}
              </span>
            )}
          </div>
          <div className="mt-4 overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-slate-500 dark:text-slate-400">
                  <th className="py-1.5 pr-3 font-medium">PSP</th>
                  <th className="py-1.5 pr-3 font-medium">Status</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Previous cycle</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Routed by approval</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Steered in</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Ceded</th>
                  <th className="py-1.5 pr-3 text-right font-medium">This cycle</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Target</th>
                  <th className="py-1.5 text-right font-medium">Reward</th>
                </tr>
              </thead>
              <tbody className="tabular-nums text-slate-700 dark:text-slate-200">
                {impactRows.map((r) => {
                  const c = impactConnectors.find((x) => x.connector === r.name)
                  const total = r.unaided + r.steered
                  const status = statusFor(r.name)
                  const live = byConnector.get(r.name)
                  const fmt = fmtFor(metric)
                  return (
                    <tr key={r.name} className="border-t border-slate-100 dark:border-slate-800">
                      <td className="py-2 pr-3">
                        <span className="inline-flex items-center gap-2 font-medium">
                          <SolidSwatch color={r.color} />
                          {r.name}
                        </span>
                      </td>
                      <td className="py-2 pr-3">
                        {status === 'met' ? (
                          <Badge variant="green">Met</Badge>
                        ) : status === 'eliminated' ? (
                          <span title={eliminatedReasons.get(r.name)}>
                            <Badge variant="red">Eliminated</Badge>
                          </span>
                        ) : status === 'missed' ? (
                          <Badge variant="gray">Missed</Badge>
                        ) : status === 'steering' ? (
                          <Badge variant="orange">Steering · {((live?.steerRate ?? 0) * 100).toFixed(0)}%</Badge>
                        ) : status === 'on_pace' ? (
                          <Badge variant="green">On pace</Badge>
                        ) : (
                          <Badge variant="gray">Pending forecast</Badge>
                        )}
                      </td>
                      <td className="py-2 pr-3 text-right">{fmt(valueOf(r, 'before'))}</td>
                      <td className="py-2 pr-3 text-right">{fmt(valueOf(r, 'unaided'))}</td>
                      <td className="py-2 pr-3 text-right">{fmt(valueOf(r, 'steered'))}</td>
                      <td className="py-2 pr-3 text-right">{fmt(metric === 'volume' ? r.ceded : r.cededPayments)}</td>
                      <td className="py-2 pr-3 text-right font-semibold">
                        {fmt(metric === 'volume' ? total : r.unaidedPayments + r.steeredPayments)}
                        {metric === 'volume' && r.goal > 0 && (
                          <span className="ml-1 font-normal text-slate-400">· {pctOfGoal(total, r.goal)}%</span>
                        )}
                      </td>
                      <td className="py-2 pr-3 text-right">{compactAmount(r.goal)}</td>
                      <td className="py-2 text-right">{compactAmount(c?.reward ?? 0)}</td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            “Ceded” is what approval-rate routing would have sent a PSP but the engine moved to one
            behind on its commitment — the other side of every steered payment. Steering only happens
            inside the approval-rate tolerance, so approvals barely move.
          </p>
        </CardBody>
      </Card>

      {/* ── Chapter 3: pacing through the cycle ── */}
      <Card>
        <CardHeader>
          <SurfaceLabel>3 · Cumulative volume vs. each promise</SurfaceLabel>
          <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
            Solid lines are delivered volume; dashed steps are each PSP&apos;s per-day targets
          </h2>
          {isPastRun && (
            <p className="mt-2 text-xs text-amber-600 dark:text-amber-500">
              Showing the finished run {selectedRun} — its own delivery across its own cycle, not
              the one currently in flight.
            </p>
          )}
        </CardHeader>
        <CardBody>
          <CommitmentPacingChart
            connectors={connectors}
            currency={currency}
            daySecs={series.data?.daySecs}
            colorFor={pacingColorFor}
            statusFor={statusFor}
            eliminatedAtMs={eliminatedAtMs}
            eliminationReasons={eliminatedReasons}
            isPastRun={isPastRun}
            height={480}
          />
          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            Each dashed step is the volume a PSP must clear by that day&apos;s end (its promise split
            across the cycle). A solid line tracking below its ladder is behind pace — the engine
            steers a little extra volume there (▲), spread through the day. When not enough traffic remains
            to land a commitment the engine drops it (red marker): from there its line is dotted and
            carries natural traffic only, so the rest can still be met.
          </p>
        </CardBody>
      </Card>

      {/* ── Chapter 4: the audit trail ── */}
      <Card>
        <CardHeader>
          <SurfaceLabel>4 · Audit trail</SurfaceLabel>
          <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
            Every forecast, steer and elimination — what happened, when, and why
          </h2>
        </CardHeader>
        <CardBody>
          {audit.events.length === 0 ? (
            <p className="text-sm text-slate-500 dark:text-slate-400">
              Nothing yet. Entries appear when the scheduler runs a forecast or a payment is
              steered.
            </p>
          ) : (
            <div className="max-h-96 space-y-2 overflow-y-auto pr-1">
              {audit.events.slice(0, AUDIT_EVENTS_SHOWN).map((event, i) => {
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
                        {event.amount != null ? ` · ${compactAmount(event.amount)}` : ''}
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
