import { useEffect, useId, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  ArrowRightLeft,
  BellRing,
  ChevronDown,
  ChevronRight,
  Clock,
  ExternalLink,
  Repeat,
  SlidersHorizontal,
  TrendingDown,
  TrendingUp,
} from 'lucide-react'
import {
  POLL_INTERVAL_MS,
  ROUTING_EVENTS_DEFAULT_LIMIT,
  useRoutingEvents,
} from '../../hooks/useRoutingEvents'
import { humanizeAuditValue } from '../../lib/auditLabels'
import { connectorColors } from '../../lib/connectorColors'
import {
  FLAP_RUN_MIN_FLIPS,
  FeedItem,
  LeaderFlipDetail,
  auditLinkForEvent,
  EventLane,
  buildLanes,
  describeCalibrationChange,
  dimensionLabel,
  flipCause,
  flipCauseText,
  flipKind,
  formatDuration,
  formatScore,
  groupFeed,
  leaderFlipDetails,
  marginPp,
  rangeDurationMs,
  retunedBeforeIds,
  summariesFromServer,
  summarizeDimensions,
} from '../../lib/routingEventInsights'
import { AnalyticsRangeValue, RoutingEvent, RoutingEventType } from '../../types/api'
import { Card, SurfaceLabel } from '../ui/Card'
import { PageHeading } from '../ui/PageHeading'
import { Spinner } from '../ui/Spinner'

const PRESET_OPTIONS: { value: AnalyticsRangeValue; label: string; badge: string }[] = [
  { value: '1h', label: 'Last 1 hour', badge: '1 hour' },
  { value: '12h', label: 'Last 12 hours', badge: '12 hours' },
  { value: '1d', label: 'Last 1 day', badge: '1 day' },
  { value: '1w', label: 'Last 1 week', badge: '1 week' },
]

/**
 * Only the event types the feed can actually carry. Auth-band crossings are switched off
 * server-side (`resolve_auth_band` always returns Off), so they are not offered as filters.
 */
const EVENT_TYPE_META: Partial<
  Record<RoutingEventType, { label: string; badge: 'blue' | 'purple'; icon: React.ElementType }>
> = {
  leader_changed: { label: 'Leader change', badge: 'blue', icon: ArrowRightLeft },
  calibration_applied: { label: 'Autopilot tuning', badge: 'purple', icon: SlidersHorizontal },
}

const ALL_EVENT_TYPES = Object.keys(EVENT_TYPE_META) as RoutingEventType[]

// How long a change that arrives while the page is open stays highlighted.
const NEW_ROW_HIGHLIGHT_MS = 4_000

// The dashboard's own text tones (see OverviewPage): captions and muted copy.
const muted = 'text-slate-500 dark:text-[#8d96aa]'
const body = 'text-slate-700 dark:text-[#c6cfdd]'
const strong = 'text-slate-950 dark:text-white'
const focusRing =
  'rounded-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-600'

const gatewayName = (gateway: string) => humanizeAuditValue(gateway) || gateway

function isToday(ms: number) {
  return new Date(ms).toDateString() === new Date().toDateString()
}

function formatEventDateTime(ms: number) {
  return new Date(ms).toLocaleString([], { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function formatTime(ms: number) {
  return new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

/** A time today, or a date and time otherwise. */
function formatMoment(ms: number) {
  return isToday(ms) ? formatTime(ms) : formatEventDateTime(ms)
}

function capitalize(text: string) {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

function pluralize(count: number, one: string, many = `${one}s`) {
  return `${count} ${count === 1 ? one : many}`
}

function MarginTag({ event }: { event: RoutingEvent }) {
  const margin = marginPp(event)
  if (flipKind(event) === 'stale' || margin == null) return null
  return (
    <span
      className="rounded px-1.5 py-0.5 text-[11px] font-medium tabular-nums leading-4 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
      title="How far the new leader's success rate was ahead, in percentage points"
    >
      +{margin.toFixed(1)} pp
    </span>
  )
}

/** "92.9%", or "100.0→70.0%" when the score moved into the flip. */
function scoreText(move: { fromScore: number; toScore: number } | null, score: number | null | undefined) {
  if (move) return `${(move.fromScore * 100).toFixed(1)}→${formatScore(move.toScore)}`
  return formatScore(score)
}

/** A PSP with its colour, and its score at the flip next to it. */
function GatewayScore({
  gateway,
  score,
  moved,
  emphasis,
  colorFor,
}: {
  gateway: string
  score: string | null
  moved: 'up' | 'down' | null
  emphasis: boolean
  colorFor: (gateway: string) => string
}) {
  const MoveIcon = moved === 'down' ? TrendingDown : moved === 'up' ? TrendingUp : null
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className="h-2 w-2 shrink-0 rounded-full" style={{ backgroundColor: colorFor(gateway) }} aria-hidden />
      <span className={emphasis ? `font-semibold ${strong}` : muted}>{gatewayName(gateway)}</span>
      {score && (
        <span className={`inline-flex items-center gap-0.5 text-[12px] tabular-nums ${emphasis ? body : muted}`}>
          {score}
          {MoveIcon && (
            <MoveIcon
              size={12}
              aria-hidden
              className={moved === 'down' ? 'text-rose-600 dark:text-rose-400' : 'text-emerald-600 dark:text-emerald-400'}
            />
          )}
        </span>
      )}
    </span>
  )
}

/** The link to the payments behind one leader change. */
function PaymentsLink({ event, detail }: { event: RoutingEvent; detail: LeaderFlipDetail | undefined }) {
  const pspNames = [event.previous_gateway, event.gateway].filter(Boolean).map((g) => gatewayName(g as string))
  const about = `${pspNames.join(' and ')} payments in ${dimensionLabel(event)} that led up to this change`
  return (
    <Link
      to={auditLinkForEvent(event, detail?.priorFlipMs ?? null)}
      aria-label={`View payments: ${about}`}
      title={about}
      className={`inline-flex items-center gap-0.5 whitespace-nowrap font-medium text-brand-600 hover:underline dark:text-sky-300 ${focusRing}`}
    >
      Payments <ExternalLink size={11} aria-hidden />
    </Link>
  )
}

/** Who handed over to whom, with their scores; the full sentence is the tooltip and what screen readers hear. */
function FlipNames({
  event,
  colorFor,
  compact = false,
}: {
  event: RoutingEvent
  colorFor: (gateway: string) => string
  /** One line: each PSP's rate at the change only, its move shown by the trend arrow. */
  compact?: boolean
}) {
  const stale = flipKind(event) === 'stale'
  const { fell, rose } = flipCause(event)
  const cause = flipCauseText(event, gatewayName)
  const rate = (move: typeof fell, score: number | null | undefined) =>
    compact ? formatScore(move ? move.toScore : score) : scoreText(move, score)
  return (
    <span
      className={`inline-flex items-center gap-x-2 gap-y-1 ${compact ? 'min-w-0 flex-nowrap whitespace-nowrap' : 'flex-wrap'}`}
      title={cause}
    >
      <span className="sr-only">{cause}</span>
      {event.previous_gateway && (
        <span aria-hidden className="inline-flex items-center gap-1.5">
          <GatewayScore
            gateway={event.previous_gateway}
            score={stale ? null : rate(fell, event.previous_score)}
            moved={fell ? 'down' : null}
            emphasis={false}
            colorFor={colorFor}
          />
          {stale && (
            <span className="rounded bg-slate-900/[0.05] px-1.5 py-0.5 text-[11px] font-medium leading-4 text-slate-600 dark:bg-white/[0.06] dark:text-slate-300">
              stale
            </span>
          )}
        </span>
      )}
      <span className="text-slate-400" aria-hidden>→</span>
      <span aria-hidden>
        <GatewayScore
          gateway={event.gateway}
          score={rate(rose, event.score)}
          moved={rose ? 'up' : null}
          emphasis
          colorFor={colorFor}
        />
      </span>
    </span>
  )
}

/** How long the previous leader held before this change. */
function HeldFor({ event, detail }: { event: RoutingEvent; detail: LeaderFlipDetail | undefined }) {
  if (!event.previous_gateway || detail?.previousHeldMs == null) return null
  return (
    <span
      className={`inline-flex items-center gap-1 whitespace-nowrap text-[12px] tabular-nums ${muted}`}
      title={`${gatewayName(event.previous_gateway)} led for ${formatDuration(detail.previousHeldMs)} before this`}
    >
      <Clock size={11} aria-hidden />
      <span className="sr-only">{gatewayName(event.previous_gateway)}</span>
      held {formatDuration(detail.previousHeldMs)}
    </span>
  )
}

function rowHighlight(isNew: boolean) {
  return `motion-safe:transition-colors motion-safe:duration-1000 ${isNew ? 'bg-sky-50 dark:bg-sky-400/[0.08]' : ''}`
}

type NodeKind = 'leader' | 'autopilot' | 'flapping'

const NODE_STYLE: Record<NodeKind, { icon: React.ElementType; label: string; className: string; small?: boolean }> = {
  leader: {
    icon: ArrowRightLeft,
    label: 'Leader change',
    className: 'border-blue-200 bg-blue-50 text-blue-600 dark:border-sky-400/30 dark:bg-sky-400/10 dark:text-sky-300',
  },
  flapping: {
    icon: Repeat,
    label: 'Oscillating',
    className: 'border-orange-200 bg-orange-50 text-orange-700 dark:border-orange-400/30 dark:bg-orange-400/10 dark:text-orange-300',
  },
  autopilot: {
    icon: SlidersHorizontal,
    label: 'Autopilot tuning',
    className: 'border-violet-200 bg-violet-50 text-violet-600 dark:border-violet-400/30 dark:bg-violet-400/10 dark:text-violet-300',
    small: true,
  },
}

/**
 * What kind of change a row is, in words and colour, at the start of the line: the same names as
 * the filter pills above the list.
 */
function KindTag({ kind, children }: { kind: NodeKind; children?: React.ReactNode }) {
  const tone =
    kind === 'leader'
      ? 'bg-sky-500/12 text-sky-700 ring-sky-500/22 dark:bg-sky-400/14 dark:text-sky-200 dark:ring-sky-400/28'
      : kind === 'flapping'
        ? 'bg-orange-500/10 text-orange-700 ring-orange-500/20 dark:text-orange-300'
        : 'bg-purple-500/10 text-purple-700 ring-purple-500/20 dark:text-purple-300'
  return (
    <span className={`inline-flex shrink-0 whitespace-nowrap rounded-md px-2 py-0.5 text-[11px] font-medium ring-1 ring-inset ${tone}`}>
      {children ?? NODE_STYLE[kind].label}
    </span>
  )
}

/** The toggle that opens a group, with its title. */
function GroupToggle({
  open,
  onToggle,
  controls,
  children,
}: {
  open: boolean
  onToggle: () => void
  controls: string
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={open}
      aria-controls={controls}
      className={`flex flex-wrap items-center gap-x-2 gap-y-1 text-left ${focusRing}`}
    >
      {open ? <ChevronDown size={14} className={muted} aria-hidden /> : <ChevronRight size={14} className={muted} aria-hidden />}
      {children}
    </button>
  )
}

/** "300 → 250 → 125 → 300", shortened in the middle when the autopilot moved many times. */
function formatPath(path: number[], format: (value: number) => string) {
  const shown = path.length > 8 ? [...path.slice(0, 3), NaN, ...path.slice(-3)] : path
  return shown.map((value) => (Number.isNaN(value) ? '…' : format(value))).join(' → ')
}

const percent = (value: number) => `${value.toFixed(1)}%`

/**
 * What one retune changed, as values: "250 → 300" for the bucket size, "5.0% → 8.5%" for hedging
 * (a % sign tells them apart, so no labels). A retune without previous values shows its settings
 * with a small "first" or "restart" tag. Inside a group whose hedging never moved, `showHedging`
 * is off: the group already says what it held at.
 */
function RetuneStep({
  event,
  retunedBefore,
  showHedging = true,
}: {
  event: RoutingEvent
  retunedBefore: boolean
  showHedging?: boolean
}) {
  const step = (previous: number | null | undefined, next: number | null | undefined, format: (v: number) => string) => {
    if (next == null) return null
    if (previous == null) return format(next)
    return previous === next ? null : `${format(previous)} → ${format(next)}`
  }
  const parts = [
    step(event.previous_bucket_size, event.bucket_size, String),
    showHedging ? step(event.previous_hedging_percent, event.hedging_percent, percent) : null,
  ].filter(Boolean)
  const fresh = event.previous_bucket_size == null && event.previous_hedging_percent == null
  return (
    <span className={`inline-flex items-center gap-2 text-[13px] tabular-nums ${body}`} title={capitalize(describeCalibrationChange(event, retunedBefore))}>
      <span className="sr-only">{describeCalibrationChange(event, retunedBefore)}</span>
      <span aria-hidden>{parts.length ? parts.join(' · ') : 'no change'}</span>
      {fresh && (
        <span
          aria-hidden
          className={`rounded px-1.5 py-0.5 text-[11px] font-medium leading-4 ${
            retunedBefore
              ? 'bg-orange-500/10 text-orange-700 dark:text-orange-300'
              : 'bg-slate-900/[0.05] text-slate-600 dark:bg-white/[0.06] dark:text-slate-300'
          }`}
        >
          {retunedBefore ? 'restart' : 'first'}
        </span>
      )}
    </span>
  )
}

/** The bucket-size path as a tiny line, so the swings read at a glance. */
function PathSparkline({ path }: { path: number[] }) {
  if (path.length < 3) return null
  const width = 64
  const height = 16
  const min = Math.min(...path)
  const span = Math.max(...path) - min || 1
  const points = path
    .map((value, i) => `${((i / (path.length - 1)) * width).toFixed(1)},${(height - 1 - ((value - min) / span) * (height - 2)).toFixed(1)}`)
    .join(' ')
  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="shrink-0 text-violet-500" aria-hidden>
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  )
}

/** Who led when, left to right across the window; empty where no PSP had recent payments. */

/** The head of a lane item: what kind of change it is, and when. */
function ItemHead({ kind, label, timeMs, sinceMs }: { kind: NodeKind; label?: React.ReactNode; timeMs: number; sinceMs?: number }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <KindTag kind={kind}>{label}</KindTag>
      <span className={`whitespace-nowrap text-[12px] tabular-nums ${muted}`}>
        {sinceMs != null ? `${formatMoment(sinceMs)} – ${formatTime(timeMs)}` : formatMoment(timeMs)}
      </span>
    </div>
  )
}

function LeaderChangeItem({
  event,
  detail,
  colorFor,
}: {
  event: RoutingEvent
  detail: LeaderFlipDetail | undefined
  colorFor: (gateway: string) => string
}) {
  return (
    <>
      <ItemHead kind="leader" timeMs={event.bucket_ms} />
      <div className="mt-2 flex items-center gap-x-2 whitespace-nowrap text-[13px]">
        <FlipNames event={event} colorFor={colorFor} compact />
        <MarginTag event={event} />
      </div>
      <div className="mt-1.5 flex items-center justify-between gap-3">
        <HeldFor event={event} detail={detail} />
        <span className="text-[12px]">
          <PaymentsLink event={event} detail={detail} />
        </span>
      </div>
    </>
  )
}

function OscillationItem({
  item,
  details,
  colorFor,
}: {
  item: Extract<FeedItem, { kind: 'flap_run' }>
  details: Map<string, LeaderFlipDetail>
  colorFor: (gateway: string) => string
}) {
  const [open, setOpen] = useState(false)
  const listId = useId()
  // Two back-to-back changes are a switch and back; Oscillating is kept for longer runs.
  const oscillating = item.events.length >= FLAP_RUN_MIN_FLIPS
  return (
    <>
      <ItemHead
        kind={oscillating ? 'flapping' : 'leader'}
        label={`${oscillating ? 'Oscillating' : 'Leader change'} · ${item.events.length} flips`}
        timeMs={item.endMs}
        sinceMs={item.startMs}
      />
      <div className="mt-2 text-sm">
        <GroupToggle open={open} onToggle={() => setOpen((value) => !value)} controls={listId}>
          <span className="sr-only">Show the {item.events.length} flips between</span>
          {item.gateways.map((gateway, index) => (
            <span key={gateway} className={`inline-flex items-center gap-1.5 ${body}`}>
              {index > 0 && <span className="text-slate-400" aria-label="and">⇄</span>}
              <span className="h-2 w-2 rounded-full" style={{ backgroundColor: colorFor(gateway) }} aria-hidden />
              {gatewayName(gateway)}
            </span>
          ))}
        </GroupToggle>
      </div>
      {open && (
        <ul id={listId} className="mt-2 space-y-2 border-l-2 border-slate-100 pl-3 dark:border-[#1e2535]">
          {item.events.map((event) => (
            <li key={event.id} className="text-[13px]">
              <div className="flex items-center justify-between gap-2">
                <span className={`text-[12px] tabular-nums ${muted}`}>{formatTime(event.bucket_ms)}</span>
                <span className="text-[12px]">
                  <PaymentsLink event={event} detail={details.get(event.id)} />
                </span>
              </div>
              <div className="mt-0.5 flex items-center gap-x-2 whitespace-nowrap">
                <FlipNames event={event} colorFor={colorFor} compact />
                <MarginTag event={event} />
              </div>
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

function RetuneItem({ event, retunedBefore }: { event: RoutingEvent; retunedBefore: boolean }) {
  return (
    <>
      <ItemHead kind="autopilot" timeMs={event.bucket_ms} />
      <div className="mt-2">
        <RetuneStep event={event} retunedBefore={retunedBefore} />
      </div>
    </>
  )
}

function RetuneRunItem({
  item,
  retunedBefore,
}: {
  item: Extract<FeedItem, { kind: 'retune_run' }>
  retunedBefore: Set<string>
}) {
  const [open, setOpen] = useState(false)
  const listId = useId()
  const bucketMoved = item.bucketPath.length > 1
  const hedgeMoved = item.hedgingPath.length > 1
  const paths = [
    bucketMoved ? `bucket ${formatPath(item.bucketPath, String)}` : null,
    hedgeMoved ? `hedging ${formatPath(item.hedgingPath, percent)}` : null,
  ].filter(Boolean)
  const held = [
    !bucketMoved && item.bucketPath.length ? `bucket ${item.bucketPath[0]}` : null,
    !hedgeMoved && item.hedgingPath.length ? `hedging ${percent(item.hedgingPath[0])}` : null,
  ].filter(Boolean)
  return (
    <>
      <ItemHead kind="autopilot" label={`Autopilot · ${item.events.length} retunes`} timeMs={item.endMs} sinceMs={item.startMs} />
      <div className="mt-2 text-[13px]">
        <GroupToggle open={open} onToggle={() => setOpen((value) => !value)} controls={listId}>
          <span className="sr-only">Show the {item.events.length} retunes:</span>
          {bucketMoved && <PathSparkline path={item.bucketPath} />}
          {paths.length > 0 && <span className={`tabular-nums ${body}`}>{paths.join('; ')}</span>}
        </GroupToggle>
        {held.length > 0 && (
          <p className={`mt-1 text-[12px] ${muted}`} title="Did not change in these retunes">
            {held.join(', ')} held
          </p>
        )}
      </div>
      {open && (
        <ul id={listId} className="mt-2 flex flex-wrap gap-1.5">
          {item.events.map((event) => (
            <li
              key={event.id}
              className="inline-flex items-center gap-2 rounded-lg border border-violet-100 bg-violet-50/70 px-2 py-0.5 dark:border-violet-400/20 dark:bg-violet-400/[0.06]"
            >
              <span className={`text-[11px] tabular-nums ${muted}`}>{formatTime(event.bucket_ms)}</span>
              <RetuneStep event={event} retunedBefore={retunedBefore.has(event.id)} showHedging={hedgeMoved} />
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

/**
 * One dimension set (e.g. Credit card · Visa · USD · 3DS) as a column: its name, who leads it
 * now, and its own changes, newest first, with back-to-back changes grouped.
 */
function LaneColumn({
  lane,
  items,
  flipDetails,
  retunedBefore,
  colorFor,
  newlyArrived,
}: {
  lane: EventLane
  items: FeedItem[]
  flipDetails: Map<string, LeaderFlipDetail>
  retunedBefore: Set<string>
  colorFor: (gateway: string) => string
  newlyArrived: Set<string>
}) {
  const flips = lane.events.filter((event) => event.event_type === 'leader_changed').length
  const retunes = lane.events.length - flips
  const name = [lane.label, ...lane.dimensions].join(' · ')
  return (
    <section
      aria-label={name}
      className="flex min-w-0 flex-col overflow-hidden rounded-2xl border border-slate-200 bg-slate-50/60 dark:border-[#2a303a] dark:bg-[#121720]"
    >
      <header className="border-b border-slate-200 bg-white px-4 py-3 dark:border-[#2a303a] dark:bg-[#161b24]">
        <h3 className={`flex flex-wrap items-baseline gap-x-1.5 text-sm font-semibold ${strong}`}>
          {[lane.label, ...lane.dimensions].map((part, index) => (
            <span key={`${index}-${part}`} className="inline-flex items-baseline gap-x-1.5">
              {index > 0 && (
                <span className="font-normal text-slate-400" aria-hidden>
                  ·
                </span>
              )}
              {part}
            </span>
          ))}
        </h3>
        <p className={`mt-1 flex flex-wrap items-center gap-x-3 text-[12px] ${muted}`}>
          {lane.leader && (
            <span className="inline-flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full" style={{ backgroundColor: colorFor(lane.leader) }} aria-hidden />
              <span className={`font-semibold ${strong}`}>{gatewayName(lane.leader)}</span> leads
            </span>
          )}
          {flips > 0 && <span>{pluralize(flips, 'leader change')}</span>}
          {retunes > 0 && <span>{pluralize(retunes, 'retune')}</span>}
        </p>
      </header>
      <ul className="max-h-[26rem] divide-y divide-slate-200/70 overflow-y-auto dark:divide-[#1e2535]">
        {items.map((item) => {
          const id = item.kind === 'event' ? item.event.id : item.id
          return (
            <li key={id} className={`px-4 py-3 ${rowHighlight(newlyArrived.has(id))}`}>
              {item.kind === 'flap_run' ? (
                <OscillationItem item={item} details={flipDetails} colorFor={colorFor} />
              ) : item.kind === 'retune_run' ? (
                <RetuneRunItem item={item} retunedBefore={retunedBefore} />
              ) : item.event.event_type === 'leader_changed' ? (
                <LeaderChangeItem event={item.event} detail={flipDetails.get(item.event.id)} colorFor={colorFor} />
              ) : (
                <RetuneItem event={item.event} retunedBefore={retunedBefore.has(item.event.id)} />
              )}
            </li>
          )
        })}
      </ul>
    </section>
  )
}

/** A headline number with its caption, in the dashboard's stat-tile style. */
function StatTile({
  label,
  value,
  caption,
  tone,
}: {
  label: string
  value: React.ReactNode
  caption: React.ReactNode
  tone?: 'warn'
}) {
  return (
    <Card className={`flex flex-col p-5 ${tone === 'warn' ? 'border-orange-300/60 dark:border-orange-500/30' : ''}`}>
      <SurfaceLabel>{label}</SurfaceLabel>
      <p
        className={`mt-3 truncate text-[2rem] font-semibold leading-none tracking-tight ${
          tone === 'warn' ? 'text-orange-600 dark:text-orange-400' : 'text-slate-950 dark:text-white'
        }`}
      >
        {value}
      </p>
      <p className={`mt-2 text-xs ${muted}`}>{caption}</p>
    </Card>
  )
}

/**
 * IDs that arrived while the same view stayed open, so a live change can be pointed out.
 * Changing `viewKey` (the time range) starts over: rows a new range brings in are not new.
 * Nothing is tracked until `ready` (the view's first load finished), so the rows of that first
 * load are the baseline rather than "new".
 */
function useNewlyArrived(ids: string[], viewKey: string, ready: boolean) {
  const known = useRef<{ viewKey: string; ids: Set<string> } | null>(null)
  const [fresh, setFresh] = useState<Set<string>>(new Set())
  useEffect(() => {
    if (!ready) return
    if (known.current === null || known.current.viewKey !== viewKey) {
      known.current = { viewKey, ids: new Set(ids) }
      setFresh(new Set())
      return
    }
    const seen = known.current.ids
    const arrived = ids.filter((id) => !seen.has(id))
    for (const id of ids) seen.add(id)
    if (!arrived.length) return
    setFresh(new Set(arrived))
    const timer = window.setTimeout(() => setFresh(new Set()), NEW_ROW_HIGHLIGHT_MS)
    return () => window.clearTimeout(timer)
  }, [ids, viewKey, ready])
  return fresh
}

export function RoutingEventsPage() {
  const [range, setRange] = useState<AnalyticsRangeValue>('12h')
  const [activeTypes, setActiveTypes] = useState<Set<RoutingEventType>>(new Set(ALL_EVENT_TYPES))
  const { events, summary, isLoading, isUnavailable, markAllSeen, unseenCount } = useRoutingEvents(range)
  const rangeBadge = PRESET_OPTIONS.find((option) => option.value === range)?.badge ?? range

  const visibleEvents = useMemo(
    () => events.filter((event) => activeTypes.has(event.event_type)),
    [events, activeTypes],
  )

  // The feed asks for the server's default limit, so a full page may be missing older events.
  const isTruncated = events.length >= ROUTING_EVENTS_DEFAULT_LIMIT

  const flipDetails = useMemo(() => leaderFlipDetails(events), [events])
  const retunedBefore = useMemo(() => retunedBeforeIds(events), [events])

  // Prefer the server's whole-window summary; derive one from the feed for older servers.
  const { summaries } = useMemo(() => {
    if (summary) {
      return {
        summaries: summariesFromServer(summary.dimensions, summary.windowStartMs, events),
        windowEndMs: summary.windowEndMs,
      }
    }
    const nowMs = Date.now()
    const windowStartMs = nowMs - (rangeDurationMs(range) ?? 0)
    return { summaries: summarizeDimensions(events, windowStartMs, nowMs, isTruncated), windowEndMs: nowMs }
  }, [summary, events, range, isTruncated])

  const colorFor = useMemo(
    () =>
      connectorColors([
        ...summaries.flatMap((dimension) => (dimension.standings ?? []).map((standing) => standing.gateway)),
        ...summaries.flatMap((dimension) =>
          dimension.timeline.map((span) => span.gateway).filter((gateway): gateway is string => Boolean(gateway)),
        ),
      ]),
    [summaries],
  )

  // One column per dimension set, each with its own changes grouped as in the feed. The columns
  // wrap to fill the width, so every set fits on one screen without sideways scrolling.
  const lanes = useMemo(
    () => buildLanes(visibleEvents).map((lane) => ({ lane, items: groupFeed(lane.events) })),
    [visibleEvents],
  )
  const feedIds = useMemo(
    () => lanes.flatMap(({ items }) => items.map((item) => (item.kind === 'event' ? item.event.id : item.id))),
    [lanes],
  )
  const newlyArrived = useNewlyArrived(feedIds, range, !isLoading)

  // Headline numbers for the stat row. Counts come from the whole-window summary, not the feed.
  const stats = useMemo(() => {
    const totalChanges = summaries.reduce((sum, dimension) => sum + dimension.flipCount, 0)
    const flapping = summaries.filter((dimension) => dimension.isFlapping)
    const latestFlip = events.find((event) => event.event_type === 'leader_changed')
    const retunes = events.filter((event) => event.event_type === 'calibration_applied').length
    return { totalChanges, flapping, latestFlip, retunes }
  }, [summaries, events])

  function toggleType(type: RoutingEventType) {
    setActiveTypes((previous) => {
      const next = new Set(previous)
      if (next.has(type)) {
        next.delete(type)
      } else {
        next.add(type)
      }
      return next
    })
  }

  const refreshSeconds = Math.round(POLL_INTERVAL_MS / 1000)

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <PageHeading
          title="Routing events"
          description="Which PSP leads each payment method on success rate, when that changed, and why."
        />
        <div className="flex items-center gap-2">
          {unseenCount > 0 && (
            <button
              type="button"
              onClick={markAllSeen}
              className={`text-[13px] font-medium leading-[18px] text-brand-600 hover:underline ${focusRing}`}
            >
              Mark all read ({unseenCount})
            </button>
          )}
          <select
            aria-label="Time range"
            value={range}
            onChange={(event) => setRange(event.target.value as AnalyticsRangeValue)}
            className="h-9 rounded-lg border border-[#e6e6ee] bg-white px-3 text-[13px] font-medium leading-[18px] text-slate-700 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-300"
          >
            {PRESET_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-16">
          <Spinner />
        </div>
      ) : isUnavailable ? (
        <Card className="px-6 py-14 text-center">
          <BellRing size={28} className={`mx-auto ${muted}`} aria-hidden />
          <p className={`mt-3 text-sm font-semibold ${strong}`}>Routing events can't be shown here</p>
          <p className={`mx-auto mt-1.5 max-w-[57ch] text-xs ${muted}`}>
            Payment analytics isn't turned on for this environment. Ask whoever runs this deployment to enable it,
            then reload this page.
          </p>
        </Card>
      ) : (
        <>
          {summaries.length > 0 && (
            <div className="grid gap-6 sm:grid-cols-2 xl:grid-cols-4">
              <StatTile
                label="Leader changes"
                value={stats.totalChanges}
                caption={`Across ${pluralize(new Set(summaries.map((summary) => summary.label)).size, 'payment method')} in the last ${rangeBadge}`}
              />
              <StatTile
                label="Oscillating"
                value={stats.flapping.length}
                tone={stats.flapping.length ? 'warn' : undefined}
                caption={
                  stats.flapping.length
                    ? `${stats.flapping.map((dimension) => dimension.label).join(', ')} ${stats.flapping.length === 1 ? 'keeps' : 'keep'} switching leader`
                    : 'Every payment method has a stable leader'
                }
              />
              <StatTile
                label="Latest leader change"
                value={stats.latestFlip ? formatMoment(stats.latestFlip.bucket_ms) : '—'}
                caption={
                  stats.latestFlip
                    ? `${gatewayName(stats.latestFlip.gateway)} took over ${dimensionLabel(stats.latestFlip)}`
                    : 'No leader change in this window'
                }
              />
              <StatTile
                label="Autopilot retunes"
                value={stats.retunes}
                caption={stats.retunes ? 'SR bucket size or hedging adjusted' : 'No retunes in this window'}
              />
            </div>
          )}

          <Card>
            <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 px-6 py-5 dark:border-[#2a303a]">
              <div className="min-w-0">
                <SurfaceLabel>Changes</SurfaceLabel>
                <p className={`mt-1 text-xs ${muted}`}>
                  {visibleEvents.length} event{visibleEvents.length === 1 ? '' : 's'}
                  {isTruncated
                    ? ` · latest ${events.length} shown`
                    : ''}
                  {` · updates every ${pluralize(refreshSeconds, 'second')}`}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Show event types">
                {ALL_EVENT_TYPES.map((type) => {
                  const meta = EVENT_TYPE_META[type]
                  if (!meta) return null
                  const active = activeTypes.has(type)
                  return (
                    <button
                      type="button"
                      key={type}
                      onClick={() => toggleType(type)}
                      aria-pressed={active}
                      className={`flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-[12px] font-medium leading-4 transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-600 ${
                        active
                          ? 'border-brand-600/40 bg-brand-50 text-brand-700 dark:border-sky-400/40 dark:bg-sky-400/10 dark:text-sky-200'
                          : 'border-[#e6e6ee] bg-white text-slate-500 hover:bg-slate-50 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-400 dark:hover:bg-[#18181f]'
                      }`}
                    >
                      <meta.icon size={13} aria-hidden />
                      {meta.label}
                    </button>
                  )
                })}
              </div>
            </div>
            {visibleEvents.length === 0 ? (
              <div className="px-6 py-14 text-center">
                <BellRing size={28} className={`mx-auto ${muted}`} aria-hidden />
                <p className={`mt-3 text-sm font-semibold ${strong}`}>No routing events in this window</p>
                <p className={`mx-auto mt-1.5 max-w-[57ch] text-xs ${muted}`}>
                  Events appear once success-rate routing has scored payments and one PSP overtakes another. Try a
                  longer time range, or{' '}
                  <Link
                    to="/routing/sr"
                    className={`font-medium text-brand-600 hover:underline dark:text-sky-300 ${focusRing}`}
                  >
                    set up success-rate routing
                  </Link>
                  .
                </p>
              </div>
            ) : (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(21rem,1fr))] items-start gap-4 px-4 pb-6 pt-5 sm:px-6">
                {lanes.map(({ lane, items }) => (
                  <LaneColumn
                    key={lane.key}
                    lane={lane}
                    items={items}
                    flipDetails={flipDetails}
                    retunedBefore={retunedBefore}
                    colorFor={colorFor}
                    newlyArrived={newlyArrived}
                  />
                ))}
              </div>
            )}
          </Card>
        </>
      )}
    </div>
  )
}
