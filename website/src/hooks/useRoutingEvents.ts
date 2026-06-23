import {
  ReactNode,
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react'
import useSWR from 'swr'
import { fetcher } from '../lib/api'
import { useAuthStore } from '../store/authStore'
import { AnalyticsRangeValue, RoutingEvent, RoutingEventsResponse } from '../types/api'

const POLL_INTERVAL_MS = 15_000
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

function routingEventsKey(merchantId: string, range: AnalyticsRangeValue): string | null {
  if (!merchantId) return null
  return `/analytics/routing-events?range=${range}&bucket=${bucketForRange(range)}`
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
  isLoading: boolean
  isUnavailable: boolean
  isSeen: (id: string) => boolean
  markSeen: (events: RoutingEvent[]) => void
  refresh: () => void
  // Let a single consumer (the simulator) tighten the shared poll while it is
  // actively producing events; pass null to release back to the idle default.
  requestPollInterval: (intervalMs: number | null) => void
}

const RoutingEventsContext = createContext<RoutingEventsContextValue | null>(null)

// Owns the one live-feed poll and the merchant-scoped seen-state. Mount once,
// above the notification bell and the routed pages, so every consumer of
// `useRoutingEvents` shares a single network poll and one consistent seen-set.
export function RoutingEventsProvider({ children }: { children: ReactNode }) {
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  const [pollIntervalMs, setPollIntervalMs] = useState(POLL_INTERVAL_MS)

  const { data, error, isLoading, mutate } = useSWR<RoutingEventsResponse>(
    routingEventsKey(merchantId, LIVE_RANGE),
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

  // Degrade silently when analytics is unavailable (e.g. ClickHouse disabled).
  const liveEvents = useMemo<RoutingEvent[]>(() => (error ? [] : data?.events ?? []), [error, data])

  const value = useMemo<RoutingEventsContextValue>(
    () => ({
      liveEvents,
      isLoading,
      isUnavailable: Boolean(error),
      isSeen,
      markSeen,
      refresh,
      requestPollInterval,
    }),
    [liveEvents, isLoading, error, isSeen, markSeen, refresh, requestPollInterval],
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

export function useRoutingEvents(
  range: AnalyticsRangeValue = LIVE_RANGE,
  // Override the shared poll cadence — e.g. tighten it while a simulation is
  // actively producing events so the Autopilot feed keeps up. Omit to use the
  // idle default. Only honored for the live feed (the bell + simulator share it).
  refreshInterval?: number,
) {
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  const {
    liveEvents,
    isLoading: liveLoading,
    isUnavailable: liveUnavailable,
    isSeen,
    markSeen,
    refresh: refreshLive,
    requestPollInterval,
  } = useRoutingEventsContext()

  const isLive = range === LIVE_RANGE

  // Live consumers share the provider's single poll; a caller that passes an
  // explicit cadence retunes it, then releases on unmount/stop so the bell's
  // idle 15s cadence is restored.
  useEffect(() => {
    if (!isLive || refreshInterval === undefined) return
    requestPollInterval(refreshInterval)
    return () => requestPollInterval(null)
  }, [isLive, refreshInterval, requestPollInterval])

  // Wider, user-driven windows (the events page) open their own subscription;
  // the live feed already covers LIVE_RANGE for the bell + simulator.
  const { data, error, isLoading: rangeLoading, mutate } = useSWR<RoutingEventsResponse>(
    isLive ? null : routingEventsKey(merchantId, range),
    fetcher,
    { refreshInterval: refreshInterval ?? POLL_INTERVAL_MS, revalidateOnFocus: false },
  )

  const events = isLive ? liveEvents : error ? [] : data?.events ?? []
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
      return `switching psp to ${event.gateway}`
    case 'gateway_entered_auth_band':
      // Score is now within tolerance of the top PSP, so the engine can route to
      // it for cost savings despite a slightly lower success rate.
      return `${event.gateway} now good enough on success — routing it to save cost`
    case 'gateway_exited_auth_band':
      return `${event.gateway} slipped on success — not eligible for cost override anymore`
  }
}
