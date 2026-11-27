import {
  ReactNode,
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import useSWR from 'swr'
import { fetcher } from '../lib/api'
import { useAuthStore } from '../store/authStore'
import {
  AnalyticsRangeValue,
  RoutingDimensionSummary,
  RoutingEvent,
  RoutingEventsResponse,
} from '../types/api'
import { calibrationClusterLabel, describeCalibrationChange } from '../lib/routingEventInsights'

// Every poll re-reads the last hour of score snapshots from ClickHouse, per open tab.
export const POLL_INTERVAL_MS = 5_000
const SEEN_STORAGE_PREFIX = 'routing_events_seen'
const MAX_SEEN_IDS = 500

// The always-on live feed. One poll at this range is shared by every
// always-mounted consumer (the notification bell) and the simulator, so they no
// longer issue overlapping requests. Wider, user-driven windows (the events page
// browsing history) are a genuinely different query and open their own.
const LIVE_RANGE: AnalyticsRangeValue = '1h'

// Short windows get second-granularity detection (catches crossings inside a
// burst); longer windows use minute buckets to keep the scan bounded.
function bucketForRange(range: AnalyticsRangeValue): string {
  return range === '15m' || range === '1h' ? '1s' : '1m'
}

// Mirrors the server's DEFAULT_ROUTING_EVENTS_LIMIT. The feed requests the default, so a
// response this long may be truncated; the per-dimension summary is not.
export const ROUTING_EVENTS_DEFAULT_LIMIT = 50

function routingEventsKey(merchantId: string, range: AnalyticsRangeValue): string | null {
  if (!merchantId) return null
  return `/analytics/routing-events?range=${range}&bucket=${bucketForRange(range)}`
}

/** Whether a range is scanned at per-second granularity (longer ranges use 1-minute buckets). */
export function isPerSecondRange(range: AnalyticsRangeValue): boolean {
  return bucketForRange(range) === '1s'
}

/** The window-wide extras newer servers send alongside the events. */
export interface RoutingEventsSummary {
  dimensions: RoutingDimensionSummary[]
  windowStartMs: number
  windowEndMs: number
}

function summaryOf(response: RoutingEventsResponse | undefined): RoutingEventsSummary | null {
  if (!response?.dimensions || response.window_start_ms == null || response.window_end_ms == null) {
    return null
  }
  return {
    dimensions: response.dimensions,
    windowStartMs: response.window_start_ms,
    windowEndMs: response.window_end_ms,
  }
}

function seenStorageKey(merchantId: string) {
  return `${SEEN_STORAGE_PREFIX}:${merchantId}`
}

function loadSeenIds(merchantId: string): Set<string> {
  try {
    const raw = localStorage.getItem(seenStorageKey(merchantId))
    if (!raw) return new Set()
    const parsed = JSON.parse(raw)
    return new Set(Array.isArray(parsed) ? parsed.filter((id) => typeof id === 'string') : [])
  } catch {
    return new Set()
  }
}

function persistSeenIds(merchantId: string, ids: Set<string>) {
  try {
    // Event IDs are stable across polls; cap stored history so it never grows unbounded.
    localStorage.setItem(
      seenStorageKey(merchantId),
      JSON.stringify(Array.from(ids).slice(-MAX_SEEN_IDS)),
    )
  } catch {
    // localStorage unavailable (private mode/quota): unseen state just resets per session.
  }
}

interface RoutingEventsContextValue {
  liveEvents: RoutingEvent[]
  liveSummary: RoutingEventsSummary | null
  isLoading: boolean
  isUnavailable: boolean
  isSeen: (id: string) => boolean
  markSeen: (events: RoutingEvent[]) => void
  refresh: () => void
  // Let a single consumer (the simulator) tighten the shared poll while it is
  // actively producing events; pass null to release back to the idle default.
  requestPollInterval: (intervalMs: number | null) => void
  // Register as an active live-feed consumer; returns an unsubscribe. The shared
  // poll only runs while the subscriber count is > 0, so pages that never show
  // events never trigger a request.
  subscribe: () => () => void
}

const RoutingEventsContext = createContext<RoutingEventsContextValue | null>(null)

// Owns the one live-feed poll and the merchant-scoped seen-state. Mount once,
// above the notification bell and the routed pages, so every consumer of
// `useRoutingEvents` shares a single network poll and one consistent seen-set.
export function RoutingEventsProvider({ children }: { children: ReactNode }) {
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  const [pollIntervalMs, setPollIntervalMs] = useState(POLL_INTERVAL_MS)

  // The live poll runs only while at least one consumer is actively subscribed
  // (the bell while its dropdown is open, the simulator/events page while
  // mounted). With no subscribers the SWR key goes null and polling stops, so we
  // don't hit /analytics/routing-events on pages that never surface events.
  const subscribersRef = useRef(0)
  const [hasSubscribers, setHasSubscribers] = useState(false)

  const subscribe = useCallback(() => {
    subscribersRef.current += 1
    setHasSubscribers(true)
    return () => {
      subscribersRef.current -= 1
      if (subscribersRef.current <= 0) {
        subscribersRef.current = 0
        setHasSubscribers(false)
      }
    }
  }, [])

  const { data, error, isLoading, mutate } = useSWR<RoutingEventsResponse>(
    hasSubscribers ? routingEventsKey(merchantId, LIVE_RANGE) : null,
    fetcher,
    { refreshInterval: pollIntervalMs, revalidateOnFocus: false },
  )

  const [seenIds, setSeenIds] = useState<Set<string>>(() => loadSeenIds(merchantId))

  useEffect(() => {
    setSeenIds(loadSeenIds(merchantId))
  }, [merchantId])

  const isSeen = useCallback((id: string) => seenIds.has(id), [seenIds])

  const markSeen = useCallback(
    (events: RoutingEvent[]) => {
      setSeenIds((previous) => {
        const next = new Set(previous)
        for (const event of events) next.add(event.id)
        persistSeenIds(merchantId, next)
        return next
      })
    },
    [merchantId],
  )

  // Force an immediate re-fetch (bypassing the poll timer) — used to pull fresh
  // events the moment a simulation batch lands.
  const refresh = useCallback(() => {
    void mutate()
  }, [mutate])

  const requestPollInterval = useCallback((intervalMs: number | null) => {
    setPollIntervalMs(intervalMs ?? POLL_INTERVAL_MS)
  }, [])

  // Keep the last non-empty feed so a consumer that unsubscribes (e.g. the bell
  // closing, pausing the poll) still shows its last known badge count instead of
  // snapping to zero while nothing is polling.
  const lastEventsRef = useRef<RoutingEvent[]>([])
  useEffect(() => {
    if (data?.events) lastEventsRef.current = data.events
  }, [data])

  // Degrade silently when analytics is unavailable (e.g. ClickHouse disabled).
  const liveEvents = useMemo<RoutingEvent[]>(
    () => (error ? [] : data?.events ?? lastEventsRef.current),
    [error, data],
  )

  const liveSummary = useMemo(() => (error ? null : summaryOf(data)), [error, data])

  const value = useMemo<RoutingEventsContextValue>(
    () => ({
      liveEvents,
      liveSummary,
      isLoading,
      isUnavailable: Boolean(error),
      isSeen,
      markSeen,
      refresh,
      requestPollInterval,
      subscribe,
    }),
    [liveEvents, liveSummary, isLoading, error, isSeen, markSeen, refresh, requestPollInterval, subscribe],
  )

  return createElement(RoutingEventsContext.Provider, { value }, children)
}

function useRoutingEventsContext(): RoutingEventsContextValue {
  const context = useContext(RoutingEventsContext)
  if (!context) {
    throw new Error('useRoutingEvents must be used within a <RoutingEventsProvider>')
  }
  return context
}

interface UseRoutingEventsOptions {
  // Override the shared poll cadence — e.g. tighten it while a simulation is
  // actively producing events so the Autopilot feed keeps up. Omit to use the
  // idle default. Only honored for the live feed (the bell + simulator share it).
  refreshInterval?: number
  // Whether this consumer currently needs live data. Defaults to true; pass
  // false (e.g. the bell while its dropdown is closed) so the consumer doesn't
  // subscribe and the shared poll can stay paused when nothing else needs it.
  enabled?: boolean
}

export function useRoutingEvents(
  range: AnalyticsRangeValue = LIVE_RANGE,
  options: UseRoutingEventsOptions = {},
) {
  const { refreshInterval, enabled = true } = options
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  const {
    liveEvents,
    liveSummary,
    isLoading: liveLoading,
    isUnavailable: liveUnavailable,
    isSeen,
    markSeen,
    refresh: refreshLive,
    requestPollInterval,
    subscribe,
  } = useRoutingEventsContext()

  const isLive = range === LIVE_RANGE

  // Register as an active live-feed consumer so the provider polls; the cleanup
  // releases on unmount / when disabled, pausing the shared poll once nothing
  // needs it. Non-live windows drive their own SWR below instead.
  useEffect(() => {
    if (!isLive || !enabled) return
    return subscribe()
  }, [isLive, enabled, subscribe])

  // Live consumers share the provider's single poll; a caller that passes an
  // explicit cadence retunes it, then releases on unmount/stop so the bell's
  // idle cadence is restored.
  useEffect(() => {
    if (!isLive || refreshInterval === undefined) return
    requestPollInterval(refreshInterval)
    return () => requestPollInterval(null)
  }, [isLive, refreshInterval, requestPollInterval])

  // Wider, user-driven windows (the events page) open their own subscription;
  // the live feed already covers LIVE_RANGE for the bell + simulator.
  const { data, error, isLoading: rangeLoading, mutate } = useSWR<RoutingEventsResponse>(
    !isLive && enabled ? routingEventsKey(merchantId, range) : null,
    fetcher,
    { refreshInterval: refreshInterval ?? POLL_INTERVAL_MS, revalidateOnFocus: false },
  )

  const events = isLive ? liveEvents : error ? [] : data?.events ?? []
  const rangeSummary = useMemo(() => (error ? null : summaryOf(data)), [error, data])
  // Null when the server predates the summary; callers fall back to deriving one from events.
  const summary = isLive ? liveSummary : rangeSummary
  const isLoading = isLive ? liveLoading : rangeLoading
  const isUnavailable = isLive ? liveUnavailable : Boolean(error)

  const unseenEvents = useMemo(
    () => events.filter((event) => !isSeen(event.id)),
    [events, isSeen],
  )

  // "Mark all read" applies to whatever this consumer is showing, but writes to
  // the shared seen-state so the badge clears everywhere at once.
  const markAllSeen = useCallback(() => {
    markSeen(events)
  }, [events, markSeen])

  const refresh = useCallback(() => {
    if (isLive) {
      refreshLive()
    } else {
      void mutate()
    }
  }, [isLive, refreshLive, mutate])

  return {
    events,
    summary,
    unseenEvents,
    unseenCount: unseenEvents.length,
    markAllSeen,
    refresh,
    isLoading,
    isUnavailable,
  }
}

export function describeRoutingEvent(event: RoutingEvent): string {
  switch (event.event_type) {
    case 'leader_changed':
      return event.previous_gateway
        ? `${event.gateway} took over from ${event.previous_gateway}`
        : `${event.gateway} took the lead`
    case 'gateway_entered_auth_band':
      // Score is now within tolerance of the top PSP, so the engine can route to
      // it for cost savings despite a slightly lower success rate.
      return `${event.gateway} now good enough on success — routing it to save cost`
    case 'gateway_exited_auth_band':
      return `${event.gateway} slipped on success — not eligible for cost override anymore`
    case 'calibration_applied':
      return describeCalibration(event)
  }
}

// An autopilot retune in plain words: the cluster it applies to and the settings that moved.
function describeCalibration(event: RoutingEvent): string {
  return `Autopilot retuned ${calibrationClusterLabel(event)}: ${describeCalibrationChange(event)}`
}
