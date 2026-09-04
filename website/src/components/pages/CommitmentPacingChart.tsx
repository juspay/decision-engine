import { cloneElement, useCallback, useEffect, useId, useMemo, useState, type ReactElement } from 'react'
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
import { NEUTRAL_INK, SECS_PER_DAY, bucketsPerDay, dayUnit, formatAchieved, formatMoney, formatMoneyExact, isTestCycle, pctOfGoal } from './volumeCommitmentChartBits'

/** Where a PSP stands against its promise, as the chart marks it. */
export type PacingStatus = 'met' | 'steering' | 'on_pace' | 'eliminated' | 'missed' | 'pending'

const DROP_COLOR = '#dc2626'
const STEER_COLOR = '#d97706'
const MET_COLOR = '#059669'
/** What a dropped PSP's remaining line carries. */
const TAIL_NOTE = 'dropped — natural traffic only'

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
  /** Recent delivery per contract day, for the tentative tail drawn from `endDay` to `pendingDay`. */
  pace: number
  /** Where this PSP's own cycle opens on the shared axis, in days. Zero unless the document mixes
   *  billing cycles, in which case only the earliest one starts at the origin. */
  offset: number
  /** Days in this PSP's own cycle — the span its promise line ramps across, which is not the axis
   *  span when the document mixes cycles. */
  cycleDays: number
  /** Where the tentative tail stops: "now", or a couple of buckets past the last measurement,
   *  whichever comes first. */
  pendingDay: number
  /** What it still has to deliver per contract day to land the promise, counted from its last
   *  reading to its own cycle close. The dashed promise line's slope is the promise's *original*
   *  even pace; this one steepens as a PSP falls behind, which is the number that decides whether
   *  the engine steers. */
  neededDaily: number
  /** Whether the running total sits below where the promise expects it by now. */
  behind: boolean
  /** Share of the delivered total that steering added. Below a couple of percent the band is a
   *  hairline tracing the running total, which reads as a second line drawn over the first rather
   *  than as an area — so it is not drawn at all. */
  steeredShare: number
  /** The counterfactual total at `endDay`: where this PSP would stand unaided. */
  endUnaided: number
}

/** Below this, the steered wedge is not worth drawing — see `steeredShare`. */
const VISIBLE_BAND_SHARE = 0.02

/** What a PSP's promise expects it to have delivered by `day` — the height of its dashed line.
 *  Counted through that PSP's *own* cycle, which is not the axis span when a document mixes
 *  billing cycles. */
function promiseAt(m: ConnectorMeta, day: number) {
  return m.goal * Math.min(1, Math.max(0, (day - m.offset) / m.cycleDays))
}

/** A round axis step for a span, so ticks land on 0.25 / 0.5 / 1 / 2.5 of a power of ten. The
 *  0.25 rung matters: without it a span just over a power of ten rounds up to half again its own
 *  height, and the plot spends that share of itself empty. */
function niceStep(span: number) {
  const magnitude = 10 ** Math.floor(Math.log10(Math.max(span, 1e-9)))
  const unit = span / magnitude
  return (unit <= 1.25 ? 0.25 : unit <= 2 ? 0.5 : unit <= 5 ? 1 : 2.5) * magnitude
}

/** Roughly the pixel width of `text` at the 10.5px the plot labels use. */
function approxTextWidth(text: string) {
  return text.length * 5.6
}

/** A PSP short of its promise's pace, with a cycle still open to make it up in. */
function needsPace(m: ConnectorMeta) {
  return m.behind && m.neededDaily > 0 && m.status !== 'met' && m.status !== 'eliminated' && m.status !== 'missed'
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
  dayLabel: string
  /** The stretch of the cycle on screen — the whole cycle, or a window that follows the run. */
  xLo: number
  xHi: number
  xAxisMap?: Record<string, AxisEntry>
  yAxisMap?: Record<string, AxisEntry>
  offset?: Offset
}) {
  const { metas, steers, drops, currency, dayLabel, xLo, xHi, xAxisMap, yAxisMap, offset } = props
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
  // Every label is two lines, so one gap serves the whole spreading pass.
  const LABEL_GAP = 30
  const labels = metas
    .map((m) => ({ m, y: y(promiseAt(m, xHi)) ?? offset.top }))
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

  // Labels drawn on the plot itself — the steer amount, the dropped-tail note, "× missed" — are
  // positioned relative to the line they belong to, and different lines cross. Each claims the
  // space it needs; a label whose space is taken tries once higher, then gives up rather than
  // printing over the one already there. The marks themselves (triangle, dotted tail) always draw.
  const claimed: Array<[number, number, number, number]> = []
  const claim = (cx: number, cy: number, width: number, anchorEnd = false) => {
    const x0 = anchorEnd ? cx - width : cx
    const box: [number, number, number, number] = [x0, cy - 11, x0 + width, cy + 3]
    const free = claimed.every(
      ([a, b, c, d]) => box[2] < a || box[0] > c || box[3] < b || box[1] > d,
    )
    if (free) claimed.push(box)
    return free
  }
  /** Vertical pitch of stacked drop captions; a caption is 16 tall. */
  const CAPTION_ROW_HEIGHT = 18
  /** Horizontal spans already taken, per row, so a caption drops to the first row it fits on. */
  const captionRows: Array<Array<[number, number]>> = []
  const captionRow = (x0: number, x1: number) => {
    const row = captionRows.findIndex((taken) => taken.every(([a, b]) => x1 < a || x0 > b))
    if (row >= 0) {
      captionRows[row].push([x0, x1])
      return row
    }
    captionRows.push([[x0, x1]])
    return captionRows.length - 1
  }
  /** Pixels two steer markers must be apart to both be drawn. A triangle is 9 wide, so this is
   *  clearance rather than mere non-overlap — abutting markers read as one smeared blob. */
  const MARKER_GAP = 18
  const drawnAt: number[] = []

  return (
    <g style={{ fontFamily: 'inherit' }}>
      {/* Drop captions along the top of the plot, stacked where they would otherwise overlap. */}
      {drops.map((d) => {
        if (!inView(d.day)) return null
        const dx = x(d.day)
        if (dx == null) return null
        const label = `${dayLabel} ${Math.round(d.day)} · ${d.names.join(', ')} dropped`
        const width = label.length * 6 + 10
        const anchorLeft = dx + width > offset.left + offset.width
        const left = anchorLeft ? dx - width - 4 : dx + 4
        // Drops far enough apart to have their own captions can still be close enough for those
        // captions to overlap, and two red labels on one another are less legible than either.
        const row = captionRow(left, left + width)
        const top = offset.top - 2 + row * CAPTION_ROW_HEIGHT
        return (
          <g key={`drop-${d.day}`}>
            <rect x={left} y={top} width={width} height={16} rx={3} fill={DROP_COLOR} opacity={0.1} />
            <text
              x={anchorLeft ? dx - 8 : dx + 9}
              y={top + 12}
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
        // Markers are ordered biggest-first per PSP, so the one kept out of a cluster is the one
        // standing for the most volume.
        if (drawnAt.some((at) => Math.abs(at - sx) < MARKER_GAP)) return null
        drawnAt.push(sx)
        // The label sits above its own marker rather than beside it: alongside, it ran straight
        // through the neighbouring triangles of the same burst.
        const nearRight = sx > offset.left + offset.width * 0.72
        const text = `steer +${formatMoney(s.amount, currency)}`
        const width = approxTextWidth(text)
        const anchorX = nearRight ? sx - 6 : sx + 6
        // Preferred height first, then one line higher; a dropped PSP's tail note sits at about
        // the same altitude and got there first.
        const labelY = [sy - 16, sy - 30].find(
          (candidate) => biggestSteer === s && claim(anchorX, candidate, width, nearRight),
        )
        return (
          <g key={`steer-${s.name}-${i}`}>
            <path d={`M${sx},${sy - 11} l4.5,7 h-9 z`} fill={STEER_COLOR} />
            {labelY != null && (
              <text
                x={anchorX}
                y={labelY}
                fontSize={10.5}
                fontWeight={600}
                fill={STEER_COLOR}
                textAnchor={nearRight ? 'end' : 'start'}
              >
                {text}
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
              {m.dropDay != null &&
                mx != null &&
                tailEnd - tailStart > (xHi - xLo) * 0.12 &&
                claim(mx - approxTextWidth(TAIL_NOTE) / 2, ey - 8, approxTextWidth(TAIL_NOTE)) && (
                  <text x={mx} y={ey - 8} fontSize={10} fill={NEUTRAL_INK} textAnchor="middle">
                    {TAIL_NOTE}
                  </text>
                )}
              {m.status === 'missed' &&
                m.endDay <= xHi + 1e-9 &&
                claim(ex, ey - 7, approxTextWidth('× missed'), true) && (
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
        const [line, terms] = labelLines(m, xHi, currency, dayLabel)
        const indent = rightEdge + (m.status === 'met' ? 16 : 8)
        return (
          <g key={`label-${m.name}`}>
            {m.status === 'met' && (
              <text x={rightEdge + 4} y={ly + 4} fontSize={11} fill={MET_COLOR} fontWeight={700}>
                ✓
              </text>
            )}
            <text x={indent} y={ly + 4} fontSize={10.5} fontWeight={600} fill={color}>
              {line}
            </text>
            <text x={indent} y={ly + 17} fontSize={10.5} fill={color} opacity={0.85}>
              {terms}
            </text>
          </g>
        )
      })}
    </g>
  )
}

/** Beneath the lines: the wedge steering added, and the steeper pace a PSP behind its promise now
 *  needs. Both are drawn in the value axis's own units, so they read against the same ticks as the
 *  lines rather than asking for a second scale. */
function PacingUnderlay(props: {
  metas: ConnectorMeta[]
  rows: Record<string, number | undefined>[]
  clipId: string
  xAxisMap?: Record<string, AxisEntry>
  yAxisMap?: Record<string, AxisEntry>
  offset?: Offset
}) {
  const { metas, rows, clipId, xAxisMap, yAxisMap, offset } = props
  const xAxis = xAxisMap ? Object.values(xAxisMap)[0] : undefined
  const yAxis = yAxisMap ? Object.values(yAxisMap)[0] : undefined
  if (!xAxis || !yAxis || !offset) return null
  const x = (v: number) => xAxis.scale(v)
  const y = (v: number) => yAxis.scale(v)

  // The window clips rather than filters: a band truncated at the nearest node would pull away
  // from the left edge while the lines, which overflow the domain, run right up to it.
  return (
    <g>
      <defs>
        <clipPath id={clipId}>
          <rect x={offset.left} y={offset.top} width={offset.width} height={offset.height} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${clipId})`}>
        {metas.filter((m) => m.steeredShare >= VISIBLE_BAND_SHARE).map((m) => {
          const pts: Array<{ x: number; top: number; bottom: number }> = []
          for (const row of rows) {
            const total = row[m.name] ?? row[`${m.name}__after`]
            const unaided = row[`${m.name}__unaided`]
            if (typeof total !== 'number' || typeof unaided !== 'number') continue
            const px = x(Number(row.day ?? 0))
            const top = y(total)
            const bottom = y(unaided)
            if (px == null || top == null || bottom == null) continue
            pts.push({ x: px, top, bottom })
          }
          // A band no wider than a hairline is a PSP the engine never had to help.
          if (pts.length < 2 || pts.reduce((w, q) => Math.max(w, q.bottom - q.top), 0) < 1) return null
          const forward = pts.map((q, i) => `${i ? 'L' : 'M'}${q.x},${q.top}`).join('')
          const back = [...pts].reverse().map((q) => `L${q.x},${q.bottom}`).join('')
          const floor = pts.map((q, i) => `${i ? 'L' : 'M'}${q.x},${q.bottom}`).join('')
          return (
            <g key={`band-${m.name}`}>
              <path d={`${forward}${back}Z`} fill={m.color} opacity={0.16} />
              {/* The band's floor is the line this PSP would have drawn unaided. */}
              <path d={floor} fill="none" stroke={m.color} strokeWidth={1} strokeDasharray="2 3" opacity={0.5} />
            </g>
          )
        })}

        {/* From the tip of each behind PSP's line to its promise at its own cycle close: the climb
            it has left. Steeper than the dashed promise it springs from, by exactly how far behind
            it is. */}
        {metas.filter(needsPace).map((m) => {
          const fromX = x(m.endDay)
          const fromY = y(m.endTotal)
          const toX = x(m.offset + m.cycleDays)
          const toY = y(m.goal)
          if (fromX == null || fromY == null || toX == null || toY == null) return null
          return (
            <g key={`need-${m.name}`}>
              <line x1={fromX} y1={fromY} x2={toX} y2={toY} stroke={m.color} strokeWidth={1.5} strokeDasharray="4 3" opacity={0.9} />
              <circle cx={fromX} cy={fromY} r={2.5} fill={m.color} />
            </g>
          )
        })}
      </g>
    </g>
  )
}

/**
 * The two lines of a PSP's end-of-line label.
 *
 * Shared with the chart body, which sizes the right margin from them. A fixed reserve is wrong in
 * both directions — too tight for a long connector name, and, far more often, a band of empty plot
 * beside labels that no longer fill it.
 */
function labelLines(
  m: ConnectorMeta,
  xHi: number,
  currency: string | null | undefined,
  dayLabel: string,
): [string, string] {
  const prefix = m.status === 'met' ? '' : m.status === 'steering' ? '↗ ' : ''
  const wholeCycle = xHi >= m.offset + m.cycleDays - 1e-9
  // Inside a window the label sits at a fraction of the goal, so it reads "$11k of $70k" rather
  // than naming the goal beside a gridline that is not it. It does not say which day that edge is:
  // the x-axis is directly beneath and already labelled.
  const position = wholeCycle
    ? formatMoney(m.goal, currency)
    : `${formatMoney(promiseAt(m, xHi), currency)} of ${formatMoney(m.goal, currency)}`
  const needs = needsPace(m)
    ? `needs ${formatMoney(m.neededDaily, currency)}/${dayLabel.toLowerCase()} `
    : ''
  return [
    `${prefix}${m.name} · ${position}`,
    `${needs}→ ${formatMoneyExact(m.reward, currency)}`,
  ]
}

/**
 * Where the shared axis starts and how long it runs.
 *
 * The series numbers each connector's buckets from that connector's *own* cycle start, and a
 * document may mix billing cycles. The axis is wall-clock: every connector is offset onto one
 * origin — the earliest cycle start — so a payment is drawn at the moment it happened rather than
 * at its position in whichever cycle it belongs to.
 *
 * Exported because the window picker needs `daysTotal` to know which windows are offerable, and it
 * is rendered by whoever owns the card header rather than by the chart.
 */
export function pacingAxis(connectors: CommitmentConnectorSeries[], daySecs?: number | null) {
  const dayMs = Math.max(1, (daySecs ?? SECS_PER_DAY) * 1000)
  const cycleStartMs = Math.min(
    ...connectors.map((c) => Date.parse(c.cycleStart) || Number.POSITIVE_INFINITY),
  )
  const offsets = new Map(
    connectors.map((c) => [
      c.connector,
      Number.isFinite(cycleStartMs)
        ? Math.max(0, ((Date.parse(c.cycleStart) || cycleStartMs) - cycleStartMs) / dayMs)
        : 0,
    ]),
  )
  const daysTotal = Math.max(
    1,
    ...connectors.map((c) => (offsets.get(c.connector) ?? 0) + c.daysTotal),
  )
  return { cycleStartMs, offsets, daysTotal }
}

/** The windows offerable for a cycle this long, and which one opens by default. */
export function pacingWindows(daysTotal: number) {
  return {
    options: (daysTotal <= 8 ? [1, 2] : [1, 7, 14]).filter((w) => w < daysTotal),
    initial: (daysTotal <= 8 ? 1 : 7) as number | 'all',
  }
}

/**
 * The window selector. Rendered by whoever owns the card header rather than by the chart, so it
 * sits with the title instead of stranded above the plot.
 */
export function PacingWindowPicker({
  daysTotal,
  daySecs,
  value,
  onChange,
}: {
  daysTotal: number
  daySecs?: number | null
  value: number | 'all'
  onChange: (value: number | 'all') => void
}) {
  const { options } = pacingWindows(daysTotal)
  if (options.length === 0) return null
  const isTest = isTestCycle(daySecs)
  const label = (w: number) =>
    w === 1 && !isTest ? 'Last 24 hours' : `Last ${w} ${w === 1 ? 'day' : 'days'}`
  return (
    <div className="inline-flex rounded-md border border-slate-200 p-0.5 dark:border-[#1f1f29]">
      {[...options, 'all' as const].map((opt) => {
        const active = opt === value
        return (
          <button
            key={String(opt)}
            type="button"
            onClick={() => onChange(opt)}
            className={`rounded px-2.5 py-1 text-[11px] font-medium transition-colors ${
              active
                ? 'bg-brand-500 text-white'
                : 'text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200'
            }`}
          >
            {opt === 'all' ? 'All' : label(opt)}
          </button>
        )
      })}
    </div>
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
  formatDay,
}: {
  active?: boolean
  payload?: Array<{ payload?: Record<string, number | undefined> }>
  label?: number
  metas: ConnectorMeta[]
  currency?: string | null
  /** The x-axis's own tick formatter, so the header cannot read differently from the ticks. */
  formatDay: (day: number) => string
}) {
  if (!active || !payload?.length) return null
  const row = payload[0]?.payload
  if (!row) return null
  return (
    <div style={{ ...CHART_TOOLTIP_STYLE, fontSize: 12, lineHeight: 1.5, minWidth: 220 }}>
      <p style={{ ...CHART_TOOLTIP_LABEL_STYLE, margin: '0 0 6px' }}>
        {typeof label === 'number' ? formatDay(label) : label}
      </p>
      {metas.map((m) => {
        const measured = row[m.name] ?? row[`${m.name}__after`]
        const pending = measured == null ? row[`${m.name}__pending`] : undefined
        const total = measured ?? pending
        const promise = row[`${m.name}__promise`]
        const unaided = row[`${m.name}__unaided`]
        // Only a measured point can be split into steered and unaided; the projected tail cannot.
        const steeredIn = measured != null && typeof unaided === 'number' ? measured - unaided : 0
        const note = [
          steeredIn >= 1 ? `+${formatMoney(steeredIn, currency)} steered` : null,
          needsPace(m) ? `needs ${formatMoney(m.neededDaily, currency)}/day` : null,
        ]
          .filter(Boolean)
          .join(' · ')
        return (
          <div key={m.name}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <span style={{ width: 8, height: 8, borderRadius: 999, backgroundColor: m.color, flexShrink: 0 }} />
              <span style={{ fontWeight: 600 }}>{m.name}</span>
              <span style={{ marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>
                {total != null ? `${pending != null ? '~' : ''}${formatMoney(total, currency)}` : '—'}
                <span style={{ opacity: 0.6 }}> / {promise != null ? formatMoney(promise, currency) : '—'}</span>
              </span>
            </div>
            {note && (
              <div style={{ marginLeft: 14, opacity: 0.65, fontSize: 11, fontVariantNumeric: 'tabular-nums' }}>{note}</div>
            )}
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
  fixedWidth,
  perDay,
  window: window_,
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
  /** Render at a fixed pixel width instead of filling the container — for server-side previews. */
  fixedWidth?: number
  /** Buckets per contract day the series was fetched with; defaults to what the pages use. */
  perDay?: number
  /** Controlled window, when the card places `PacingWindowPicker` in its own header. Omit to let
   *  the chart own the state and draw the control above the plot. */
  window?: number | 'all'
}) {
  const clipId = `pacing-clip-${useId().replace(/:/g, '')}`
  const dayMs = Math.max(1, (daySecs ?? SECS_PER_DAY) * 1000)
  const bucketDays = 1 / Math.max(1, perDay ?? bucketsPerDay(daySecs))
  // How far past the last measurement the tentative tail may run. It exists to cover ingestion
  // lag, which is a bucket or two; a longer silence is traffic that stopped, and extrapolating
  // the burst that preceded it would draw hours of volume that never arrived — and, because the
  // value axis has to fit whatever is drawn, would scale the whole chart off that guess.
  const maxPendingDays = bucketDays * 2
  // The clock the live view redraws on; started below, once there is something for it to move.
  const [, setTick] = useState(0)
  const dayLabel = dayUnit(daySecs).short
  const { cycleStartMs, offsets, daysTotal } = useMemo(
    () => pacingAxis(connectors, daySecs),
    [connectors, daySecs],
  )
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
    type Node = { at: number; total: number; unaided: number; steered: number }
    const PACE_BUCKETS = 3
    const nodesFor = (c: CommitmentConnectorSeries): { nodes: Node[]; pace: number } => {
      const off = offsets.get(c.connector) ?? 0
      const pts = c.points.map((p) => ({ ...p, day: p.day + off })).sort((a, b) => a.day - b.day)
      const nodes: Node[] = []
      let running = 0
      // The same line with every steered payment taken back out: where this PSP would have stood
      // on approval-rate routing alone. The gap between the two is what the engine added.
      let unaided = 0
      let prevDay: number | null = null
      for (const p of pts) {
        const day = clampDay(p.day)
        if (prevDay != null && day - prevDay > bucketDays * 1.5) {
          nodes.push({ at: Math.min(day, edge), total: running, unaided, steered: 0 })
        }
        running += p.total
        unaided += Math.max(0, p.total - p.steered)
        nodes.push({ at: Math.min(day + bucketDays, edge), total: running, unaided, steered: p.steered })
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
      const endDay = isPastRun ? daysTotal : nodes.length ? nodes[nodes.length - 1].at : 0
      return {
        name: c.connector,
        color: colorFor(c.connector, i),
        goal: c.goal,
        reward: c.reward,
        status,
        dropDay,
        endDay,
        endTotal: 0,
        pace,
        pendingDay: Math.min(nowDay, endDay + maxPendingDays),
        offset: offsets.get(c.connector) ?? 0,
        cycleDays: Math.max(1, c.daysTotal),
        // These need `endTotal`, which the row pass fills in; computed once it has.
        neededDaily: 0,
        behind: false,
        steeredShare: 0,
        endUnaided: 0,
      }
    })

    // Every instant any PSP's line bends at, plus the ends of the promise lines and the drops.
    const dayset = new Set<number>([0, daysTotal, ...(isPastRun ? [] : [nowDay])])
    for (const { nodes } of nodesByName.values()) for (const n of nodes) dayset.add(n.at)
    for (const m of metas) if (m.dropDay != null) dayset.add(m.dropDay)
    if (!isPastRun) for (const m of metas) dayset.add(m.pendingDay)
    const days = [...dayset].sort((a, b) => a - b)

    const rows: Record<string, number | undefined>[] = []
    const running: Record<string, number> = {}
    const unaidedRunning: Record<string, number> = {}
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
          unaidedRunning[m.name] = nodes[idx].unaided
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
          // The floor of the steered band, drawn wherever the line itself is drawn.
          row[`${m.name}__unaided`] = unaidedRunning[m.name] ?? 0
          m.endTotal = total
          m.endUnaided = unaidedRunning[m.name] ?? 0
        }
        // The stretch not yet measured: from the last bucket to "now", projected at the recent
        // pace and drawn tentatively. Shares its first point with the line so there is no gap.
        if (
          !isPastRun &&
          m.pendingDay > m.endDay + 1e-9 &&
          (Math.abs(day - m.pendingDay) < 1e-9 || Math.abs(day - m.endDay) < 1e-9)
        ) {
          row[`${m.name}__pending`] = m.endTotal + m.pace * (day - m.endDay)
        }
        const throughCycle = (clampDay(day) - m.offset) / m.cycleDays
        row[`${m.name}__promise`] = m.goal * Math.min(1, Math.max(0, throughCycle))
      }
      rows.push(row)
    }

    // What each PSP owes per contract day from here — the promise's remainder over the days its
    // own cycle has left, which is steeper than the original even pace exactly when it is behind.
    for (const m of metas) {
      m.steeredShare =
        m.endTotal > 0 ? Math.max(0, m.endTotal - m.endUnaided) / m.endTotal : 0
      if (isPastRun) continue
      const daysLeft = Math.max(0, m.offset + m.cycleDays - nowDay)
      m.neededDaily = daysLeft > 0 ? Math.max(0, m.goal - m.endTotal) / daysLeft : 0
      const owedByNow = m.goal * Math.min(1, Math.max(0, (nowDay - m.offset) / m.cycleDays))
      m.behind = m.endTotal < owedByNow - 1e-9
    }

    // Steer triangles: one per bucket is noise on a long cycle — keep the largest few per PSP.
    // Steering arrives in bursts, though, so the largest few are often adjacent buckets that draw
    // as a pile of overlapping markers on one spot. Merge a run of them into a single marker
    // carrying the whole burst before taking the top few, so each triangle stands for something a
    // reader can distinguish.
    const MAX_STEERS_PER_PSP = 6
    const keptSteers = metas.flatMap((m) => {
      const mine = steers.filter((s) => s.name === m.name).sort((a, b) => a.day - b.day)
      const merged: Steer[] = []
      for (const s of mine) {
        const last = merged[merged.length - 1]
        if (last && s.day - last.day <= bucketDays * 1.5) {
          last.amount += s.amount
          last.day = s.day
          last.endDay = s.endDay
          last.total = s.total
          continue
        }
        merged.push({ ...s })
      }
      return merged.sort((a, b) => b.amount - a.amount).slice(0, MAX_STEERS_PER_PSP)
    })

    // Two commitments dropped by the same forecast are usually milliseconds apart, not identical,
    // so grouping on an exact day left them as separate captions a pixel apart. Merge anything
    // inside a bucket — the resolution the chart draws at — into one caption naming both.
    const dropped = metas
      .filter((m) => m.dropDay != null)
      .sort((a, b) => (a.dropDay ?? 0) - (b.dropDay ?? 0))
    const drops: Drop[] = []
    for (const m of dropped) {
      const day = m.dropDay ?? 0
      const last = drops[drops.length - 1]
      if (last && day - last.day <= bucketDays) {
        last.names.push(m.name)
        continue
      }
      drops.push({ day, names: [m.name], reason: eliminationReasons?.get(m.name) })
    }

    // Round the axis up to a clean step so the ticks read $2.5M / $5M rather than $8.7M.
    const projected = (m: ConnectorMeta) =>
      isPastRun ? m.endTotal : m.endTotal + m.pace * Math.max(0, m.pendingDay - m.endDay)
    const raw = Math.max(1, ...metas.map((m) => Math.max(m.goal, m.endTotal, projected(m)))) * 1.05
    const step = niceStep(raw)
    const yMax = Math.ceil(raw / step) * step
    const yTicks: number[] = []
    for (let v = 0; v <= yMax + 1e-9; v += step) yTicks.push(v)
    return { rows, metas, steers: keptSteers, drops, yMax, yTicks }
  }, [connectors, colorFor, statusFor, eliminatedAtMs, eliminationReasons, isPastRun, daysTotal, nowDay, cycleStartMs, dayMs, bucketDays, maxPendingDays, offsets])

  // ── The window: a slice of the cycle that follows the run, or the whole cycle ("All"). ──────
  // Opens on the first contract day so an early run is legible instead of a sliver in the
  // bottom-left of a whole-cycle axis, then slides forward with the run; "All" unzooms.
  const windowOptions = useMemo(() => pacingWindows(daysTotal).options, [daysTotal])
  // A card that renders `PacingWindowPicker` in its own header owns the value and passes it in;
  // one that does not gets the control above the plot and the state here.
  const [ownWindow, setOwnWindow] = useState<number | 'all'>(() => pacingWindows(daysTotal).initial)
  const windowSel = window_ ?? ownWindow
  const win = windowSel === 'all' || !windowOptions.includes(windowSel) ? null : windowSel
  const runEdge = isPastRun ? daysTotal : nowDay
  // The window's right edge follows the run, not the window's own width. Pinning it to the width
  // reserved the whole span up front, so a cycle younger than the window spent the difference as
  // blank plot — at "Last 7 days" on a five-day-old run, a fifth of the chart drew nothing and
  // squeezed everything that did into what was left. A few buckets keep a brand-new run from
  // collapsing to a zero-width axis.
  const xHi = win == null ? daysTotal : Math.min(daysTotal, Math.max(runEdge, bucketDays * 4))
  const xLo = win == null ? 0 : Math.max(0, xHi - win)

  // The live view redraws on its own clock, not on the poll, so the tentative tail and the window
  // edge keep pace with "now" between fetches. Only two things actually move, so the clock runs
  // only while one of them can: a window whose right edge has caught up to now and is now tracking
  // it, and a tail still growing toward its cap. In "All", or before the run reaches the window's
  // width, or once every tail is capped, nothing on screen depends on the current instant.
  //
  // Note what is deliberately *not* a condition: whether traffic is arriving. The contract's cycle
  // is wall-clock — it burns down whether or not payments are being sent — so an idle run still
  // advances toward its close, and the axis is right to show it.
  const trackingNow = win != null && nowDay > win
  const tailGrowing = metas.some((m) => m.pendingDay < m.endDay + maxPendingDays - 1e-9)
  const animating = !isPastRun && (trackingNow || tailGrowing)
  // Half a second is a visible step on a test cycle, where a contract day is a minute, and
  // invisible on a calendar one — where it would be sixty redraws a minute to move the axis by
  // a thousandth of a pixel.
  const tickMs = Math.min(5_000, Math.max(500, Math.round(dayMs / 120)))
  useEffect(() => {
    if (!animating) return undefined
    const id = window.setInterval(() => setTick((t) => t + 1), tickMs)
    return () => window.clearInterval(id)
  }, [animating, tickMs])

  // Inside a window a contract day wide or narrower, "Day 30.25" names nothing a reader can act
  // on; the wall-clock instant it stands for does, and the axis has the cycle's origin to work it
  // out. A test cycle keeps contract days, where a "day" is a minute and a clock would mislead.
  const isTest = isTestCycle(daySecs)
  const asClock = win != null && win <= 1 && !isTest && Number.isFinite(cycleStartMs)

  const ticks = useMemo(() => {
    const span = xHi - xLo
    if (asClock) {
      // Snap to whole hours on the reader's clock rather than to fractions of the cycle's own
      // origin, so the labels read 06:00 and not 05:30 wherever the cycle happens to open.
      const HOUR_STEPS = [1, 2, 3, 6, 12]
      const stepHours = HOUR_STEPS.find((h) => (span * 24) / h <= 6) ?? 12
      const stepMs = stepHours * 3_600_000
      const at = (day: number) => cycleStartMs + day * dayMs
      // `getTimezoneOffset` is minutes *behind* UTC, so subtracting it puts an instant on the wall
      // clock, which is where the snapping has to happen.
      const shift = new Date(at(xHi)).getTimezoneOffset() * 60_000
      const out: number[] = []
      let ms = Math.ceil((at(xLo) - shift) / stepMs) * stepMs + shift
      for (; ms < at(xHi) - 1; ms += stepMs) out.push((ms - cycleStartMs) / dayMs)
      out.push(xHi)
      return out
    }
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
  }, [daysTotal, win, xLo, xHi, asClock, cycleStartMs, dayMs])

  // The value axis follows the window too: zoomed to what is on screen, with clean steps.
  const { yLo, yHi, yTicksInView } = useMemo(() => {
    if (win == null) return { yLo: 0, yHi: yMax, yTicksInView: yTicks }
    const values: number[] = []
    for (const m of metas) {
      values.push(promiseAt(m, xLo), promiseAt(m, xHi))
    }
    for (const row of rows) {
      const day = Number(row.day ?? 0)
      if (day < xLo - 1e-9 || day > xHi + 1e-9) continue
      for (const m of metas) {
        for (const key of [m.name, `${m.name}__after`, `${m.name}__pending`, `${m.name}__unaided`]) {
          const v = row[key]
          if (typeof v === 'number') values.push(v)
        }
      }
    }
    const lo = Math.max(0, Math.min(...values, Number.POSITIVE_INFINITY))
    const hi = Math.max(...values, 1)
    const span = Math.max(hi - lo, hi * 0.1, 1)
    const step = niceStep(span)
    const floor = Math.max(0, Math.floor((lo - span * 0.05) / step) * step)
    const ceil = Math.ceil((hi + span * 0.08) / step) * step
    const t: number[] = []
    for (let v = floor; v <= ceil + 1e-9; v += step) t.push(v)
    return { yLo: floor, yHi: ceil, yTicksInView: t }
  }, [win, rows, metas, xLo, xHi, daysTotal, yMax, yTicks])

  // A cycle that keeps advancing while nothing is delivered is the confusing case: the axis moves,
  // the lines do not, and without a word for it the chart looks broken rather than idle. Measured
  // from the newest bucket any PSP has reported, so ingestion lag on one connector does not read
  // as a stall.
  const idleFor = nowDay - Math.max(0, ...metas.map((m) => m.endDay))
  const stalled = !isPastRun && idleFor > 1

  // The right rail is reserved for the end-of-line labels, so it is sized from them rather than
  // fixed. The old 196 was set when a label ran to three lines with the day spelled out; the
  // shorter ones since left a band of empty plot beside them.
  const rightMargin = useMemo(() => {
    const widest = Math.max(
      0,
      ...metas.flatMap((m) =>
        labelLines(m, xHi, currency, dayLabel).map((line) => approxTextWidth(line)),
      ),
    )
    return Math.round(Math.min(240, Math.max(96, widest + 26)))
  }, [metas, xHi, currency, dayLabel])

  const formatDay = useCallback(
    (day: number) =>
      asClock
        ? new Date(cycleStartMs + day * dayMs).toLocaleTimeString([], {
            hour: '2-digit',
            minute: '2-digit',
          })
        : `${dayLabel} ${Number.isInteger(day) ? day : day.toFixed(2).replace(/0+$/, '')}`,
    [asClock, cycleStartMs, dayMs, dayLabel],
  )

  return (
    <div className="w-full">
      {window_ === undefined && (
        <div className="mb-2 flex justify-end">
          <PacingWindowPicker
            daysTotal={daysTotal}
            daySecs={daySecs}
            value={windowSel}
            onChange={setOwnWindow}
          />
        </div>
      )}
      <div className="w-full" style={{ height }}>
        <Sized width={fixedWidth} height={height}>
          <ComposedChart data={rows} margin={{ top: 20, right: rightMargin, bottom: 4, left: 8 }}>
            <CartesianGrid vertical={false} stroke="currentColor" opacity={0.08} />
            <XAxis
              type="number"
              dataKey="day"
              domain={[xLo, xHi]}
              allowDataOverflow
              ticks={ticks}
              tickFormatter={formatDay}
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
              content={<PacingTooltip metas={metas} currency={currency} formatDay={formatDay} />}
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
            <Customized
              component={<PacingUnderlay metas={metas} rows={rows} clipId={clipId} />}
            />
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
                <PacingOverlay metas={metas} steers={steers} drops={drops} currency={currency} dayLabel={dayLabel} xLo={xLo} xHi={xHi} />
              }
            />
          </ComposedChart>
        </Sized>
      </div>
      {stalled && (
        <p className="mt-2 text-[11px] text-amber-600 dark:text-amber-500">
          No volume measured for {idleFor.toFixed(1)} {dayLabel.toLowerCase()}
          {idleFor >= 2 ? 's' : ''}. The cycle runs on wall-clock time, so it keeps counting down
          whether or not payments are arriving — which is why the axis advances while the lines
          hold flat.
        </p>
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
              {formatAchieved(achieved, c.goal, currency)} delivered · {pctOfGoal(achieved, c.goal)}%
            </p>
          </div>
        )
      })}
    </div>
  )
}
