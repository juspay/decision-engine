import { useState, useRef, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown } from 'lucide-react'

interface Option {
  value: string
  label: string
}

interface SearchableSelectProps {
  value: string
  onChange: (value: string) => void
  options: Option[]
  className?: string
  /**
   * Replaces the compact `cond-select` trigger styling entirely — pass a full
   * input-style class set (padding, border, text size) to render the trigger
   * like a regular form field. The trigger becomes block-level full-width and
   * the label no longer truncates at 10rem.
   */
  triggerClassName?: string
  disabled?: boolean
  dataCy?: string
}

export function SearchableSelect({
  value,
  onChange,
  options,
  className = '',
  triggerClassName,
  disabled = false,
  dataCy,
}: SearchableSelectProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({})
  const triggerRef = useRef<HTMLButtonElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const selectedLabel = options.find(o => o.value === value)?.label ?? value

  const filtered = (query
    ? options.filter(o =>
        o.label.toLowerCase().includes(query.toLowerCase()) ||
        o.value.toLowerCase().includes(query.toLowerCase())
      )
    : options
  ).slice().sort((a, b) => a.label.localeCompare(b.label))

  function openDropdown() {
    const rect = triggerRef.current?.getBoundingClientRect()
    if (!rect) return
    setDropdownStyle({
      position: 'fixed',
      top: rect.bottom + 4,
      left: rect.left,
      minWidth: rect.width,
      zIndex: 9999,
    })
    setOpen(true)
  }

  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  useEffect(() => {
    if (!open) return
    function onOutside(e: MouseEvent) {
      const target = e.target as Node
      if (
        !triggerRef.current?.contains(target) &&
        !dropdownRef.current?.contains(target)
      ) {
        close()
      }
    }
    // Listens on the document rather than the search input, so Escape still closes the list once
    // focus has moved off the input.
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') close()
    }
    document.addEventListener('mousedown', onOutside)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onOutside)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  function close() {
    setOpen(false)
    setQuery('')
  }

  function select(val: string) {
    onChange(val)
    close()
  }

  return (
    <div className={`relative ${className || (triggerClassName ? 'block' : 'inline-block')}`} data-cy={dataCy}>
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        onClick={() => open ? close() : openDropdown()}
        className={
          triggerClassName
            ? `flex w-full items-center justify-between gap-2 text-left ${triggerClassName}`
            : 'cond-select flex w-full items-center justify-between gap-1 pr-2'
        }
        style={{ backgroundImage: 'none', display: 'flex', alignItems: 'center' }}
        data-value={value}
      >
        <span className="min-w-0 truncate text-left">{selectedLabel || <span className="text-slate-500">select...</span>}</span>
        <ChevronDown
          size={11}
          className={`shrink-0 text-slate-500 transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && createPortal(
        <div
          ref={dropdownRef}
          style={dropdownStyle}
          className="w-max min-w-[13rem] max-w-[320px] rounded-lg border border-slate-200 dark:border-[#222226] bg-white dark:bg-[#111118] shadow-lg"
        >
          <div className="p-1.5 border-b border-slate-100 dark:border-[#1c1c24]">
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={e => setQuery(e.target.value)}
              placeholder="Search…"
              className="w-full rounded-md border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226] dark:bg-[#0f0f11]"
            />
          </div>
          <div
            className="overflow-y-auto py-0.5"
            style={{ maxHeight: `${Math.min(filtered.length * 32 + 8, 272)}px` }}
          >
            {filtered.length === 0 ? (
              <p className="px-3 py-2.5 text-sm text-slate-500">No matches</p>
            ) : (
              filtered.map(o => (
                <button
                  key={o.value}
                  type="button"
                  data-value={o.value}
                  onClick={() => select(o.value)}
                  className={`w-full px-3 py-2.5 text-left text-sm transition-colors hover:bg-slate-50 dark:hover:bg-[#1c1c24] ${
                    o.value === value
                      ? 'text-brand-600 dark:text-brand-400 font-medium bg-brand-50/50 dark:bg-brand-900/10'
                      : ''
                  }`}
                >
                  {o.label}
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
