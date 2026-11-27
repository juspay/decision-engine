import type { AnalyticsRangeValue, RoutingDimensionSummary, RoutingEvent, ScoreTrend } from '../types/api'

/**
 * Read-only insights over the routing-events feed. The server's per-dimension summary and
 * leader-change context are the source of truth when present; for servers that predate them
 * the same shapes are derived from the events themselves, so the page works against both.
 */

/** A flip won by less than this (in percentage points) is labelled a narrow lead. */
export const NARROW_MARGIN_PP = 2
/** Flips in one dimension this close together are folded into one back-and-forth run. */
export const FLAP_RUN_GAP_MS = 2 * 60 * 60 * 1000
/** Back-to-back events of the same kind are grouped once there are more than two. */
export const GROUP_MIN_EVENTS = 3
/** A group of leader changes this large is labelled Flapping; a smaller one is a plain switch and back. */
export const FLAP_RUN_MIN_FLIPS = 3
/** A dimension is flagged as flapping when this many flips land in any FLAP_WINDOW_MS. */
export const FLAPPING_MIN_FLIPS = 4
export const FLAP_WINDOW_MS = 6 * 60 * 60 * 1000

const RANGE_MS: Partial<Record<AnalyticsRangeValue, number>> = {
  '15m': 15 * 60 * 1000,
  '1h': 60 * 60 * 1000,
  '12h': 12 * 60 * 60 * 1000,
  '1d': 24 * 60 * 60 * 1000,
  '1w': 7 * 24 * 60 * 60 * 1000,
}

export function rangeDurationMs(range: AnalyticsRangeValue): number | null {
  return RANGE_MS[range] ?? null
}

/**
 * What a leader is tracked per: the payment method plus, when the server sends it, the SR v3
 * cluster. Each cluster keeps its own scores, so Visa and Mastercard leaders are separate.
 */
export function dimensionKey(event: Pick<RoutingEvent, 'payment_method_type' | 'payment_method' | 'sr_dimension'>): string {
  return `${event.payment_method_type ?? ''}|${event.payment_method ?? ''}|${event.sr_dimension ?? ''}`
}

/**
 * A cluster's dimensions beyond its payment method, read from the SR v3 cluster label:
 * "ORDER_PAYMENT, DEBIT, CARD, VISA, USD, THREE_DS" reads "Visa, USD, 3DS". The label starts
 * with the payment type, payment method type and payment method; what follows narrows it.
 */
export function srClusterQualifiers(srDimension: string | null | undefined): string | null {
  if (!srDimension) return null
  const parts = srDimension
    .split(',')
    .map((part) => part.trim())
    .slice(3)
    .filter(Boolean)
    .map((part) => {
      const upper = part.toUpperCase()
      if (AUTH_TYPE_LABELS[upper]) return AUTH_TYPE_LABELS[upper]
      // Currency and country codes stay as codes.
      if (/^[A-Z]{2,3}$/.test(part)) return part
      return part
        .toLowerCase()
        .split(/[\s_]+/)
        .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ')
    })
  return parts.length ? parts.join(', ') : null
}

/** The SR dimensions an event can be split by, each shown in its own column. */
export const DIMENSION_COLUMNS = [
  { key: 'card_network', label: 'Network' },
  { key: 'currency', label: 'Currency' },
  { key: 'country', label: 'Country' },
  { key: 'auth_type', label: '3DS' },
  { key: 'card_is_in', label: 'BIN' },
] as const

export type DimensionColumn = (typeof DIMENSION_COLUMNS)[number]['key']

/** A dimension's value as people say it: VISA reads "Visa", THREE_DS reads "3DS". */
export function dimensionValue(event: RoutingEvent, column: DimensionColumn): string | null {
  const value = event[column]
  if (!value) return null
  if (column === 'auth_type') return AUTH_TYPE_LABELS[value.toUpperCase()] ?? value
  if (column === 'card_network') {
    return value
      .toLowerCase()
      .split(/[\s_]+/)
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ')
  }
  return value
}

/** The dimension columns at least one of these events has a value for, in display order. */
export function activeDimensionColumns(events: RoutingEvent[]): DimensionColumn[] {
  return DIMENSION_COLUMNS.map((column) => column.key).filter((key) => events.some((event) => event[key]))
}

/** One dimension set (an SR cluster) and its events, shown as a column of its own. */
export interface EventLane {
  key: string
  /** The payment method, e.g. "Credit card". */
  label: string
  /** The rest of the set, e.g. ["Visa", "USD", "3DS"]. */
  dimensions: string[]
  /** Newest first. */
  events: RoutingEvent[]
  /** The leader after the newest leader change, if the lane has one. */
  leader: string | null
  latestMs: number
}

/**
 * An event with its dimension fields filled in. Leader changes recorded before the server sent
 * each dimension separately only carry the cluster label ("UNKNOWN, DEBIT, CARD, Visa, USD,
 * THREE_DS"); its values are told apart by their shape: an auth type, a 3-letter currency, a
 * 2-letter country, a numeric BIN, and otherwise the card network.
 */
function withDimensionFields(event: RoutingEvent): RoutingEvent {
  if (!event.sr_dimension || DIMENSION_COLUMNS.some((column) => event[column.key])) return event
  const fields: Partial<Record<DimensionColumn, string>> = {}
  for (const part of event.sr_dimension.split(',').map((value) => value.trim()).slice(3)) {
    const upper = part.toUpperCase()
    const column: DimensionColumn =
      upper in AUTH_TYPE_LABELS
        ? 'auth_type'
        : /^\d{6,8}$/.test(part)
          ? 'card_is_in'
          : /^[A-Z]{3}$/.test(part)
            ? 'currency'
            : /^[A-Z]{2}$/.test(part)
              ? 'country'
              : 'card_network'
    fields[column] ??= part
  }
  return { ...event, ...fields }
}

/**
 * Which set an event belongs to: its payment method plus each dimension it was scored on. A
 * leader change and an Autopilot retune of the same set land together, whatever case the
 * values came in. Leader changes recorded before dimensions were fall back to the cluster
 * label, then to the payment method alone.
 */
export function laneKey(event: RoutingEvent): string {
  // Keyed on exactly what the lane's header shows, so sets that read the same share one lane.
  return [humanizeDimension(event.payment_method_type, event.payment_method), ...laneDimensions(event)]
    .map((part) => part.toUpperCase())
    .join(' · ')
}

/** The set's dimensions as shown, e.g. ["Visa", "USD", "3DS"]. */
function laneDimensions(event: RoutingEvent): string[] {
  const withFields = withDimensionFields(event)
  return DIMENSION_COLUMNS.map((column) => dimensionValue(withFields, column.key)).filter(
    (value): value is string => Boolean(value),
  )
}

/** The events split into one lane per dimension set, the most recently active set first. */
export function buildLanes(events: RoutingEvent[]): EventLane[] {
  const lanes = new Map<string, EventLane>()
  for (const event of [...events].sort((a, b) => b.bucket_ms - a.bucket_ms)) {
    const key = laneKey(event)
    let lane = lanes.get(key)
    if (!lane) {
      lane = {
        key,
        label: humanizeDimension(event.payment_method_type, event.payment_method),
        dimensions: laneDimensions(event),
        events: [],
        leader: null,
        latestMs: event.bucket_ms,
      }
      lanes.set(key, lane)
    }
    if (lane.leader == null && event.event_type === 'leader_changed') lane.leader = event.gateway
    lane.events.push(event)
  }
  return [...lanes.values()].sort((a, b) => b.latestMs - a.latestMs || a.key.localeCompare(b.key))
}

/** What narrows an event beyond its payment method: its SR cluster, or a retune's cluster. */
export function eventQualifiers(event: RoutingEvent): string | null {
  return event.event_type === 'calibration_applied' ? calibrationQualifiers(event) : srClusterQualifiers(event.sr_dimension)
}

const ACRONYMS = new Set(['UPI', 'EMI', 'BNPL', 'ACH', 'SEPA', 'EFT', 'NB', 'BACS', 'PIX', '3DS'])

/**
 * A payment method as people say it: CREDIT + CARD reads "Credit card", UPI + UPI_COLLECT reads
 * "UPI collect". Words repeated across the two parts are said once; known acronyms stay upper.
 */
export function humanizeDimension(
  paymentMethodType: string | null | undefined,
  paymentMethod: string | null | undefined,
): string {
  const words: string[] = []
  for (const part of [paymentMethodType, paymentMethod]) {
    for (const word of (part ?? '').split(/[\s_-]+/).filter(Boolean)) {
      if (!words.some((seen) => seen.toLowerCase() === word.toLowerCase())) words.push(word)
    }
  }
  if (!words.length) return 'All payments'
  return words
    .map((word, index) => {
      if (ACRONYMS.has(word.toUpperCase())) return word.toUpperCase()
      const lower = word.toLowerCase()
      return index === 0 ? lower.charAt(0).toUpperCase() + lower.slice(1) : lower
    })
    .join(' ')
}

export function dimensionLabel(event: RoutingEvent): string {
  return humanizeDimension(event.payment_method_type, event.payment_method)
}

export function formatScore(score: number | null | undefined): string | null {
  return score == null ? null : `${(score * 100).toFixed(1)}%`
}

/**
 * Winning margin of a leader flip in percentage points, rounded to the 0.1 pp it is shown at
 * (so a 1.9999 margin is both displayed and classified as 2.0), or null when a score is missing.
 */
export function marginPp(event: RoutingEvent): number | null {
  if (event.score == null || event.previous_score == null) return null
  return Math.round((event.score - event.previous_score) * 1000) / 10
}

export type FlipKind = 'decisive' | 'narrow' | 'stale' | 'unknown'

/**
 * How a leader flip happened. The detector hands the lead over outright when the incumbent
 * ages out (no fresh score / too few txns): that is a `stale` handover, not a win. The
 * server says so in `leader_change_reason`; without it, a new leader scoring at or below the
 * old one's last value can only have come from such a handover.
 */
export function flipKind(event: RoutingEvent): FlipKind {
  if (event.leader_change_reason === 'incumbent_stale') return 'stale'
  const margin = marginPp(event)
  if (margin == null) return 'unknown'
  if (event.leader_change_reason == null && margin <= 0) return 'stale'
  return margin < NARROW_MARGIN_PP ? 'narrow' : 'decisive'
}

/** A score move smaller than this (pp) between the bucket before a flip and the flip is noise. */
export const SCORE_MOVE_MIN_PP = 0.5

export interface ScoreMove {
  gateway: string
  fromScore: number
  toScore: number
}

/**
 * What moved in the bucket that flipped the lead: the outgoing leader falling, the new
 * leader rising, or both. Empty when the server sends no before-scores or nothing moved
 * enough to matter.
 */
export function flipCause(event: RoutingEvent): { fell: ScoreMove | null; rose: ScoreMove | null } {
  const moved = (gateway: string | null, from?: number | null, to?: number | null) =>
    gateway && from != null && to != null ? { gateway, fromScore: from, toScore: to } : null
  const outgoing = moved(event.previous_gateway, event.previous_score_before, event.previous_score)
  const incoming = moved(event.gateway, event.score_before, event.score)
  const minMove = SCORE_MOVE_MIN_PP / 100
  return {
    fell: outgoing && outgoing.fromScore - outgoing.toScore >= minMove ? outgoing : null,
    rose: incoming && incoming.toScore - incoming.fromScore >= minMove ? incoming : null,
  }
}

/**
 * Why the lead moved, as one sentence under the "Stripe → Adyen" title: a stale handover, the
 * scores that moved, or failing those the two scores at the flip. `name` renders a gateway for
 * display, so the sentence uses the same names as the rest of the page.
 */
export function flipCauseText(event: RoutingEvent, name: (gateway: string) => string = (g) => g): string {
  const to = name(event.gateway)
  const from = event.previous_gateway ? name(event.previous_gateway) : null
  const pct = (score: number | null | undefined) => formatScore(score) ?? 'n/a'
  if (!from) return `${to} took the lead at ${pct(event.score)}.`
  if (flipKind(event) === 'stale') {
    return `${from} stopped receiving payments, so the lead passed to ${to} (${pct(event.score)}) without a contest.`
  }
  const { fell, rose } = flipCause(event)
  if (fell && rose) {
    return `${from} fell from ${pct(fell.fromScore)} to ${pct(fell.toScore)} while ${to} rose from ${pct(rose.fromScore)} to ${pct(rose.toScore)}.`
  }
  if (fell) return `${from} fell from ${pct(fell.fromScore)} to ${pct(fell.toScore)}; ${to} held ${pct(event.score)}.`
  if (rose) return `${to} rose from ${pct(rose.fromScore)} to ${pct(rose.toScore)}, past ${from} at ${pct(event.previous_score)}.`
  if (event.score != null && event.previous_score != null) {
    return `${to} at ${pct(event.score)} against ${from} at ${pct(event.previous_score)}.`
  }
  return `${to} took over from ${from}.`
}

export function formatDuration(ms: number): string {
  const totalMinutes = Math.max(0, Math.round(ms / 60_000))
  if (totalMinutes < 1) return '<1m'
  const days = Math.floor(totalMinutes / (24 * 60))
  const hours = Math.floor((totalMinutes % (24 * 60)) / 60)
  const minutes = totalMinutes % 60
  if (days > 0) return hours > 0 ? `${days}d ${hours}h` : `${days}d`
  if (hours > 0) return minutes > 0 ? `${hours}h ${String(minutes).padStart(2, '0')}m` : `${hours}h`
  return `${minutes}m`
}

export interface LeaderFlipDetail {
  /** How long the displaced gateway led before this flip; null when its start is outside the feed. */
  previousHeldMs: number | null
  /** Txn count to surface only when the score rests on less data than the dimension's full window. */
  thinTransactionCount: number | null
  /** The previous leader change in the same dimension, if the feed has one. */
  priorFlipMs: number | null
}

/**
 * Per-event detail for leader flips: how long the displaced leader held the top spot
 * (gap to the previous flip in the same dimension that crowned it) and whether the new
 * leader's score was based on a partially filled SR window.
 */
export function leaderFlipDetails(events: RoutingEvent[]): Map<string, LeaderFlipDetail> {
  const byDimension = new Map<string, RoutingEvent[]>()
  for (const event of events) {
    if (event.event_type !== 'leader_changed') continue
    const key = dimensionKey(event)
    const list = byDimension.get(key) ?? []
    list.push(event)
    byDimension.set(key, list)
  }

  const details = new Map<string, LeaderFlipDetail>()
  for (const list of byDimension.values()) {
    list.sort((a, b) => a.bucket_ms - b.bucket_ms)
    // The fullest window seen in this dimension approximates the configured SR window size;
    // a count below it means the new leader's score is still warming up.
    const fullWindow = Math.max(0, ...list.map((event) => event.transaction_count ?? 0))
    list.forEach((event, index) => {
      const prior = index > 0 ? list[index - 1] : null
      const previousHeldMs =
        prior && prior.gateway === event.previous_gateway ? event.bucket_ms - prior.bucket_ms : null
      const count = event.transaction_count
      details.set(event.id, {
        previousHeldMs,
        thinTransactionCount: count != null && count < fullWindow ? count : null,
        priorFlipMs: prior ? prior.bucket_ms : null,
      })
    })
  }
  return details
}

export interface LeaderShare {
  gateway: string
  ms: number
  share: number
}

export interface GatewayStanding {
  gateway: string
  score: number
  transactionCount: number | null
  lastSeenMs: number
  eligible: boolean
}

export interface DimensionSummary {
  key: string
  label: string
  /** The cluster's dimensions beyond the payment method, e.g. "Visa, USD, 3DS"; null when none. */
  qualifiers: string | null
  /** Success rate per gateway and payment volume over the window; null from older servers. */
  trend: ScoreTrend | null
  /** The last reported leader; null when no gateway has led in the window. */
  leader: string | null
  /** When `leader` took over inside the window; null when it already led at the start. */
  leaderSinceMs: number | null
  flipCount: number
  isFlapping: boolean
  /** Share of the covered span each gateway spent as leader, largest first. */
  leaderShares: LeaderShare[]
  /** Time in the covered span with no fresh, eligible score (no one leading). */
  noLeaderMs: number
  noLeaderShare: number
  coveredFromMs: number
  /** Who led when across the covered span; `gateway: null` = no fresh scores. */
  timeline: { gateway: string | null; startMs: number; endMs: number }[]
  /** Every gateway's latest score, best first; null when the server does not send them. */
  standings: GatewayStanding[] | null
  lastActivityMs: number | null
}

/** Dimension keys whose leader changed FLAPPING_MIN_FLIPS+ times within any FLAP_WINDOW_MS. */
export function flappingDimensions(events: RoutingEvent[]): Set<string> {
  const flipTimes = new Map<string, number[]>()
  for (const event of events) {
    if (event.event_type !== 'leader_changed') continue
    const key = dimensionKey(event)
    flipTimes.set(key, [...(flipTimes.get(key) ?? []), event.bucket_ms])
  }
  const flapping = new Set<string>()
  for (const [key, times] of flipTimes) {
    times.sort((a, b) => a - b)
    for (let start = 0, end = 0; end < times.length; end += 1) {
      while (times[end] - times[start] > FLAP_WINDOW_MS) start += 1
      if (end - start + 1 >= FLAPPING_MIN_FLIPS) {
        flapping.add(key)
        break
      }
    }
  }
  return flapping
}

function withShares(held: Map<string, number>, noLeaderMs: number) {
  const totalMs = Array.from(held.values()).reduce((sum, ms) => sum + ms, 0) + noLeaderMs
  const leaderShares = Array.from(held, ([gateway, ms]) => ({
    gateway,
    ms,
    share: totalMs > 0 ? ms / totalMs : 0,
  })).sort((a, b) => b.ms - a.ms || a.gateway.localeCompare(b.gateway))
  return { leaderShares, noLeaderShare: totalMs > 0 ? noLeaderMs / totalMs : 0 }
}

/** Flapping payment methods first, then the most changes, then by name: the order needing attention. */
function sortSummaries(summaries: DimensionSummary[]) {
  return summaries.sort(
    (a, b) =>
      Number(b.isFlapping) - Number(a.isFlapping) || b.flipCount - a.flipCount || a.label.localeCompare(b.label),
  )
}

/**
 * The page's summary from the server's per-dimension leadership: whole-window counts and
 * time shares (not capped by the event limit), with stretches without fresh scores kept
 * apart instead of being credited to a gateway. Flapping is read from the feed's flips.
 */
export function summariesFromServer(
  dimensions: RoutingDimensionSummary[],
  windowStartMs: number,
  events: RoutingEvent[],
): DimensionSummary[] {
  const flapping = flappingDimensions(events)
  return sortSummaries(
    dimensions.map((dimension) => {
      const key = dimensionKey(dimension)
      const held = new Map(dimension.leadership.map((entry) => [entry.gateway, entry.leader_ms]))
      return {
        key,
        label: humanizeDimension(dimension.payment_method_type, dimension.payment_method),
        qualifiers: srClusterQualifiers(dimension.sr_dimension),
        trend: dimension.trend ?? null,
        leader: dimension.leader,
        leaderSinceMs: dimension.leader_since_ms,
        flipCount: dimension.leader_change_count,
        isFlapping: flapping.has(key),
        ...withShares(held, dimension.no_leader_ms),
        noLeaderMs: dimension.no_leader_ms,
        coveredFromMs: windowStartMs,
        timeline: (dimension.timeline ?? []).map((span) => ({
          gateway: span.gateway,
          startMs: span.start_ms,
          endMs: span.end_ms,
        })),
        standings: dimension.standings.map((standing) => ({
          gateway: standing.gateway,
          score: standing.score,
          transactionCount: standing.transaction_count,
          lastSeenMs: standing.last_seen_ms,
          eligible: standing.eligible,
        })),
        lastActivityMs: dimension.last_activity_ms,
      }
    }),
  )
}

/**
 * Fallback for servers without the summary: derive it from the feed's leader flips. Before
 * the first in-feed flip the leader is that flip's `previous_gateway`. When the feed was
 * truncated (`isTruncated`) the span starts at the oldest returned event, since the share
 * before older, missing flips would be guessed. Only dimensions with flips appear.
 */
export function summarizeDimensions(
  events: RoutingEvent[],
  windowStartMs: number,
  nowMs: number,
  isTruncated: boolean,
): DimensionSummary[] {
  const leaderEvents = events.filter((event) => event.event_type === 'leader_changed')
  const oldestMs = leaderEvents.reduce((min, event) => Math.min(min, event.bucket_ms), nowMs)
  const coveredFromMs = isTruncated ? oldestMs : Math.min(windowStartMs, oldestMs)
  const flapping = flappingDimensions(events)

  const byDimension = new Map<string, RoutingEvent[]>()
  for (const event of leaderEvents) {
    const key = dimensionKey(event)
    byDimension.set(key, [...(byDimension.get(key) ?? []), event])
  }

  const summaries: DimensionSummary[] = []
  for (const [key, list] of byDimension) {
    list.sort((a, b) => a.bucket_ms - b.bucket_ms)
    const held = new Map<string, number>()
    const addHeld = (gateway: string | null, ms: number) => {
      if (!gateway || ms <= 0) return
      held.set(gateway, (held.get(gateway) ?? 0) + ms)
    }
    addHeld(list[0].previous_gateway, list[0].bucket_ms - coveredFromMs)
    list.forEach((event, index) => {
      const endMs = index + 1 < list.length ? list[index + 1].bucket_ms : nowMs
      addHeld(event.gateway, endMs - event.bucket_ms)
    })
    const timeline = [
      { gateway: list[0].previous_gateway, startMs: coveredFromMs, endMs: list[0].bucket_ms },
      ...list.map((event, index) => ({
        gateway: event.gateway as string | null,
        startMs: event.bucket_ms,
        endMs: index + 1 < list.length ? list[index + 1].bucket_ms : nowMs,
      })),
    ].filter((span) => span.endMs > span.startMs)
    const latest = list[list.length - 1]
    summaries.push({
      key,
      label: dimensionLabel(latest),
      qualifiers: srClusterQualifiers(latest.sr_dimension),
      trend: null,
      leader: latest.gateway,
      leaderSinceMs: latest.bucket_ms,
      flipCount: list.length,
      isFlapping: flapping.has(key),
      ...withShares(held, 0),
      noLeaderMs: 0,
      coveredFromMs,
      timeline,
      standings: null,
      lastActivityMs: null,
    })
  }
  return sortSummaries(summaries)
}

export type FeedItem =
  | { kind: 'event'; event: RoutingEvent }
  | {
      kind: 'flap_run'
      id: string
      label: string
      /** Gateways that traded the lead, in order of first appearance. */
      gateways: string[]
      /** Newest first, matching the feed. */
      events: RoutingEvent[]
      startMs: number
      endMs: number
    }
  | {
      kind: 'retune_run'
      id: string
      /** The cluster the autopilot kept retuning, e.g. "Debit card (Visa, USD, 3DS)". */
      label: string
      /** Bucket sizes in order, starting from the value before the first retune. */
      bucketPath: number[]
      /** Hedging % in order, the same way; a single entry means it never changed. */
      hedgingPath: number[]
      /** Newest first, matching the feed. */
      events: RoutingEvent[]
      startMs: number
      endMs: number
    }

const AUTH_TYPE_LABELS: Record<string, string> = { THREE_DS: '3DS', NO_THREE_DS: 'No 3DS' }

/** "Debit card (Visa, USD, 3DS)": the payment method plus whatever narrows the cluster. */
/** A cluster's dimensions beyond its payment method, e.g. "Visa, USD, 3DS", or null when it has none. */
export function calibrationQualifiers(event: RoutingEvent): string | null {
  const titleCase = (value: string) =>
    value
      .toLowerCase()
      .split(/[\s_]+/)
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ')
  const qualifiers = [
    event.card_network ? titleCase(event.card_network) : null,
    event.currency?.toUpperCase() ?? null,
    event.country?.toUpperCase() ?? null,
    event.auth_type ? (AUTH_TYPE_LABELS[event.auth_type.toUpperCase()] ?? titleCase(event.auth_type)) : null,
  ].filter((part): part is string => Boolean(part))
  return qualifiers.length ? qualifiers.join(', ') : null
}

export function calibrationClusterLabel(event: RoutingEvent): string {
  const scope = humanizeDimension(event.payment_method_type, event.payment_method)
  const qualifiers = calibrationQualifiers(event)
  return qualifiers ? `${scope} (${qualifiers})` : scope
}

function calibrationKey(event: RoutingEvent): string {
  return [
    event.payment_method_type,
    event.payment_method,
    event.card_network,
    event.currency,
    event.country,
    event.auth_type,
  ].join('|')
}

/**
 * What one retune changed, naming only the settings that moved ("bucket size 250 → 300"). A
 * retune without previous values states its settings instead: "(first tuning)" for a cluster's
 * first, or "(started over)" when `retunedBefore` says the cluster was tuned earlier and its
 * settings were since cleared (Hard refresh), so Autopilot tuned it again from scratch.
 */
export function describeCalibrationChange(event: RoutingEvent, retunedBefore = false): string {
  const hedge = (value: number) => `${value.toFixed(1)}%`
  const changes: string[] = []
  const bucketMoved =
    event.bucket_size != null && event.previous_bucket_size != null && event.previous_bucket_size !== event.bucket_size
  const hedgeMoved =
    event.hedging_percent != null &&
    event.previous_hedging_percent != null &&
    event.previous_hedging_percent !== event.hedging_percent
  if (bucketMoved) changes.push(`bucket size ${event.previous_bucket_size} → ${event.bucket_size}`)
  if (hedgeMoved && event.previous_hedging_percent != null && event.hedging_percent != null) {
    changes.push(`hedging ${hedge(event.previous_hedging_percent)} → ${hedge(event.hedging_percent)}`)
  }
  if (changes.length) return changes.join(', ')
  const settings = [
    event.bucket_size != null ? `bucket size ${event.bucket_size}` : null,
    event.hedging_percent != null ? `hedging ${hedge(event.hedging_percent)}` : null,
  ].filter(Boolean)
  if (!settings.length) return 'settings unchanged'
  const isFirstTuning = event.previous_bucket_size == null && event.previous_hedging_percent == null
  const note = !isFirstTuning ? 'unchanged' : retunedBefore ? 'started over' : 'first tuning'
  return `${settings.join(', ')} (${note})`
}

/**
 * Retunes whose cluster was already tuned earlier in `events`. One of these without previous
 * values started over (its settings were cleared) rather than being the cluster's first tuning.
 */
export function retunedBeforeIds(events: RoutingEvent[]): Set<string> {
  const ids = new Set<string>()
  const seen = new Set<string>()
  for (const event of [...events].sort((a, b) => a.bucket_ms - b.bucket_ms)) {
    if (event.event_type !== 'calibration_applied') continue
    const key = calibrationKey(event)
    if (seen.has(key)) ids.add(event.id)
    seen.add(key)
  }
  return ids
}

/** A value's path through a run of retunes, without repeats where it held still. */
function settingPath(chronological: RoutingEvent[], read: (event: RoutingEvent) => [number | null | undefined, number | null | undefined]) {
  const path: number[] = []
  const push = (value: number | null | undefined) => {
    if (value != null && path[path.length - 1] !== value) path.push(value)
  }
  chronological.forEach((event, index) => {
    const [previous, next] = read(event)
    if (index === 0) push(previous)
    push(next)
  })
  return path
}

/**
 * The feed in time order (newest first), with back-to-back events of the same kind grouped:
 * - leader changes in the same dimension (`flap_run`);
 * - autopilot retunes of the same cluster (`retune_run`).
 * A group needs GROUP_MIN_EVENTS or more, each within FLAP_RUN_GAP_MS of the next. Any other
 * event between them ends the group, so every item keeps its place in time.
 */
export function groupFeed(events: RoutingEvent[]): FeedItem[] {
  const items: FeedItem[] = []
  let run: RoutingEvent[] = []

  const runKey = (event: RoutingEvent) => {
    if (event.event_type === 'leader_changed') return `flip|${dimensionKey(event)}`
    if (event.event_type === 'calibration_applied') return `retune|${calibrationKey(event)}`
    return null
  }

  const flush = () => {
    if (run.length >= GROUP_MIN_EVENTS) {
      const chronological = [...run].reverse()
      const startMs = chronological[0].bucket_ms
      const endMs = run[0].bucket_ms
      if (run[0].event_type === 'leader_changed') {
        const gateways: string[] = []
        for (const event of chronological) {
          for (const gateway of [event.previous_gateway, event.gateway]) {
            if (gateway && !gateways.includes(gateway)) gateways.push(gateway)
          }
        }
        items.push({ kind: 'flap_run', id: `run:${run[0].id}`, label: dimensionLabel(run[0]), gateways, events: run, startMs, endMs })
      } else {
        items.push({
          kind: 'retune_run',
          id: `retunes:${run[0].id}`,
          label: calibrationClusterLabel(run[0]),
          bucketPath: settingPath(chronological, (event) => [event.previous_bucket_size, event.bucket_size]),
          hedgingPath: settingPath(chronological, (event) => [event.previous_hedging_percent, event.hedging_percent]),
          events: run,
          startMs,
          endMs,
        })
      }
    } else {
      for (const event of run) items.push({ kind: 'event', event })
    }
    run = []
  }

  for (const event of events) {
    const key = runKey(event)
    const last = run[run.length - 1]
    const continuesRun =
      last != null && key != null && key === runKey(last) && last.bucket_ms - event.bucket_ms <= FLAP_RUN_GAP_MS
    if (!continuesRun) flush()
    if (key != null) {
      run.push(event)
    } else {
      items.push({ kind: 'event', event })
    }
  }
  flush()
  return items
}

/**
 * Payment audit link for the payments behind one flip: the flip's dimension, only the two PSPs
 * that traded the lead, from when the outgoing leader took over (the previous flip in this
 * dimension, at most an hour back) through the flip's minute. Those are the outcomes that
 * moved the scores. The routing event carries no payment ids, so this is as exact as the
 * feed allows.
 *
 * The end is capped at `nowMs` because the audit page rejects (and silently skips searching)
 * a custom window that ends in the future.
 */
export function auditLinkForEvent(
  event: RoutingEvent,
  priorFlipMs: number | null = null,
  nowMs: number = Date.now(),
): string {
  const maxLookbackMs = 60 * 60 * 1000
  const startMs = Math.max(priorFlipMs ?? 0, event.bucket_ms - maxLookbackMs)
  const params = new URLSearchParams({
    start_ms: String(startMs),
    end_ms: String(Math.min(event.bucket_ms + 60_000, nowMs)),
  })
  const gateways = [event.previous_gateway, event.gateway].filter((g): g is string => Boolean(g))
  if (gateways.length) params.set('gateway', gateways.join(','))
  if (event.payment_method_type) params.set('payment_method_type', event.payment_method_type)
  if (event.payment_method) params.set('payment_method', event.payment_method)
  return `/audit?${params.toString()}`
}
