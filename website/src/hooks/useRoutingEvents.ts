import { useCallback, useEffect, useState } from 'react'
import useSWR from 'swr'
import { fetcher } from '../lib/api'
import { useAuthStore } from '../store/authStore'
import { AnalyticsRangeValue, RoutingEvent, RoutingEventsResponse } from '../types/api'

const POLL_INTERVAL_MS = 15_000
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

export function useRoutingEvents(
  range: AnalyticsRangeValue = '12h',
  // Override the poll cadence — e.g. tighten it while a simulation is actively
  // producing events so the Autopilot feed keeps up instead of lagging by a
  // full poll interval. Falls back to the idle default.
  refreshInterval: number = POLL_INTERVAL_MS,
) {
  const merchantId = useAuthStore((state) => state.user?.merchantId) ?? ''
  // Short windows get second-granularity detection (catches crossings inside a
  // burst); longer windows use minute buckets to keep the scan bounded.
  const bucket = range === '15m' || range === '1h' ? '1s' : '1m'
  const { data, error, isLoading, mutate } = useSWR<RoutingEventsResponse>(
    merchantId ? `/analytics/routing-events?range=${range}&bucket=${bucket}` : null,
    fetcher,
    { refreshInterval, revalidateOnFocus: false },
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

  // Force an immediate re-fetch (bypassing the poll timer) — used to pull fresh
  // events the moment a simulation batch lands.
  const refresh = useCallback(() => {
    void mutate()
  }, [mutate])

  return {
    events,
    unseenEvents,
    unseenCount: unseenEvents.length,
    markAllSeen,
    refresh,
    isLoading,
    isUnavailable: Boolean(error),
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
