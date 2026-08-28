import { useEffect, useRef, useState } from 'react'
import { AnalyticsRangeValue } from '../../types/api'
import { Button } from './Button'
import { DateRangeCalendar } from './DateRangeCalendar'
import { RANGE_OPTIONS, customWindowFrom, formatWindowLabel } from '../../lib/timeRange'

type TimeRangeFilterProps = {
  range: AnalyticsRangeValue
  /** The custom window's ends, as datetime-local strings. */
  customStart: string
  customEnd: string
  onRangeChange: (range: AnalyticsRangeValue) => void
  /** Fired once, with both ends, when a custom window is applied. */
  onCustomChange: (start: string, end: string) => void
  /** Which edge the custom popover hangs from — set to `left` when the control sits at page start. */
  align?: 'left' | 'right'
  className?: string
}

/**
 * The preset pills plus the custom-window popover, shared by every page that scopes its data to a
 * time window, so the ranges offered and the way a custom window is picked stay the same throughout.
 */
export function TimeRangeFilter({
  range,
  customStart,
  customEnd,
  onRangeChange,
  onCustomChange,
  align = 'right',
  className = '',
}: TimeRangeFilterProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: MouseEvent) {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false)
    }
    function handleEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false)
    }

    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [open])

  // A page can drop out of custom on its own (a URL change, a reset); the popover follows.
  useEffect(() => {
    if (range !== 'custom') setOpen(false)
  }, [range])

  const customWindow = range === 'custom' ? customWindowFrom(customStart, customEnd) : undefined

  function handlePresetClick(value: AnalyticsRangeValue) {
    if (value === 'custom') {
      // Re-clicking Custom toggles the popover rather than re-selecting a range already active.
      setOpen((current) => (range === 'custom' ? !current : true))
      if (range !== 'custom') onRangeChange('custom')
      return
    }
    setOpen(false)
    onRangeChange(value)
  }

  return (
    <div ref={containerRef} className={`relative ${className}`}>
      <div className="flex flex-wrap items-center gap-1 rounded-[18px] border border-slate-200 bg-white/70 p-1 dark:border-[#2a303a] dark:bg-[#11151d]">
        {RANGE_OPTIONS.map((option) => (
          <Button
            key={option.value}
            size="sm"
            variant="secondary"
            title={option.label}
            className={pillClass(range === option.value)}
            onClick={() => handlePresetClick(option.value)}
          >
            {option.value === 'custom' ? 'Custom' : option.value}
          </Button>
        ))}
      </div>

      {range === 'custom' && open ? (
        <div
          className={`absolute top-[calc(100%+10px)] z-[90] rounded-2xl border border-slate-200 bg-white/95 p-4 shadow-[0_24px_70px_-34px_rgba(15,23,42,0.48)] backdrop-blur dark:border-[#2a303a] dark:bg-[#11151d]/95 dark:shadow-[0_24px_70px_-34px_rgba(0,0,0,0.72)] ${
            align === 'left' ? 'left-0' : 'right-0'
          }`}
        >
          <div className="mb-3">
            <p className="text-[13px] font-semibold leading-[18px] text-slate-900 dark:text-white">
              Select time range
            </p>
            <p className="mt-1 text-[12px] leading-[18px] text-slate-500 dark:text-[#8a8a93]">
              {customWindow
                ? formatWindowLabel('custom', customWindow)
                : 'Click a start day, then an end day'}
            </p>
          </div>

          <DateRangeCalendar
            start={customStart}
            end={customEnd}
            onCancel={() => setOpen(false)}
            onApply={(nextStart, nextEnd) => {
              onCustomChange(nextStart, nextEnd)
              setOpen(false)
            }}
          />
        </div>
      ) : null}
    </div>
  )
}

function pillClass(active: boolean) {
  return active
    ? '!border-brand-500/70 !bg-white !text-slate-950 shadow-[0_14px_30px_-24px_rgba(59,130,246,0.55)] dark:!border-brand-500/70 dark:!bg-[#161b24] dark:!text-white'
    : '!border-transparent !bg-slate-100 !text-slate-600 hover:!bg-slate-200 hover:!text-slate-900 dark:!bg-[#161b24] dark:!text-[#a7b2c6] dark:hover:!bg-[#1c2330] dark:hover:!text-white'
}
