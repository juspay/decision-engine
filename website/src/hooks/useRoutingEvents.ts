import { useCallback, useEffect, useState } from 'react'
import useSWR from 'swr'
import { fetcher } from '../lib/api'
import { useAuthStore } from '../store/authStore'
import { AnalyticsRangeValue, RoutingEvent, RoutingEventsResponse } from '../types/api'

const POLL_INTERVAL_MS = 30_000
const SEEN_STORAGE_PREFIX = 'routing_events_seen'
const MAX_SEEN_IDS = 500

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

export function useRoutingEvents(range: AnalyticsRangeValue = '12h') {
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  // Short windows get second-granularity detection (catches crossings inside a
  // burst); longer windows use minute buckets to keep the scan bounded.
  const bucket = range === '15m' || range === '1h' ? '1s' : '1m'
  const { data, error, isLoading } = useSWR<RoutingEventsResponse>(
    merchantId ? `/analytics/routing-events?range=${range}&bucket=${bucket}` : null,
    fetcher,
    { refreshInterval: POLL_INTERVAL_MS, revalidateOnFocus: false },
  )

  const [seenIds, setSeenIds] = useState<Set<string>>(() => loadSeenIds(merchantId))

  useEffect(() => {
    setSeenIds(loadSeenIds(merchantId))
  }, [merchantId])

  // Degrade silently when analytics is unavailable (e.g. ClickHouse disabled).
  const events: RoutingEvent[] = error ? [] : data?.events ?? []
  const unseenEvents = events.filter((event) => !seenIds.has(event.id))

  const markAllSeen = useCallback(() => {
    setSeenIds((previous) => {
      const next = new Set(previous)
      for (const event of events) next.add(event.id)
      persistSeenIds(merchantId, next)
      return next
    })
  }, [events, merchantId])

  return {
    events,
    unseenEvents,
    unseenCount: unseenEvents.length,
    markAllSeen,
    isLoading,
    isUnavailable: Boolean(error),
  }
}

// SR scores are success rates on a 0..1 scale; show them as percentages.
function formatScore(value: number | null): string {
  if (value == null) return '–'
  return `${(value * 100).toFixed(1)}%`
}

export function describeRoutingEvent(event: RoutingEvent): string {
  const dimension = [event.payment_method_type, event.payment_method]
    .filter(Boolean)
    .join('/')
  const scope = dimension ? ` for ${dimension}` : ''
  const score = formatScore(event.score)
  const previousScore = formatScore(event.previous_score)

  switch (event.event_type) {
    case 'leader_changed':
      return `${event.gateway} overtook ${event.previous_gateway ?? 'previous leader'}${scope} — success rate ${score} vs ${previousScore}`
    case 'gateway_entered_auth_band':
      return `${event.gateway} entered the auth band of ${event.previous_gateway ?? 'the leader'}${scope} — success rate ${score} vs leader ${previousScore}`
    case 'gateway_exited_auth_band':
      return `${event.gateway} dropped out of the auth band of ${event.previous_gateway ?? 'the leader'}${scope} — success rate ${score} vs leader ${previousScore}`
  }
}
