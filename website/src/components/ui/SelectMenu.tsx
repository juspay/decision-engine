import { useEffect, useRef, useState } from 'react'
import { Check, ChevronDown } from 'lucide-react'

export type SelectMenuItem<Value extends string> = {
  value: Value
  label: string
  /** Hover text, when the label alone is too terse to explain the option. Defaults to the label. */
  title?: string
}

type SelectMenuProps<Value extends string> = {
  items: readonly SelectMenuItem<Value>[]
  value: Value
  onSelect: (value: Value) => void
  /** Prefixes the trigger and names the control for assistive tech, e.g. "View". */
  label: string
  className?: string
}

/**
 * A menu that picks one of `items`. Reach for it over a row of buttons when the list keeps growing:
 * the trigger is one fixed-width control whatever the list holds, so the row it sits in never has
 * to give up space for a new entry.
 */
export function SelectMenu<Value extends string>({
  items,
  value,
  onSelect,
  label,
  className = '',
}: SelectMenuProps<Value>) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: MouseEvent) {
      if (!containerRef.current?.contains(event.target as Node)) setOpen(false)
    }
    function handleEscape(event: KeyboardEvent) {
      if (event.key !== 'Escape') return
      setOpen(false)
      // Escape hands focus back to the trigger, so the control is not lost from under the keyboard.
      triggerRef.current?.focus()
    }

    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [open])

  const selected = items.find((item) => item.value === value)

  return (
    <div ref={containerRef} className={`relative shrink-0 ${className}`}>
      <button
        ref={triggerRef}
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={`${label}: ${selected?.label ?? value}`}
        onClick={() => setOpen((current) => !current)}
        className="flex h-9 max-w-full items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-sm transition-colors hover:bg-slate-50 hover:text-slate-900 focus:outline-none focus-visible:ring-2 focus-visible:ring-brand-500/50 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-[#e5e7eb] dark:hover:bg-[#1c2330]"
      >
        <span className="font-medium text-slate-500 dark:text-[#8a8a93]">{label}</span>
        <span className="truncate">{selected?.label ?? value}</span>
        <ChevronDown
          className={`h-3.5 w-3.5 shrink-0 text-slate-400 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open ? (
        <div
          role="menu"
          aria-label={label}
          className="absolute left-0 top-[calc(100%+8px)] z-[90] w-max min-w-full max-w-[320px] overflow-hidden rounded-2xl border border-slate-200 bg-white/95 p-1 shadow-[0_24px_70px_-34px_rgba(15,23,42,0.48)] backdrop-blur dark:border-[#2a303a] dark:bg-[#11151d]/95 dark:shadow-[0_24px_70px_-34px_rgba(0,0,0,0.72)]"
        >
          {items.map((item) => {
            const active = item.value === value
            return (
              <button
                key={item.value}
                type="button"
                role="menuitemradio"
                aria-checked={active}
                title={item.title ?? item.label}
                onClick={() => {
                  onSelect(item.value)
                  setOpen(false)
                }}
                className={`flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-[13px] leading-[18px] transition-colors ${
                  active
                    ? 'bg-brand-500/10 font-semibold text-slate-900 dark:text-white'
                    : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-[#a7b2c6] dark:hover:bg-[#1c2330] dark:hover:text-white'
                }`}
              >
                <Check
                  className={`h-3.5 w-3.5 shrink-0 text-brand-500 ${active ? '' : 'invisible'}`}
                />
                <span className="truncate">{item.label}</span>
              </button>
            )
          })}
        </div>
      ) : null}
    </div>
  )
}
