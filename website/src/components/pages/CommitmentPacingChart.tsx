import { cloneElement, useEffect, useMemo, useState, type ReactElement } from 'react'
import {
  CartesianGrid,
  ComposedChart,
  Customized,
  Line,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { CHART_TOOLTIP_LABEL_STYLE, CHART_TOOLTIP_STYLE } from '../../lib/chartStyles'
import { CommitmentConnectorSeries } from '../../types/api'
import { NEUTRAL_INK, SECS_PER_DAY, bucketsPerDay, dayUnit, formatMoney, formatMoneyExact } from './volumeCommitmentChartBits'

/** Where a PSP stands against its promise, as the chart marks it. */
export type PacingStatus = 'met' | 'steering' | 'on_pace' | 'eliminated' | 'missed' | 'pending'

const DROP_COLOR = '#dc2626'
const STEER_COLOR = '#d97706'
const MET_COLOR = '#059669'

type Scale = (value: number) => number | undefined
type AxisEntry = { scale: Scale }
type Offset = { top: number; left: number; width: number; height: number }

type ConnectorMeta = {
  name: string
  color: string
  goal: number
  reward: number
  status: PacingStatus
  /** Contract day the engine dropped it on, when it did. */
  dropDay?: number
  /** Last day its running total is drawn to: the end of the last bucket the series has reported
   *  on a live run (never "now" — what has not been measured yet is not drawn), the cycle end on
   *  a finished one. */
  endDay: number
  endTotal: number
  /** Recent delivery per contract day, for the tentative tail drawn from `endDay` to "now". */
  pace: number
}

/** One steered bucket: where the stretch starts and ends on the line, and how much moved. */
type Steer = { name: string; day: number; endDay: number; amount: number; total: number; startTotal: number; color: string }
type Drop = { day: number; names: string[]; reason?: string }

/** Non-line annotations (drop captions, steer triangles, promise labels), positioned via `<Customized>` axis scales. */
function PacingOverlay(props: {
  metas: ConnectorMeta[]
  steers: Steer[]
  drops: Drop[]
  currency?: string | null
  daysTotal: number
  dayLabel: string
  /** The stretch of the cycle on screen — the whole cycle, or a window that follows the run. */
  xLo: number
  xHi: number
  xAxisMap?: Record<string, AxisEntry>
  yAxisMap?: Record<string, AxisEntry>
  offset?: Offset
}) {
  const { metas, steers, drops, currency, daysTotal, dayLabel, xLo, xHi, xAxisMap, yAxisMap, offset } = props
  const xAxis = xAxisMap ? Object.values(xAxisMap)[0] : undefined
  const yAxis = yAxisMap ? Object.values(yAxisMap)[0] : undefined
  if (!xAxis || !yAxis || !offset) return null
  const x = (v: number) => xAxis.scale(v)
  const y = (v: number) => yAxis.scale(v)
  const rightEdge = x(xHi)
  if (rightEdge == null) return null
  const inView = (day: number) => day >= xLo - 1e-9 && day <= xHi + 1e-9

  // Promise labels: one per PSP where its promise line meets the right edge of the view (its goal
  // when the whole cycle is on screen), nudged apart when they sit close.
  const LABEL_GAP = 30
  const labels = metas
    .map((m) => ({ m, y: y((m.goal * xHi) / daysTotal) ?? offset.top }))
    .sort((a, b) => a.y - b.y)
  for (let i = 1; i < labels.length; i += 1) {
    if (labels[i].y - labels[i - 1].y < LABEL_GAP) labels[i].y = labels[i - 1].y + LABEL_GAP
  }
  const bottom = offset.top + offset.height - 6
  for (let i = labels.length - 1; i >= 0; i -= 1) {
    if (labels[i].y > bottom) labels[i].y = bottom
    if (i < labels.length - 1 && labels[i + 1].y - labels[i].y < LABEL_GAP) {
      labels[i].y = labels[i + 1].y - LABEL_GAP
    }
  }

  const biggestSteer = steers.reduce<Steer | null>((best, s) => (best && best.amount >= s.amount ? best : s), null)

  return (
    <g style={{ fontFamily: 'inherit' }}>
      {/* Drop captions along the top of the plot. */}
      {drops.map((d) => {
        if (!inView(d.day)) return null
        const dx = x(d.day)
        if (dx == null) return null
        const label = `${dayLabel} ${Math.round(d.day)} · ${d.names.join(', ')} dropped`
        const width = label.length * 6 + 10
        const anchorLeft = dx + width > offset.left + offset.width
        return (
          <g key={`drop-${d.day}`}>
            <rect
              x={anchorLeft ? dx - width - 4 : dx + 4}
              y={offset.top - 2}
              width={width}
              height={16}
              rx={3}
              fill={DROP_COLOR}
              opacity={0.1}
            />
            <text
              x={anchorLeft ? dx - 8 : dx + 9}
              y={offset.top + 10}
              fontSize={10.5}
              fontWeight={600}
              fill={DROP_COLOR}
              textAnchor={anchorLeft ? 'end' : 'start'}
            >
              {label}
            </text>
          </g>
        )
      })}

      {/* Steer chunks: a triangle on the line where the engine pushed volume across; the biggest
          chunk gets a label. */}
      {steers.map((s, i) => {
        if (!inView(s.day)) return null
        const sx = x(s.day)
        const sy = y(s.total)
        if (sx == null || sy == null) return null
        const isBiggest = biggestSteer === s
        return (
          <g key={`steer-${s.name}-${i}`}>
            <path d={`M${sx},${sy - 11} l4.5,7 h-9 z`} fill={STEER_COLOR} />
            {isBiggest && (
              <text x={sx + 7} y={sy - 6} fontSize={10.5} fontWeight={600} fill={STEER_COLOR}>
                steer +{formatMoney(s.amount, currency)}
              </text>
            )}
          </g>
        )
      })}

      {/* "dropped — natural traffic only" beside the gray tail, and "× missed" at its end. */}
      {metas
        .filter((m) => m.status === 'eliminated' || m.status === 'missed')
        .map((m) => {
          if (m.endDay < xLo) return null
          const ex = x(Math.min(m.endDay, xHi))
          const ey = y(m.endTotal)
          if (ex == null || ey == null) return null
          const tailStart = Math.max(m.dropDay ?? m.endDay, xLo)
          const tailEnd = Math.min(m.endDay, xHi)
          const midDay = (tailStart + tailEnd) / 2
          const mx = x(midDay)
          return (
            <g key={`tail-${m.name}`}>
              {m.dropDay != null && mx != null && tailEnd - tailStart > (xHi - xLo) * 0.12 && (
                <text x={mx} y={ey - 8} fontSize={10} fill={NEUTRAL_INK} textAnchor="middle">
                  dropped — natural traffic only
                </text>
              )}
              {m.status === 'missed' && m.endDay <= xHi + 1e-9 && (
                <text x={ex} y={ey - 7} fontSize={10.5} fontWeight={600} fill={DROP_COLOR} textAnchor="end">
                  × missed
                </text>
              )}
            </g>
          )
        })}

      {/* Promise labels down the right edge. */}
      {labels.map(({ m, y: ly }) => {
        const color = m.color
        const prefix = m.status === 'met' ? '✓ ' : m.status === 'steering' ? '↗ ' : ''
        return (
          <g key={`label-${m.name}`}>
            {m.status === 'met' && (
              <text x={rightEdge + 4} y={ly + 4} fontSize={11} fill={MET_COLOR} fontWeight={700}>
                ✓
              </text>
            )}
            <text
              x={rightEdge + (m.status === 'met' ? 16 : 8)}
              y={ly + 4}
              fontSize={10.5}
              fontWeight={600}
              fill={color}
            >
              {m.status === 'met' ? '' : prefix}
              {m.name} promise · {formatMoney(m.goal, currency)}
            </text>
            <text x={rightEdge + (m.status === 'met' ? 16 : 8)} y={ly + 17} fontSize={10.5} fill={color} opacity={0.85}>
              → {formatMoneyExact(m.reward, currency)}
              {m.status === 'steering' ? ' · steering' : ''}
            </text>
          </g>
        )
      })}
    </g>
  )
}

/** `ResponsiveContainer` normally; a fixed width renders to markup for SSR previews and tests. */
function Sized({ width, height, children }: { width?: number; height: number; children: ReactElement }) {
  if (width) return cloneElement(children, { width, height })
  return <ResponsiveContainer>{children}</ResponsiveContainer>
}

function PacingTooltip({
  active,
  payload,
  label,
  metas,
  currency,
  dayLabel,
}: {
  active?: boolean
  payload?: Array<{ payload?: Record<string, number | undefined> }>
  label?: number
  metas: ConnectorMeta[]
  currency?: string | null
  dayLabel: string
}) {
  if (!active || !payload?.length) return null
  const row = payload[0]?.payload
  if (!row) return null
  return (
    <div style={{ ...CHART_TOOLTIP_STYLE, fontSize: 12, lineHeight: 1.5, minWidth: 220 }}>
      <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>
        {dayLabel} {typeof label === 'number' ? label.toFixed(label % 1 === 0 ? 0 : 1) : label}
      </p>
      {metas.map((m) => {
        const measured = row[m.name] ?? row[`${m.name}__after`]
        const pending = measured == null ? row[`${m.name}__pending`] : undefined
        const total = measured ?? pending
        const promise = row[`${m.name}__promise`]
        return (
          <div key={m.name} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ width: 8, height: 8, borderRadius: 999, backgroundColor: m.color, flexShrink: 0 }} />
            <span style={{ fontWeight: 600 }}>{m.name}</span>
            <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>
              {total != null ? `${pending != null ? '~' : ''}${formatMoney(total, currency)}` : '—'}
              <span style={{ opacity: 0.6 }}> / {promise != null ? formatMoney(promise, currency) : '—'}</span>
            </span>
          </div>
        )
      })}
    </div>
  )
}

/** Running total per PSP vs its dashed promise, with steer triangles and drop markers; dropped PSPs keep their color. */
export function CommitmentPacingChart({
  connectors,
  currency,
  daySecs,
  colorFor,
  statusFor,
  eliminatedAtMs,
  eliminationReasons,
  isPastRun,
  height = 460,
  showLegend = true,
  fixedWidth,
  perDay,
}: {
  connectors: CommitmentConnectorSeries[]
  currency?: string | null
  daySecs?: number | null
  colorFor: (connector: string, index: number) => string
  statusFor: (connector: string) => PacingStatus
  /** When the engine dropped each PSP, from the audit trail. */
  eliminatedAtMs?: Map<string, number>
  eliminationReasons?: Map<string, string>
  isPastRun?: boolean
  height?: number
  showLegend?: boolean
  /** Render at a fixed pixel width instead of filling the container — for server-side previews. */
  fixedWidth?: number
  /** Buckets per contract day the series was fetched with; defaults to what the pages use. */
  perDay?: number
}) {
  const dayMs = Math.max(1, (daySecs ?? SECS_PER_DAY) * 1000)
  const bucketDays = 1 / Math.max(1, perDay ?? bucketsPerDay(daySecs))

  // A live run re-renders on its own clock so the tentative tail keeps pace with "now" between
  // polls; a finished run has nothing to advance.
  const [, setTick] = useState(0)
  useEffect(() => {
    if (isPastRun) return undefined
    const id = window.setInterval(() => setTick((t) => t + 1), 500)
    return () => window.clearInterval(id)
  }, [isPastRun])
  const dayLabel = dayUnit(daySecs).short
  const daysTotal = Math.max(1, ...connectors.map((c) => c.daysTotal))
  const cycleStartMs = Math.min(...connectors.map((c) => Date.parse(c.cycleStart) || Number.POSITIVE_INFINITY))
  const nowDay = Number.isFinite(cycleStartMs)
    ? Math.min(daysTotal, Math.max(0, (Date.now() - cycleStartMs) / dayMs))
    : daysTotal

  const { rows, metas, steers, drops, yMax, yTicks } = useMemo(() => {
    const clampDay = (day: number) => Math.min(daysTotal, day)
    const dayOf = (ms: number) => clampDay(Math.max(0, (ms - cycleStartMs) / dayMs))
    // The run's right edge: the cycle end once it is over, else this instant. Nothing is drawn
    // past it, and a bucket still in progress is cut off at it.
    const edge = isPastRun ? daysTotal : nowDay

    // Each PSP's line as nodes: the running total at the *end* of every reported bucket (so a
    // bucket's delivery is drawn across the time it happened in), with a hold node where reported
    // buckets are not adjacent — the series omits empty buckets, and a gap with data on both
    // sides is a genuinely quiet stretch, which is flat, not a slope. Nothing is added after the
    // last reported bucket: a silent tail is either ingestion lag or quiet, and drawing it flat
    // to "now" would have to be redrawn as a slope once the late buckets arrived.
    type Node = { at: number; total: number; steered: number }
    const PACE_BUCKETS = 3
    const nodesFor = (c: CommitmentConnectorSeries): { nodes: Node[]; pace: number } => {
      const pts = [...c.points].sort((a, b) => a.day - b.day)
      const nodes: Node[] = []
      let running = 0
      let prevDay: number | null = null
      for (const p of pts) {
        const day = clampDay(p.day)
        if (prevDay != null && day - prevDay > bucketDays * 1.5) {
          nodes.push({ at: Math.min(day, edge), total: running, steered: 0 })
        }
        running += p.total
        nodes.push({ at: Math.min(day + bucketDays, edge), total: running, steered: p.steered })
        prevDay = day
      }
      // Recent pace from the last few *complete* buckets — one still in progress (cut at the
      // edge) would understate it. Measured over the span they cover, so a quiet bucket counts.
      const complete = pts.filter((p) => clampDay(p.day) + bucketDays <= edge + 1e-9)
      const recent = complete.slice(-PACE_BUCKETS)
      const span = recent.length ? clampDay(recent[recent.length - 1].day) + bucketDays - clampDay(recent[0].day) : 0
      const pace = span > 0 ? recent.reduce((sum, p) => sum + p.total, 0) / span : 0
      return { nodes, pace }
    }
    const nodesByName = new Map(connectors.map((c) => [c.connector, nodesFor(c)]))

    const metas: ConnectorMeta[] = connectors.map((c, i) => {
      const status = statusFor(c.connector)
      const droppedAt = eliminatedAtMs?.get(c.connector)
      const dropDay =
        status === 'eliminated' || status === 'missed'
          ? droppedAt != null && Number.isFinite(cycleStartMs)
            ? dayOf(droppedAt)
            : undefined
          : undefined
      const { nodes, pace } = nodesByName.get(c.connector) ?? { nodes: [], pace: 0 }
      return {
        name: c.connector,
        color: colorFor(c.connector, i),
        goal: c.goal,
        reward: c.reward,
        status,
        dropDay,
        endDay: isPastRun ? daysTotal : nodes.length ? nodes[nodes.length - 1].at : 0,
        endTotal: 0,
        pace,
      }
    })

    // Every instant any PSP's line bends at, plus the ends of the promise lines and the drops.
    const dayset = new Set<number>([0, daysTotal, ...(isPastRun ? [] : [nowDay])])
    for (const { nodes } of nodesByName.values()) for (const n of nodes) dayset.add(n.at)
    for (const m of metas) if (m.dropDay != null) dayset.add(m.dropDay)
    const days = [...dayset].sort((a, b) => a - b)

    const rows: Record<string, number | undefined>[] = []
    const running: Record<string, number> = {}
    const pointer: Record<string, number> = {}
    const steers: Steer[] = []
    for (const day of days) {
      const row: Record<string, number | undefined> = { day }
      for (const m of metas) {
        const nodes = nodesByName.get(m.name)?.nodes ?? []
        let idx = pointer[m.name] ?? 0
        while (idx < nodes.length && nodes[idx].at <= day + 1e-9) {
          const startTotal = running[m.name] ?? 0
          running[m.name] = nodes[idx].total
          if (nodes[idx].steered > 0) {
            // The triangle sits on the line where the steered bucket ends.
            steers.push({
              name: m.name,
              day: nodes[idx].at,
              endDay: nodes[idx].at,
              amount: nodes[idx].steered,
              total: nodes[idx].total,
              startTotal,
              color: m.color,
            })
          }
          idx += 1
        }
        pointer[m.name] = idx
        const total = running[m.name] ?? 0
        if (day <= m.endDay + 1e-9) {
          if (m.dropDay != null && day >= m.dropDay) {
            row[`${m.name}__after`] = total
            // Share the drop point so the colored line hands over to the gray one without a gap.
            if (day === m.dropDay) row[m.name] = total
          } else {
            row[m.name] = total
          }
          m.endTotal = total
        }
        // The stretch not yet measured: from the last bucket to "now", projected at the recent
        // pace and drawn tentatively. Shares its first point with the line so there is no gap.
        if (!isPastRun && nowDay > m.endDay + 1e-9 && (day === nowDay || Math.abs(day - m.endDay) < 1e-9)) {
          row[`${m.name}__pending`] = m.endTotal + m.pace * (day - m.endDay)
        }
        row[`${m.name}__promise`] = (m.goal * clampDay(day)) / daysTotal
      }
      rows.push(row)
    }

    // Steer triangles: one per bucket is noise on a long cycle — keep the largest few per PSP.
    const MAX_STEERS_PER_PSP = 6
    const keptSteers = metas.flatMap((m) =>
      steers
        .filter((s) => s.name === m.name)
        .sort((a, b) => b.amount - a.amount)
        .slice(0, MAX_STEERS_PER_PSP),
    )

    const dropsByDay = new Map<number, Drop>()
    for (const m of metas) {
      if (m.dropDay == null) continue
      const key = Math.round(m.dropDay * 100) / 100
      const drop = dropsByDay.get(key) ?? { day: m.dropDay, names: [], reason: eliminationReasons?.get(m.name) }
      drop.names.push(m.name)
      dropsByDay.set(key, drop)
    }
    const drops = [...dropsByDay.values()].sort((a, b) => a.day - b.day)

    // Round the axis up to a clean step so the ticks read $2.5M / $5M rather than $8.7M.
    const projected = (m: ConnectorMeta) => (isPastRun ? m.endTotal : m.endTotal + m.pace * Math.max(0, nowDay - m.endDay))
    const raw = Math.max(1, ...metas.map((m) => Math.max(m.goal, m.endTotal, projected(m)))) * 1.05
    const magnitude = 10 ** Math.floor(Math.log10(raw))
    const unit = raw / magnitude
    const step = (unit <= 2 ? 0.5 : unit <= 5 ? 1 : 2.5) * magnitude
    const yMax = Math.ceil(raw / step) * step
    const yTicks: number[] = []
    for (let v = 0; v <= yMax + 1e-9; v += step) yTicks.push(v)
    return { rows, metas, steers: keptSteers, drops, yMax, yTicks }
  }, [connectors, colorFor, statusFor, eliminatedAtMs, eliminationReasons, isPastRun, daysTotal, nowDay, cycleStartMs, dayMs, bucketDays])

  // ── The window: a slice of the cycle that follows the run, or the whole cycle ("All"). ──────
  // Opens on the first contract day so an early run is legible instead of a sliver in the
  // bottom-left of a whole-cycle axis, then slides forward with the run; "All" unzooms.
  const windowOptions = useMemo(
    () => (daysTotal <= 8 ? [1, 2] : [7, 14]).filter((w) => w < daysTotal),
    [daysTotal],
  )
  const [windowSel, setWindowSel] = useState<number | 'all'>(daysTotal <= 8 ? 1 : 7)
  const win = windowSel === 'all' || !windowOptions.includes(windowSel) ? null : windowSel
  const runEdge = isPastRun ? daysTotal : nowDay
  const xHi = win == null ? daysTotal : Math.min(daysTotal, Math.max(win, runEdge))
  const xLo = win == null ? 0 : Math.max(0, xHi - win)

  const ticks = useMemo(() => {
    const span = xHi - xLo
    // Inside a window, snap the step to a clean value (¼, ½, 1, 2, 5…) so ticks read
    // "Min 0.75" rather than "Min 0.85"; the whole cycle keeps its usual spacing.
    const NICE = [0.25, 0.5, 1, 2, 5, 10, 15, 30]
    const step =
      win != null
        ? [...NICE].reverse().find((n) => n <= span / 4) ?? 0.25
        : daysTotal <= 8 ? 1 : daysTotal <= 16 ? 2 : daysTotal <= 40 ? 6 : Math.ceil(daysTotal / 6)
    const out: number[] = []
    const first = win != null ? Math.ceil(xLo / step) * step : xLo
    for (let d = first; d < xHi - 1e-9; d += step) out.push(Math.round(d * 100) / 100)
    out.push(Math.round(xHi * 100) / 100)
    return out
  }, [daysTotal, win, xLo, xHi])

  // The value axis follows the window too: zoomed to what is on screen, with clean steps.
  const { yLo, yHi, yTicksInView } = useMemo(() => {
    if (win == null) return { yLo: 0, yHi: yMax, yTicksInView: yTicks }
    const values: number[] = []
    for (const m of metas) {
      values.push((m.goal * xLo) / daysTotal, (m.goal * xHi) / daysTotal)
    }
    for (const row of rows) {
      const day = Number(row.day ?? 0)
      if (day < xLo - 1e-9 || day > xHi + 1e-9) continue
      for (const m of metas) {
        for (const key of [m.name, `${m.name}__after`, `${m.name}__pending`]) {
          const v = row[key]
          if (typeof v === 'number') values.push(v)
        }
      }
    }
    const lo = Math.max(0, Math.min(...values, Number.POSITIVE_INFINITY))
    const hi = Math.max(...values, 1)
    const span = Math.max(hi - lo, hi * 0.1, 1)
    const magnitude = 10 ** Math.floor(Math.log10(span))
    const unit = span / magnitude
    const step = (unit <= 2 ? 0.5 : unit <= 5 ? 1 : 2.5) * magnitude
    const floor = Math.max(0, Math.floor((lo - span * 0.05) / step) * step)
    const ceil = Math.ceil((hi + span * 0.08) / step) * step
    const t: number[] = []
    for (let v = floor; v <= ceil + 1e-9; v += step) t.push(v)
    return { yLo: floor, yHi: ceil, yTicksInView: t }
  }, [win, rows, metas, xLo, xHi, daysTotal, yMax, yTicks])

  const anyDrop = drops.length > 0
  const anySteer = steers.length > 0

  const windowLabel = (w: number) => `Last ${w} ${w === 1 ? 'day' : 'days'}`

  return (
    <div className="w-full">
      {windowOptions.length > 0 && (
        <div className="mb-2 flex justify-end">
          <div className="inline-flex rounded-md border border-slate-200 p-0.5 dark:border-[#1f1f29]">
            {[...windowOptions, 'all' as const].map((opt) => {
              const active = opt === 'all' ? win == null : win === opt
              return (
                <button
                  key={String(opt)}
                  type="button"
                  onClick={() => setWindowSel(opt)}
                  className={`rounded px-2.5 py-1 text-[11px] font-medium transition-colors ${
                    active
                      ? 'bg-brand-500 text-white'
                      : 'text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200'
                  }`}
                >
                  {opt === 'all' ? 'All' : windowLabel(opt)}
                </button>
              )
            })}
          </div>
        </div>
      )}
      <div className="w-full" style={{ height }}>
        <Sized width={fixedWidth} height={height}>
          <ComposedChart data={rows} margin={{ top: 20, right: 196, bottom: 4, left: 8 }}>
            <CartesianGrid vertical={false} stroke="currentColor" opacity={0.08} />
            <XAxis
              type="number"
              dataKey="day"
              domain={[xLo, xHi]}
              allowDataOverflow
              ticks={ticks}
              tickFormatter={(d: number) => `${dayLabel} ${Number.isInteger(d) ? d : d.toFixed(2).replace(/0+$/, '')}`}
              tick={{ fontSize: 11 }}
              stroke="currentColor"
              opacity={0.5}
              tickLine={false}
            />
            <YAxis
              domain={[yLo, yHi]}
              allowDataOverflow
              ticks={yTicksInView}
              tickFormatter={(v: number) => formatMoney(v, currency)}
              tick={{ fontSize: 11 }}
              stroke="currentColor"
              opacity={0.5}
              width={60}
              tickLine={false}
            />
            <Tooltip
              content={<PacingTooltip metas={metas} currency={currency} dayLabel={dayLabel} />}
              wrapperStyle={{ zIndex: 30, outline: 'none' }}
              cursor={{ stroke: 'currentColor', opacity: 0.2 }}
            />
            {drops
              .filter((d) => d.day <= xHi)
              .map((d) => (
                <ReferenceArea key={`area-${d.day}`} x1={Math.max(d.day, xLo)} x2={xHi} fill={DROP_COLOR} fillOpacity={0.06} strokeOpacity={0} />
              ))}
            {drops
              .filter((d) => d.day >= xLo && d.day <= xHi)
              .map((d) => (
                <ReferenceLine key={`line-${d.day}`} x={d.day} stroke={DROP_COLOR} strokeDasharray="2 3" strokeWidth={1.5} />
              ))}
            {metas.map((m) => (
              <Line
                key={`${m.name}-promise`}
                type="linear"
                dataKey={`${m.name}__promise`}
                stroke={m.color}
                strokeWidth={1.5}
                strokeDasharray="6 4"
                opacity={0.55}
                dot={false}
                activeDot={false}
                isAnimationActive={false}
              />
            ))}
            {!isPastRun &&
              metas.map((m) => (
                <Line
                  key={`${m.name}-pending`}
                  type="linear"
                  dataKey={`${m.name}__pending`}
                  stroke={m.color}
                  strokeWidth={2.25}
                  strokeDasharray="1 5"
                  strokeLinecap="round"
                  opacity={0.45}
                  dot={false}
                  activeDot={false}
                  isAnimationActive={false}
                  connectNulls={false}
                />
              ))}
            {metas.map((m) => (
              <Line
                key={m.name}
                type="linear"
                dataKey={m.name}
                stroke={m.color}
                strokeWidth={2.25}
                dot={false}
                activeDot={{ r: 3.5, strokeWidth: 0 }}
                isAnimationActive={false}
                connectNulls={false}
              />
            ))}
            {metas
              .filter((m) => m.dropDay != null)
              .map((m) => (
                <Line
                  key={`${m.name}-after`}
                  type="linear"
                  dataKey={`${m.name}__after`}
                  stroke={m.color}
                  strokeWidth={2}
                  strokeDasharray="2 3"
                  opacity={0.6}
                  dot={false}
                  activeDot={false}
                  isAnimationActive={false}
                  connectNulls={false}
                />
              ))}
            <Customized
              component={
                <PacingOverlay metas={metas} steers={steers} drops={drops} currency={currency} daysTotal={daysTotal} dayLabel={dayLabel} xLo={xLo} xHi={xHi} />
              }
            />
          </ComposedChart>
        </Sized>
      </div>
      {showLegend && (
        <div className="mt-2 flex flex-wrap items-center gap-x-5 gap-y-1.5 text-[11px] text-slate-600 dark:text-slate-300">
          {metas.map((m) => (
            <span key={m.name} className="inline-flex items-center gap-1.5">
              <span
                className="inline-block h-0.5 w-4 rounded"
                style={{ backgroundColor: m.color }}
              />
              {m.name} — running total
            </span>
          ))}
          <span className="inline-flex items-center gap-1.5 text-slate-500 dark:text-slate-400">
            <span className="inline-block h-0 w-4 border-t-[1.5px] border-dashed border-slate-500 dark:border-slate-400" />
            dashed = each PSP&apos;s promise
          </span>
          {!isPastRun && (
            <span className="inline-flex items-center gap-1.5 text-slate-500 dark:text-slate-400">
              <span className="inline-block h-0 w-4 border-t-2 border-dotted border-slate-400 dark:border-slate-500" />
              dotted = not yet measured, at recent pace
            </span>
          )}
          {anySteer && (
            <span className="inline-flex items-center gap-1.5 text-slate-500 dark:text-slate-400">
              <span style={{ color: STEER_COLOR }}>▲</span> steered chunk
            </span>
          )}
          {anyDrop && (
            <span className="inline-flex items-center gap-1.5 text-slate-500 dark:text-slate-400">
              <span className="inline-block h-0 w-4 border-t-[1.5px] border-dotted" style={{ borderColor: DROP_COLOR }} />
              commitment dropped
            </span>
          )}
        </div>
      )}
    </div>
  )
}

/** One card per PSP: promise, reward terms, reward, and current standing by badge. */
export function CommitmentContractCards({
  connectors,
  currency,
  colorFor,
  statusFor,
  achievedFor,
  reasonFor,
}: {
  connectors: CommitmentConnectorSeries[]
  currency?: string | null
  colorFor: (connector: string, index: number) => string
  statusFor: (connector: string) => PacingStatus
  achievedFor: (connector: string) => number
  reasonFor?: (connector: string) => string | undefined
}) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {connectors.map((c, i) => {
        const status = statusFor(c.connector)
        const color = colorFor(c.connector, i)
        const achieved = achievedFor(c.connector)
        const pct = c.goal > 0 ? Math.min(100, (achieved / c.goal) * 100) : 0
        const goalText = formatMoney(c.goal, currency)
        const isRebate = c.rewardNote.includes('%')
        const rebate = isRebate ? c.rewardNote.split(' ')[0] : null
        const chip =
          status === 'met'
            ? { text: 'Met', cls: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-300' }
            : status === 'steering'
              ? { text: 'Steering', cls: 'bg-amber-500/10 text-amber-600 dark:text-amber-300' }
              : status === 'eliminated'
                ? { text: 'Eliminated', cls: 'bg-red-500/10 text-red-600 dark:text-red-300' }
                : status === 'missed'
                  ? { text: 'Missed', cls: 'bg-slate-500/10 text-slate-600 dark:text-slate-300' }
                  : status === 'on_pace'
                    ? { text: 'On pace', cls: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-300' }
                    : { text: 'Pending', cls: 'bg-slate-500/10 text-slate-600 dark:text-slate-300' }
        return (
          <div
            key={c.connector}
            title={reasonFor?.(c.connector)}
            className="rounded-2xl border border-slate-200 bg-white px-4 py-3 shadow-sm dark:border-[#2a303a] dark:bg-[#11151d]"
            style={{ borderLeft: `4px solid ${color}` }}
          >
            <div className="flex items-center justify-between gap-2">
              <span className="flex items-center gap-2 text-sm font-semibold text-slate-800 dark:text-white">
                <span className="inline-block h-2 w-2 rounded-full" style={{ backgroundColor: color }} />
                {c.connector}
              </span>
              <span className={`rounded-md px-1.5 py-0.5 text-[10px] font-medium ${chip.cls}`}>{chip.text}</span>
            </div>
            <p className="mt-1 text-xs text-slate-500 dark:text-slate-400">
              Promise <strong className="text-slate-700 dark:text-slate-200">{goalText}</strong> · {c.rewardNote}
            </p>
            <p className="mt-1.5 text-2xl font-semibold" style={{ color }}>
              {formatMoneyExact(c.reward, currency)}
            </p>
            <p className="mt-0.5 text-[11px] text-slate-400 dark:text-slate-500">
              {rebate ? `${rebate} × ${goalText}` : `flat on hitting ${goalText}`}
            </p>
            <p className="mt-1 text-[11px] tabular-nums text-slate-500 dark:text-slate-400">
              {formatMoney(achieved, currency)} delivered · {pct.toFixed(0)}%
            </p>
          </div>
        )
      })}
    </div>
  )
}
