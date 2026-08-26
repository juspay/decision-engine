import { RoutingAlgorithm } from '../types/api'

/** Trailing `Z`, `+05:30`, or `-0800`. */
const HAS_TIMEZONE = /(Z|[+-]\d{2}:?\d{2})$/i

/**
 * The routing API builds its stamps from `OffsetDateTime::now_utc()` but stores them as
 * `PrimitiveDateTime`, so they serialize as `2026-08-26T09:19:00.123456789` — a UTC instant with
 * its offset stripped. ECMAScript reads a date-time with no offset as *local* time, which shows a
 * UTC 09:19 as 9:19 AM in Kolkata instead of 2:49 PM. Mark it as UTC before parsing.
 */
export function parseBackendTimestamp(value: string): number {
  const trimmed = value.trim()
  if (!trimmed) return 0

  // `Date` only guarantees millisecond precision; nanoseconds from `time` are trimmed off.
  const normalized = trimmed.replace(/(\.\d{3})\d+/, '$1')
  const ms = new Date(HAS_TIMEZONE.test(normalized) ? normalized : `${normalized}Z`).getTime()
  return Number.isFinite(ms) ? ms : 0
}

/**
 * When a rule last changed. The backend only sets `modified_at` once a rule has been edited, so a
 * rule that has never been touched since creation falls back to `created_at`.
 */
export function lastModifiedMs(algo: RoutingAlgorithm): number {
  const stamp = algo.modified_at || algo.created_at
  return stamp ? parseBackendTimestamp(stamp) : 0
}

/** `Aug 26, 2026` and `2:41 PM`, split so the table column stays narrow. */
export function formatLastModified(algo: RoutingAlgorithm) {
  const ms = lastModifiedMs(algo)
  if (!ms) return null

  const date = new Date(ms)
  return {
    date: date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }),
    time: date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
    full: date.toLocaleString(undefined, { dateStyle: 'full', timeStyle: 'medium' }),
  }
}
