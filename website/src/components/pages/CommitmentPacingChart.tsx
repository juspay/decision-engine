import { cloneElement, useMemo, type ReactElement } from 'react'
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
import { NEUTRAL_INK, SECS_PER_DAY, dayUnit, formatMoney, formatMoneyExact } from './volumeCommitmentChartBits'

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
  /** Last day its running total is drawn to. */
  endDay: number
  endTotal: number
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
  xAxisMap?: Record<string, AxisEntry>
  yAxisMap?: Record<string, AxisEntry>
  offset?: Offset
}) {
  const { metas, steers, drops, currency, daysTotal, dayLabel, xAxisMap, yAxisMap, offset } = props
  const xAxis = xAxisMap ? Object.values(xAxisMap)[0] : undefined
  const yAxis = yAxisMap ? Object.values(yAxisMap)[0] : undefined
  if (!xAxis || !yAxis || !offset) return null
  const x = (v: number) => xAxis.scale(v)
  const y = (v: number) => yAxis.scale(v)
  const rightEdge = x(daysTotal)
  if (rightEdge == null) return null

  // Promise labels: one per PSP at the height of its goal, nudged apart when goals sit close.
  const LABEL_GAP = 30
  const labels = metas
    .map((m) => ({ m, y: y(m.goal) ?? offset.top }))
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
          const ex = x(m.endDay)
          const ey = y(m.endTotal)
          if (ex == null || ey == null) return null
          const midDay = m.dropDay != null ? (m.dropDay + m.endDay) / 2 : m.endDay
          const mx = x(midDay)
          return (
            <g key={`tail-${m.name}`}>
              {m.dropDay != null && mx != null && m.endDay - m.dropDay > daysTotal * 0.12 && (
                <text x={mx} y={ey - 8} fontSize={10} fill={NEUTRAL_INK} textAnchor="middle">
                  dropped — natural traffic only
                </text>
              )}
              {m.status === 'missed' && (
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
        const total = row[m.name] ?? row[`${m.name}__after`]
        const promise = row[`${m.name}__promise`]
        return (
          <div key={m.name} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ width: 8, height: 8, borderRadius: 999, backgroundColor: m.color, flexShrink: 0 }} />
            <span style={{ fontWeight: 600 }}>{m.name}</span>
            <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>
              {total != null ? formatMoney(total, currency) : '—'}
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
}) {
  const dayMs = Math.max(1, (daySecs ?? SECS_PER_DAY) * 1000)
  const dayLabel = dayUnit(daySecs).short
  const daysTotal = Math.max(1, ...connectors.map((c) => c.daysTotal))
  const cycleStartMs = Math.min(...connectors.map((c) => Date.parse(c.cycleStart) || Number.POSITIVE_INFINITY))
  const nowDay = Number.isFinite(cycleStartMs)
    ? Math.min(daysTotal, Math.max(0, (Date.now() - cycleStartMs) / dayMs))
    : daysTotal

  const { rows, metas, steers, drops, yMax, yTicks } = useMemo(() => {
    const clampDay = (day: number) => Math.min(daysTotal, day)
    const dayOf = (ms: number) => clampDay(Math.max(0, (ms - cycleStartMs) / dayMs))
    const metas: ConnectorMeta[] = connectors.map((c, i) => {
      const status = statusFor(c.connector)
      const droppedAt = eliminatedAtMs?.get(c.connector)
      const dropDay =
        status === 'eliminated' || status === 'missed'
          ? droppedAt != null && Number.isFinite(cycleStartMs)
            ? dayOf(droppedAt)
            : undefined
          : undefined
      return {
        name: c.connector,
        color: colorFor(c.connector, i),
        goal: c.goal,
        reward: c.reward,
        status,
        dropDay,
        endDay: isPastRun ? daysTotal : nowDay,
        endTotal: 0,
      }
    })

    // Every distinct bucket start any PSP reported, plus the ends of the lines.
    const dayset = new Set<number>([0, daysTotal, ...(isPastRun ? [] : [nowDay])])
    for (const c of connectors) for (const p of c.points) dayset.add(clampDay(p.day))
    for (const m of metas) if (m.dropDay != null) dayset.add(m.dropDay)
    const days = [...dayset].sort((a, b) => a - b)

    const rows: Record<string, number | undefined>[] = []
    const running: Record<string, number> = {}
    const pointer: Record<string, number> = {}
    const sortedPoints = new Map(connectors.map((c) => [c.connector, [...c.points].sort((a, b) => a.day - b.day)]))
    const steers: Steer[] = []
    for (const day of days) {
      const row: Record<string, number | undefined> = { day }
      for (const m of metas) {
        const pts = sortedPoints.get(m.name) ?? []
        let idx = pointer[m.name] ?? 0
        while (idx < pts.length && clampDay(pts[idx].day) <= day) {
          const startTotal = running[m.name] ?? 0
          running[m.name] = startTotal + pts[idx].total
          if (pts[idx].steered > 0) {
            // A bucket's delivery is drawn between its start and the next bucket's start.
            const bucketStart = clampDay(pts[idx].day)
            const next = pts[idx + 1] ? clampDay(pts[idx + 1].day) : clampDay(m.endDay)
            steers.push({
              name: m.name,
              day: bucketStart,
              endDay: Math.max(bucketStart, next),
              amount: pts[idx].steered,
              total: running[m.name],
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
    const raw = Math.max(1, ...metas.map((m) => Math.max(m.goal, m.endTotal))) * 1.05
    const magnitude = 10 ** Math.floor(Math.log10(raw))
    const unit = raw / magnitude
    const step = (unit <= 2 ? 0.5 : unit <= 5 ? 1 : 2.5) * magnitude
    const yMax = Math.ceil(raw / step) * step
    const yTicks: number[] = []
    for (let v = 0; v <= yMax + 1e-9; v += step) yTicks.push(v)
    return { rows, metas, steers: keptSteers, drops, yMax, yTicks }
  }, [connectors, colorFor, statusFor, eliminatedAtMs, eliminationReasons, isPastRun, daysTotal, nowDay, cycleStartMs, dayMs])

  const ticks = useMemo(() => {
    const step = daysTotal <= 8 ? 1 : daysTotal <= 16 ? 2 : daysTotal <= 40 ? 6 : Math.ceil(daysTotal / 6)
    const out: number[] = []
    for (let d = 0; d < daysTotal; d += step) out.push(d)
    out.push(daysTotal)
    return out
  }, [daysTotal])

  const anyDrop = drops.length > 0
  const anySteer = steers.length > 0

  return (
    <div className="w-full">
      <div className="w-full" style={{ height }}>
        <Sized width={fixedWidth} height={height}>
          <ComposedChart data={rows} margin={{ top: 20, right: 196, bottom: 4, left: 8 }}>
            <CartesianGrid vertical={false} stroke="currentColor" opacity={0.08} />
            <XAxis
              type="number"
              dataKey="day"
              domain={[0, daysTotal]}
              ticks={ticks}
              tickFormatter={(d: number) => `${dayLabel} ${d}`}
              tick={{ fontSize: 11 }}
              stroke="currentColor"
              opacity={0.5}
              tickLine={false}
            />
            <YAxis
              domain={[0, yMax]}
              ticks={yTicks}
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
            {drops.map((d) => (
              <ReferenceArea key={`area-${d.day}`} x1={d.day} x2={daysTotal} fill={DROP_COLOR} fillOpacity={0.06} strokeOpacity={0} />
            ))}
            {drops.map((d) => (
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
                <PacingOverlay metas={metas} steers={steers} drops={drops} currency={currency} daysTotal={daysTotal} dayLabel={dayLabel} />
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
