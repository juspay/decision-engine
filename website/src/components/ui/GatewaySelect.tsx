import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown } from 'lucide-react'

const MAX_LIST_HEIGHT = 272
const MIN_LIST_HEIGHT = 160
const VIEWPORT_MARGIN = 12

interface GatewaySelectProps {
  value: string
  onChange: (value: string) => void
  options: string[]
  placeholder?: string
  className?: string
  dataCy?: string
  /** Enter with nothing highlighted in the list — the caller's "commit this row" shortcut. */
  onEnter?: () => void
}

export function GatewaySelect({
  value,
  onChange,
  options,
  placeholder = 'Gateway name',
  className = '',
  dataCy,
  onEnter,
}: GatewaySelectProps) {
  const [open, setOpen] = useState(false)
  const [highlight, setHighlight] = useState(-1)
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({})
  const [maxHeight, setMaxHeight] = useState(MAX_LIST_HEIGHT)
  const wrapRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const query = value.trim().toLowerCase()
  const filtered = query ? options.filter((o) => o.toLowerCase().includes(query)) : options

  function openDropdown() {
    const rect = inputRef.current?.getBoundingClientRect()
    if (!rect) return
    const below = window.innerHeight - rect.bottom - VIEWPORT_MARGIN
    const above = rect.top - VIEWPORT_MARGIN
    const dropUp = below < MIN_LIST_HEIGHT && above > below
    setMaxHeight(Math.min(MAX_LIST_HEIGHT, Math.max(MIN_LIST_HEIGHT, dropUp ? above : below)))
    setDropdownStyle({
      position: 'fixed',
      left: rect.left,
      minWidth: rect.width,
      zIndex: 9999,
      ...(dropUp ? { bottom: window.innerHeight - rect.top + 4 } : { top: rect.bottom + 4 }),
    })
    setOpen(true)
  }

  function close() {
    setOpen(false)
    setHighlight(-1)
  }

  useEffect(() => {
    if (!open) return
    function onOutside(e: MouseEvent) {
      const target = e.target as Node
      if (!wrapRef.current?.contains(target) && !dropdownRef.current?.contains(target)) close()
    }
    document.addEventListener('mousedown', onOutside)
    return () => document.removeEventListener('mousedown', onOutside)
  }, [open])

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (!open) return openDropdown()
      setHighlight((h) => (filtered.length ? (h + 1) % filtered.length : -1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (!open) return openDropdown()
      setHighlight((h) => (filtered.length ? (h <= 0 ? filtered.length - 1 : h - 1) : -1))
    } else if (e.key === 'Enter') {
      e.preventDefault()
      // Nothing highlighted means the typed text is the answer, so Enter still adds the row the way
      // it did when this was a plain input.
      if (open && highlight >= 0 && filtered[highlight]) {
        onChange(filtered[highlight])
        close()
      } else {
        close()
        onEnter?.()
      }
    } else if (e.key === 'Escape') {
      if (open) {
        e.stopPropagation()
        close()
      }
    }
  }

  return (
    <div ref={wrapRef} className={`relative ${className}`} data-cy={dataCy}>
      <input
        ref={inputRef}
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
          setHighlight(-1)
          if (!open) openDropdown()
        }}
        onFocus={openDropdown}
        // Options cancel their own mousedown, so focus only really leaves for another field — at
        // which point a list still hanging under this row is just clutter.
        onBlur={close}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        role="combobox"
        aria-expanded={open}
        aria-autocomplete="list"
        autoComplete="off"
        className="w-full rounded-lg border border-slate-200 bg-transparent py-2 pl-3 pr-8 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]"
      />
      <button
        type="button"
        tabIndex={-1}
        aria-label={open ? 'Hide gateway list' : 'Show gateway list'}
        // Keeps focus in the input, so the editors' commit-on-leave never fires from a toggle click.
        onMouseDown={(e) => {
          e.preventDefault()
          inputRef.current?.focus()
          if (open) close()
          else openDropdown()
        }}
        className="absolute inset-y-0 right-0 flex items-center px-2 text-slate-500 hover:text-slate-600 dark:hover:text-slate-200"
      >
        <ChevronDown size={13} className={`transition-transform duration-150 ${open ? 'rotate-180' : ''}`} />
      </button>

      {open && createPortal(
        <div
          ref={dropdownRef}
          style={dropdownStyle}
          className="w-max min-w-[13rem] max-w-[320px] rounded-lg border border-slate-200 bg-white shadow-lg dark:border-[#222226] dark:bg-[#111118]"
        >
          <div
            className="overflow-y-auto py-0.5"
            style={{ maxHeight: `${Math.min(Math.max(filtered.length, 1) * 32 + 8, maxHeight)}px` }}
          >
            {filtered.length === 0 ? (
              <p className="px-3 py-2.5 text-sm text-slate-500">
                {value.trim()
                  ? <>No connector matches — <span className="font-mono">{value.trim()}</span> is used as typed</>
                  : 'No gateways'}
              </p>
            ) : (
              filtered.map((o, i) => (
                <button
                  key={o}
                  type="button"
                  data-value={o}
                  onMouseDown={(e) => e.preventDefault()}
                  onMouseEnter={() => setHighlight(i)}
                  onClick={() => { onChange(o); close(); inputRef.current?.focus() }}
                  className={`w-full px-3 py-2.5 text-left font-mono text-sm transition-colors ${
                    i === highlight ? 'bg-slate-50 dark:bg-[#1c1c24]' : ''
                  } ${o === value ? 'bg-brand-50/50 font-medium text-brand-600 dark:bg-brand-900/10 dark:text-brand-400' : ''}`}
                >
                  {o}
                </button>
              ))
            )}
          </div>
        </div>,
        document.body
      )}
    </div>
  )
}
