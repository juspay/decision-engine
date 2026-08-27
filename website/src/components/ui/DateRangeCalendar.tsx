import { useEffect, useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Button } from './Button'
import { customWindowFrom, fromDateTimeInputValue, toDateTimeInputValue } from '../../lib/timeRange'

type DateRangeCalendarProps = {
  /** datetime-local strings — the currently applied window. */
  start: string
  end: string
  /** Both ends are handed back together, so a window never applies half-changed. */
  onApply: (start: string, end: string) => void
  onCancel?: () => void
}

type CalendarCell = {
  key: string
  date: Date
  inMonth: boolean
}

type Draft = {
  start: Date
  /** null while the user has picked an anchor day and not yet closed the range. */
  end: Date | null
}

function pad(value: number) {
  return value.toString().padStart(2, '0')
}

function startOfDay(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate())
}

function startOfMonth(date: Date) {
  return new Date(date.getFullYear(), date.getMonth(), 1)
}

function clampToNow(date: Date) {
  const now = new Date()
  return date.getTime() > now.getTime() ? now : date
}

function monthLabel(date: Date) {
  return new Intl.DateTimeFormat(undefined, { month: 'long', year: 'numeric' }).format(date)
}

function buildCalendar(viewDate: Date): CalendarCell[] {
  const firstOfMonth = startOfMonth(viewDate)
  const start = new Date(firstOfMonth)
  start.setDate(start.getDate() - firstOfMonth.getDay())

  const cells: CalendarCell[] = Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start)
    date.setDate(start.getDate() + index)
    return {
      key: `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`,
      date,
      inMonth: date.getMonth() === viewDate.getMonth(),
    }
  })

  const lastRow = cells.slice(35)
  return lastRow.every((cell) => !cell.inMonth) ? cells.slice(0, 35) : cells
}

/** Moves `day`'s date onto `time`'s clock, so picking a day never discards the chosen time. */
function withTimeOf(day: Date, time: Date) {
  const next = new Date(day)
  next.setHours(time.getHours(), time.getMinutes(), 0, 0)
  return next
}

function parseOr(value: string, fallback: Date) {
  const parsed = fromDateTimeInputValue(value)
  return parsed === null ? fallback : clampToNow(new Date(parsed))
}

/**
 * One calendar that captures both ends of a window: the first click drops the anchor, the second
 * closes the range, and the grid previews the span under the cursor in between. Clicking a day
 * earlier than the anchor restarts from that day rather than producing a reversed window.
 */
export function DateRangeCalendar({ start, end, onApply, onCancel }: DateRangeCalendarProps) {
  const now = new Date()
  const initialEnd = useMemo(() => parseOr(end, new Date()), [end])
  const initialStart = useMemo(
    () => parseOr(start, new Date(initialEnd.getTime() - 24 * 60 * 60 * 1000)),
    [start, initialEnd],
  )

  const [draft, setDraft] = useState<Draft>({ start: initialStart, end: initialEnd })
  const [hoverDate, setHoverDate] = useState<Date | null>(null)
  const [viewDate, setViewDate] = useState<Date>(startOfMonth(initialEnd))

  useEffect(() => {
    setDraft({ start: initialStart, end: initialEnd })
    setViewDate(startOfMonth(initialEnd))
  }, [initialStart.getTime(), initialEnd.getTime()])

  const calendar = useMemo(() => buildCalendar(viewDate), [viewDate])
  const viewingCurrentOrFutureMonth = startOfMonth(viewDate).getTime() >= startOfMonth(now).getTime()

  // While the range is open, the hovered day stands in for the missing end so the preview reads as
  // a span rather than a lone selected day.
  const previewEnd = draft.end ?? hoverDate
  const rangeComplete = draft.end !== null

  // Same day, end time before start time: the days read as a range but the window is empty, so
  // Apply stays inert rather than handing the page bounds it cannot query with.
  const draftValue = draft.end
    ? {
        start: toDateTimeInputValue(draft.start.getTime()),
        end: toDateTimeInputValue(draft.end.getTime()),
      }
    : null
  const draftWindow = draftValue ? customWindowFrom(draftValue.start, draftValue.end) : undefined

  function selectDay(date: Date) {
    setDraft((current) => {
      if (current.end === null) {
        // Second click: close the range, unless it lands before the anchor.
        if (startOfDay(date).getTime() < startOfDay(current.start).getTime()) {
          return { start: clampToNow(withTimeOf(date, current.start)), end: null }
        }
        return { ...current, end: clampToNow(withTimeOf(date, initialEnd)) }
      }
      // A completed range: this click starts a new one.
      return { start: clampToNow(withTimeOf(date, current.start)), end: null }
    })
  }

  function updateTime(bound: 'start' | 'end', part: 'hours' | 'minutes', value: string) {
    setDraft((current) => {
      const target = bound === 'start' ? current.start : current.end
      if (!target) return current
      const next = new Date(target)
      if (part === 'hours') next.setHours(Number(value))
      else next.setMinutes(Number(value))
      return { ...current, [bound]: clampToNow(next) }
    })
  }

  function apply() {
    if (!draftValue) return
    onApply(draftValue.start, draftValue.end)
  }

  function cellState(date: Date) {
    const day = startOfDay(date).getTime()
    const anchor = startOfDay(draft.start).getTime()
    const closing = previewEnd ? startOfDay(previewEnd).getTime() : null

    if (day === anchor) return 'edge'
    if (closing !== null && day === closing) return 'edge'
    if (closing !== null && day > anchor && day < closing) return 'inside'
    return 'idle'
  }

  return (
    <div className="w-[268px]">
      <div className="mb-2 flex items-center justify-between gap-2">
        <p className="text-[12px] font-semibold leading-4 text-slate-900 dark:text-white">
          {monthLabel(viewDate)}
        </p>
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => setViewDate((c) => new Date(c.getFullYear(), c.getMonth() - 1, 1))}
            aria-label="Previous month"
            className="flex h-6 w-6 items-center justify-center rounded-lg border border-slate-200 text-slate-500 transition hover:border-slate-300 hover:text-slate-900 dark:border-[#2a303a] dark:text-[#8a8a93] dark:hover:text-white"
          >
            <ChevronLeft size={12} />
          </button>
          <button
            type="button"
            disabled={viewingCurrentOrFutureMonth}
            onClick={() => setViewDate((c) => new Date(c.getFullYear(), c.getMonth() + 1, 1))}
            aria-label="Next month"
            className="flex h-6 w-6 items-center justify-center rounded-lg border border-slate-200 text-slate-500 transition hover:border-slate-300 hover:text-slate-900 disabled:cursor-not-allowed disabled:opacity-35 disabled:hover:border-slate-200 disabled:hover:text-slate-500 dark:border-[#2a303a] dark:text-[#8a8a93] dark:hover:text-white dark:disabled:hover:text-[#8a8a93]"
          >
            <ChevronRight size={12} />
          </button>
        </div>
      </div>

      <div className="grid grid-cols-7 text-center text-[11px] font-semibold uppercase leading-4 tracking-[0.1em] text-slate-500 dark:text-[#78849a]">
        {['S', 'M', 'T', 'W', 'T', 'F', 'S'].map((day, index) => (
          <span key={index} className="py-0.5">
            {day}
          </span>
        ))}
      </div>

      <div className="grid grid-cols-7" onMouseLeave={() => setHoverDate(null)}>
        {calendar.map((cell) => {
          const future = startOfDay(cell.date).getTime() > startOfDay(now).getTime()
          const state = future ? 'idle' : cellState(cell.date)
          return (
            <div
              key={cell.key}
              className={`py-px ${
                state === 'inside'
                  ? 'bg-brand-500/10 dark:bg-brand-500/15'
                  : state === 'edge'
                    ? 'bg-brand-500/10 first:rounded-l-md last:rounded-r-md dark:bg-brand-500/15'
                    : ''
              }`}
            >
              <button
                type="button"
                disabled={future}
                onClick={() => selectDay(cell.date)}
                onMouseEnter={() => setHoverDate(rangeComplete ? null : cell.date)}
                className={`flex h-7 w-full items-center justify-center rounded-md text-[12px] leading-4 transition ${
                  future
                    ? 'cursor-not-allowed text-slate-500 opacity-35 dark:text-[#78849a]'
                    : state === 'edge'
                      ? 'bg-brand-600 text-white shadow-[0_8px_20px_-14px_rgba(59,130,246,0.7)] dark:bg-brand-500'
                      : state === 'inside'
                        ? 'text-brand-700 dark:text-brand-200'
                        : cell.inMonth
                          ? 'text-slate-700 hover:bg-slate-100 dark:text-[#e5ecf7] dark:hover:bg-[#1a2130]'
                          : 'text-slate-500 hover:bg-slate-100 dark:text-[#4b5565] dark:hover:bg-[#161b24]'
                }`}
              >
                {cell.date.getDate()}
              </button>
            </div>
          )
        })}
      </div>

      <div className="mt-2 space-y-1.5 border-t border-slate-100 pt-2 dark:border-[#2a303a]">
        {(['start', 'end'] as const).map((bound) => {
          const value = bound === 'start' ? draft.start : draft.end
          return (
            <div key={bound} className="flex items-center gap-1.5">
              <span className="w-9 text-[11px] font-medium leading-4 text-slate-500 dark:text-[#8a8a93]">
                {bound === 'start' ? 'From' : 'To'}
              </span>
              <TimeSelect
                unit="hours"
                value={value}
                disabled={!value}
                onChange={(next) => updateTime(bound, 'hours', next)}
              />
              <span className="text-[11px] font-semibold leading-4 text-slate-500 dark:text-[#8a8a93]">
                :
              </span>
              <TimeSelect
                unit="minutes"
                value={value}
                disabled={!value}
                onChange={(next) => updateTime(bound, 'minutes', next)}
              />
              <span className="ml-auto truncate text-[11px] leading-4 text-slate-500 dark:text-[#8a8a93]">
                {value
                  ? new Intl.DateTimeFormat(undefined, { day: '2-digit', month: 'short' }).format(value)
                  : 'Pick a day'}
              </span>
            </div>
          )
        })}
      </div>

      {draftValue && !draftWindow ? (
        <p className="mt-2 text-[11px] leading-4 text-red-600 dark:text-red-400">
          The end must come after the start, and neither may be in the future.
        </p>
      ) : null}

      <div className="mt-2.5 flex items-center justify-end gap-1.5">
        {onCancel ? (
          <Button size="sm" variant="secondary" onClick={onCancel}>
            Cancel
          </Button>
        ) : null}
        <Button size="sm" onClick={apply} disabled={!draftWindow}>
          Apply
        </Button>
      </div>
    </div>
  )
}

function TimeSelect({
  unit,
  value,
  disabled,
  onChange,
}: {
  unit: 'hours' | 'minutes'
  value: Date | null
  disabled: boolean
  onChange: (value: string) => void
}) {
  const length = unit === 'hours' ? 24 : 60
  const current = value ? (unit === 'hours' ? value.getHours() : value.getMinutes()) : 0
  return (
    <select
      value={pad(current)}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
      aria-label={unit === 'hours' ? 'Hour' : 'Minute'}
      className="h-7 w-[52px] rounded-lg border border-slate-200 bg-white/90 px-1.5 text-[12px] leading-4 text-slate-700 disabled:opacity-40 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7]"
    >
      {Array.from({ length }, (_, index) => (
        <option key={index} value={pad(index)}>
          {pad(index)}
        </option>
      ))}
    </select>
  )
}
