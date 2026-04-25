import { useEffect, useMemo, useRef, useState } from 'react'
import { CalendarDays, ChevronLeft, ChevronRight, Clock3 } from 'lucide-react'
import { Button } from './Button'

type DateTimePickerProps = {
  value: string
  onChange: (value: string) => void
  className?: string
}

type CalendarCell = {
  key: string
  date: Date
  inMonth: boolean
}

function pad(value: number) {
  return value.toString().padStart(2, '0')
}

function toDateTimeInputValue(date: Date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}

function parseDateTimeInputValue(value: string) {
  const parsed = new Date(value)
  return Number.isFinite(parsed.getTime()) ? parsed : null
}

function formatDisplayValue(date: Date | null) {
  if (!date) return 'Select date and time'
  return new Intl.DateTimeFormat(undefined, {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

function monthLabel(date: Date) {
  return new Intl.DateTimeFormat(undefined, {
    month: 'long',
    year: 'numeric',
  }).format(date)
}

function sameDay(left: Date, right: Date) {
  return (
    left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate()
  )
}

function buildCalendar(viewDate: Date): CalendarCell[] {
  const startOfMonth = new Date(viewDate.getFullYear(), viewDate.getMonth(), 1)
  const startOffset = startOfMonth.getDay()
  const start = new Date(startOfMonth)
  start.setDate(start.getDate() - startOffset)

  return Array.from({ length: 42 }, (_, index) => {
    const date = new Date(start)
    date.setDate(start.getDate() + index)
    return {
      key: `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`,
      date,
      inMonth: date.getMonth() === viewDate.getMonth(),
    }
  })
}

export function DateTimePicker({ value, onChange, className = '' }: DateTimePickerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const parsedValue = useMemo(() => parseDateTimeInputValue(value), [value])
  const [open, setOpen] = useState(false)
  const [draftDate, setDraftDate] = useState<Date>(parsedValue || new Date())
  const [viewDate, setViewDate] = useState<Date>(parsedValue || new Date())

  useEffect(() => {
    if (parsedValue) {
      setDraftDate(parsedValue)
      setViewDate(new Date(parsedValue.getFullYear(), parsedValue.getMonth(), 1))
    }
  }, [parsedValue?.getTime()])

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: MouseEvent) {
      if (!containerRef.current?.contains(event.target as Node)) {
        setOpen(false)
      }
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }

    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [open])

  const calendar = useMemo(() => buildCalendar(viewDate), [viewDate])

  function selectDay(date: Date) {
    setDraftDate((current) => {
      const next = new Date(date)
      next.setHours(current.getHours(), current.getMinutes(), 0, 0)
      return next
    })
  }

  function updateTime(part: 'hours' | 'minutes', nextValue: string) {
    setDraftDate((current) => {
      const next = new Date(current)
      if (part === 'hours') next.setHours(Number(nextValue))
      else next.setMinutes(Number(nextValue))
      return next
    })
  }

  function applyDraft() {
    onChange(toDateTimeInputValue(draftDate))
    setOpen(false)
  }

  function useNow() {
    const now = new Date()
    setDraftDate(now)
    setViewDate(new Date(now.getFullYear(), now.getMonth(), 1))
  }

  return (
    <div ref={containerRef} className={`relative ${className}`}>
      <button
        type="button"
        onClick={() => {
          if (!open) {
            const next = parsedValue || new Date()
            setDraftDate(next)
            setViewDate(new Date(next.getFullYear(), next.getMonth(), 1))
          }
          setOpen((current) => !current)
        }}
        className="flex h-11 w-full items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-white/90 px-4 text-left text-sm text-slate-700 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.2)] transition focus:outline-none focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5ecf7] dark:shadow-none"
      >
        <span className="truncate">{formatDisplayValue(parsedValue)}</span>
        <CalendarDays size={16} className="shrink-0 text-slate-400 dark:text-[#8a8a93]" />
      </button>

      {open ? (
        <div className="absolute left-0 top-[calc(100%+10px)] z-[80] w-[284px] rounded-[24px] border border-slate-200 bg-white/95 p-3 shadow-[0_24px_70px_-40px_rgba(15,23,42,0.45)] backdrop-blur dark:border-[#2a303a] dark:bg-[#11151d]/95 dark:shadow-[0_24px_70px_-40px_rgba(0,0,0,0.7)]">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-[13px] font-semibold text-slate-900 dark:text-white">{monthLabel(viewDate)}</p>
              <p className="mt-1 text-[11px] text-slate-500 dark:text-[#8a8a93]">Choose a day and time</p>
            </div>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={() => setViewDate((current) => new Date(current.getFullYear(), current.getMonth() - 1, 1))}
                className="flex h-7 w-7 items-center justify-center rounded-full border border-slate-200 text-slate-500 transition hover:border-slate-300 hover:text-slate-900 dark:border-[#2a303a] dark:text-[#8a8a93] dark:hover:text-white"
              >
                <ChevronLeft size={14} />
              </button>
              <button
                type="button"
                onClick={() => setViewDate((current) => new Date(current.getFullYear(), current.getMonth() + 1, 1))}
                className="flex h-7 w-7 items-center justify-center rounded-full border border-slate-200 text-slate-500 transition hover:border-slate-300 hover:text-slate-900 dark:border-[#2a303a] dark:text-[#8a8a93] dark:hover:text-white"
              >
                <ChevronRight size={14} />
              </button>
            </div>
          </div>

          <div className="mt-3 grid grid-cols-7 gap-1 text-center text-[10px] font-semibold uppercase tracking-[0.14em] text-slate-400 dark:text-[#667085]">
            {['S', 'M', 'T', 'W', 'T', 'F', 'S'].map((day) => (
              <span key={day} className="py-2">
                {day}
              </span>
            ))}
          </div>

          <div className="grid grid-cols-7 gap-1">
            {calendar.map((cell) => {
              const selected = sameDay(cell.date, draftDate)
              return (
                <button
                  key={cell.key}
                  type="button"
                  onClick={() => selectDay(cell.date)}
                  className={`flex h-9 items-center justify-center rounded-lg text-[13px] transition ${
                    selected
                      ? 'bg-brand-600 text-white shadow-[0_12px_30px_-22px_rgba(59,130,246,0.7)] dark:bg-brand-500 dark:text-white'
                      : cell.inMonth
                        ? 'text-slate-700 hover:bg-slate-100 dark:text-[#e5ecf7] dark:hover:bg-[#1a2130]'
                        : 'text-slate-300 hover:bg-slate-100 dark:text-[#4b5565] dark:hover:bg-[#161b24]'
                  }`}
                >
                  {cell.date.getDate()}
                </button>
              )
            })}
          </div>

          <div className="mt-3 rounded-[18px] border border-slate-200 bg-slate-50/70 p-3 dark:border-[#2a303a] dark:bg-[#161b24]">
            <div className="mb-2 flex items-center gap-2">
              <Clock3 size={13} className="text-slate-400 dark:text-[#8a8a93]" />
              <p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-slate-500 dark:text-[#8a8a93]">
                Time
              </p>
            </div>

            <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2">
              <select
                value={pad(draftDate.getHours())}
                onChange={(event) => updateTime('hours', event.target.value)}
                className="h-9 rounded-xl border border-slate-200 bg-white/90 px-3 text-sm text-slate-700 dark:border-[#2a303a] dark:bg-[#11151d] dark:text-[#e5ecf7]"
              >
                {Array.from({ length: 24 }, (_, index) => pad(index)).map((hour) => (
                  <option key={hour} value={hour}>
                    {hour}
                  </option>
                ))}
              </select>
              <span className="text-sm font-semibold text-slate-400 dark:text-[#8a8a93]">:</span>
              <select
                value={pad(draftDate.getMinutes())}
                onChange={(event) => updateTime('minutes', event.target.value)}
                className="h-9 rounded-xl border border-slate-200 bg-white/90 px-3 text-sm text-slate-700 dark:border-[#2a303a] dark:bg-[#11151d] dark:text-[#e5ecf7]"
              >
                {Array.from({ length: 60 }, (_, index) => pad(index)).map((minute) => (
                  <option key={minute} value={minute}>
                    {minute}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="mt-3 flex items-center justify-between gap-2">
            <Button size="sm" variant="ghost" onClick={useNow}>
              Now
            </Button>
            <div className="flex items-center gap-2">
              <Button size="sm" variant="secondary" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button size="sm" onClick={applyDraft}>
                Apply
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  )
}
