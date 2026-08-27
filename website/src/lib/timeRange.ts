import { AnalyticsRange, AnalyticsRangeValue } from '../types/api'

export type TimeWindow = {
  start_ms: number
  end_ms: number
}

export const PRESET_RANGES: readonly AnalyticsRange[] = ['15m', '1h', '12h', '1d', '1w']

export const RANGE_OPTIONS: { value: AnalyticsRangeValue; label: string }[] = [
  { value: '15m', label: 'Last 15 mins' },
  { value: '1h', label: 'Last 1 hour' },
  { value: '12h', label: 'Last 12 hours' },
  { value: '1d', label: 'Last 1 day' },
  { value: '1w', label: 'Last 1 week' },
  { value: 'custom', label: 'Custom window' },
]

const RANGE_DURATION_MS: Record<AnalyticsRange, number> = {
  '15m': 15 * 60 * 1000,
  '1h': 60 * 60 * 1000,
  '12h': 12 * 60 * 60 * 1000,
  '1d': 24 * 60 * 60 * 1000,
  '1w': 7 * 24 * 60 * 60 * 1000,
}

export function presetWindow(range: AnalyticsRange): TimeWindow {
  const now = Date.now()
  return { start_ms: now - RANGE_DURATION_MS[range], end_ms: now }
}

export function parseRange(value: string | null): AnalyticsRangeValue {
  if (value === 'custom') return value
  return PRESET_RANGES.includes(value as AnalyticsRange) ? (value as AnalyticsRange) : '1d'
}

export function toDateTimeInputValue(timestampMs: number) {
  const date = new Date(timestampMs)
  const pad = (value: number) => value.toString().padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

export function fromDateTimeInputValue(value: string) {
  const timestamp = new Date(value).getTime()
  return Number.isFinite(timestamp) ? timestamp : null
}

/**
 * The window a pair of datetime-local strings describes, or `undefined` when the pair is not a
 * usable window: unparseable, reversed, or reaching into the future.
 */
export function customWindowFrom(start: string, end: string): TimeWindow | undefined {
  const start_ms = fromDateTimeInputValue(start)
  const end_ms = fromDateTimeInputValue(end)
  const now = Date.now()
  if (start_ms === null || end_ms === null) return undefined
  if (end_ms <= start_ms || start_ms > now || end_ms > now) return undefined
  return { start_ms, end_ms }
}

export function formatWindowBound(timestampMs: number) {
  return new Intl.DateTimeFormat(undefined, {
    day: '2-digit',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timestampMs))
}

export function formatWindowLabel(range: AnalyticsRangeValue, customWindow?: TimeWindow) {
  if (range !== 'custom') {
    return RANGE_OPTIONS.find((option) => option.value === range)?.label || 'Selected window'
  }
  if (!customWindow) return 'Custom window'
  return `${formatWindowBound(customWindow.start_ms)} to ${formatWindowBound(customWindow.end_ms)}`
}
