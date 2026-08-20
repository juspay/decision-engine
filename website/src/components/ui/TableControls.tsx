import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown, Search, Check, X, MoreVertical, type LucideIcon } from 'lucide-react'

/**
 * A column header that doubles as its own filter. `all` is the unfiltered state; any other value
 * renders in the header itself so a narrowed table always says why it is short.
 */
export function HeaderFilter({
  label,
  value,
  options,
  onChange,
  ariaLabel,
}: {
  label: string
  value: string
  options: { value: string; label: string }[]
  onChange: (value: string) => void
  ariaLabel: string
}) {
  const [open, setOpen] = useState(false)
  const isFiltered = value !== 'all'
  const current = options.find((o) => o.value === value)

  return (
    <div className="relative inline-flex">
      <button
        type="button"
        aria-label={ariaLabel}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className={`inline-flex items-center gap-1 rounded uppercase tracking-wide transition-colors ${
          isFiltered
            ? 'text-brand-600 dark:text-brand-400'
            : 'text-slate-400 hover:text-slate-600 dark:text-[#4e5870] dark:hover:text-[#8d96a8]'
        }`}
      >
        {isFiltered ? `${label}: ${current?.label ?? value}` : label}
        <ChevronDown size={13} className="shrink-0" />
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute left-0 top-full z-20 mt-1.5 max-h-64 min-w-[170px] overflow-y-auto rounded-lg border border-slate-200 bg-white py-1 shadow-lg dark:border-[#2a303a] dark:bg-[#11151d]">
            {options.map((o) => (
              <button
                key={o.value}
                type="button"
                onClick={() => { onChange(o.value); setOpen(false) }}
                className={`flex w-full items-center justify-between gap-2 px-3 py-1.5 text-left text-xs normal-case tracking-normal transition-colors hover:bg-slate-50 dark:hover:bg-[#1c2030] ${
                  o.value === value
                    ? 'font-semibold text-brand-600 dark:text-brand-400'
                    : 'text-slate-600 dark:text-slate-300'
                }`}
              >
                <span className="truncate">{o.label}</span>
                {o.value === value && <Check size={12} className="shrink-0" />}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

/** The name column's header, which swaps its label for a filter input on demand. */
export function HeaderSearch({
  label,
  value,
  onChange,
  ariaLabel,
}: {
  label: string
  value: string
  onChange: (value: string) => void
  ariaLabel: string
}) {
  const [open, setOpen] = useState(false)

  if (!open && !value) {
    return (
      <button
        type="button"
        aria-label={ariaLabel}
        onClick={() => setOpen(true)}
        className="inline-flex items-center gap-1 uppercase tracking-wide text-slate-400 transition-colors hover:text-slate-600 dark:text-[#4e5870] dark:hover:text-[#8d96a8]"
      >
        {label}
        <Search size={13} className="shrink-0" />
      </button>
    )
  }

  return (
    // The input itself is the field: the global input styling in index.css already gives it a
    // border, radius and focus ring, so a bordered wrapper would draw a second outline around it.
    // The icons sit on top of the single input instead.
    <div className="relative w-[240px] max-w-full">
      <Search
        size={14}
        className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-slate-400"
      />
      <input
        autoFocus
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Filter by name or ID"
        aria-label={ariaLabel}
        className="w-full rounded-lg border border-slate-200 bg-white py-1.5 pl-8 pr-8 text-sm font-normal normal-case tracking-normal text-slate-800 placeholder:text-slate-400 focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#0d1018] dark:text-slate-100 dark:placeholder:text-[#5b6577]"
      />
      <button
        type="button"
        aria-label="Clear name filter"
        onClick={() => { onChange(''); setOpen(false) }}
        className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 transition-colors hover:text-slate-700 dark:hover:text-slate-200"
      >
        <X size={14} />
      </button>
    </div>
  )
}

export type RowMenuItem = {
  label: string
  icon: LucideIcon
  onSelect: () => void
  disabled?: boolean
  /** Why the item is unavailable — shown in place of the label's hover title. */
  hint?: string
  tone?: 'default' | 'positive' | 'danger'
}

/**
 * Per-row action menu.
 *
 * The panel is portalled and positioned from the trigger's rect rather than absolutely placed:
 * the table lives in an `overflow-x-auto` wrapper, and once one axis is not `visible` the other
 * computes to `auto`, so an in-flow dropdown would be clipped by the scroll container.
 */
export function RowMenu({ items }: { items: RowMenuItem[] }) {
  const [open, setOpen] = useState(false)
  const [style, setStyle] = useState<React.CSSProperties>({})
  const triggerRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    function onOutside(e: MouseEvent) {
      const target = e.target as Node
      if (!triggerRef.current?.contains(target) && !panelRef.current?.contains(target)) setOpen(false)
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setOpen(false)
    }
    // A menu anchored to a rect goes stale the moment anything moves underneath it.
    function onReflow() { setOpen(false) }
    document.addEventListener('mousedown', onOutside)
    document.addEventListener('keydown', onKey)
    window.addEventListener('resize', onReflow)
    window.addEventListener('scroll', onReflow, true)
    return () => {
      document.removeEventListener('mousedown', onOutside)
      document.removeEventListener('keydown', onKey)
      window.removeEventListener('resize', onReflow)
      window.removeEventListener('scroll', onReflow, true)
    }
  }, [open])

  function toggle() {
    if (open) { setOpen(false); return }
    const rect = triggerRef.current?.getBoundingClientRect()
    if (!rect) return
    const width = 190
    const estimatedHeight = items.length * 38 + 8
    // Flip above the trigger when there is not room below it.
    const flip = rect.bottom + estimatedHeight > window.innerHeight - 8
    setStyle({
      position: 'fixed',
      top: flip ? Math.max(8, rect.top - estimatedHeight - 4) : rect.bottom + 4,
      left: Math.max(8, rect.right - width),
      width,
      zIndex: 9999,
    })
    setOpen(true)
  }

  const toneClasses: Record<string, string> = {
    default: 'text-slate-700 dark:text-slate-300',
    positive: 'text-emerald-600 dark:text-emerald-400',
    danger: 'text-red-600 dark:text-red-400',
  }

  return (
    <div className="flex justify-end">
      <button
        ref={triggerRef}
        type="button"
        onClick={toggle}
        aria-label="Rule actions"
        aria-haspopup="menu"
        aria-expanded={open}
        className={`rounded-md p-1.5 transition-colors ${
          open
            ? 'bg-slate-100 text-slate-700 dark:bg-[#1c2030] dark:text-white'
            : 'text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-[#1c2030] dark:hover:text-white'
        }`}
      >
        <MoreVertical size={16} />
      </button>

      {open && createPortal(
        <div
          ref={panelRef}
          role="menu"
          style={style}
          className="overflow-hidden rounded-lg border border-slate-200 bg-white py-1 shadow-lg dark:border-[#2a303a] dark:bg-[#11151d]"
        >
          {items.map((item) => {
            const Icon = item.icon
            return (
              <button
                key={item.label}
                type="button"
                role="menuitem"
                disabled={item.disabled}
                title={item.disabled ? item.hint : undefined}
                onClick={() => { setOpen(false); item.onSelect() }}
                className={`flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13px] font-medium transition-colors hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent dark:hover:bg-[#1c2030] ${
                  toneClasses[item.tone ?? 'default']
                }`}
              >
                <Icon size={14} className="shrink-0" />
                {item.label}
              </button>
            )
          })}
        </div>,
        document.body
      )}
    </div>
  )
}
