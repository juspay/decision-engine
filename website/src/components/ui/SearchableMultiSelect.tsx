import { useState, useRef, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown, X } from 'lucide-react'

interface Option {
  value: string
  label: string
}

interface SearchableMultiSelectProps {
  values: string[]
  onChange: (values: string[]) => void
  options: Option[]
  placeholder?: string
  className?: string
}

export function SearchableMultiSelect({
  values,
  onChange,
  options,
  placeholder = 'Select…',
  className = '',
}: SearchableMultiSelectProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({})
  const triggerRef = useRef<HTMLDivElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const filtered = (query
    ? options.filter(o =>
        o.label.toLowerCase().includes(query.toLowerCase()) ||
        o.value.toLowerCase().includes(query.toLowerCase())
      )
    : options
  )

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
      if (!triggerRef.current?.contains(target) && !dropdownRef.current?.contains(target)) {
        close()
      }
    }
    // Listens on the document rather than the search input: focus moves to an option button as soon
    // as one is toggled, and Escape has to close the list from there too.
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

  function toggle(val: string) {
    onChange(values.includes(val) ? values.filter(v => v !== val) : [...values, val])
  }

  function remove(val: string, e: React.MouseEvent) {
    e.stopPropagation()
    onChange(values.filter(v => v !== val))
  }

  return (
    <div className={`relative ${className}`}>
      <div
        ref={triggerRef}
        onClick={() => (open ? close() : openDropdown())}
        className="flex w-full min-w-0 cursor-pointer flex-wrap items-center gap-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm dark:border-[#222226]"
      >
        {values.length === 0 && (
          <span className="text-slate-400">{placeholder}</span>
        )}
        {values.map(v => {
          const label = options.find(o => o.value === v)?.label ?? v
          return (
            <span
              key={v}
              className="flex items-center gap-1 rounded-md bg-brand-100 dark:bg-brand-500/20 border border-brand-200 dark:border-brand-500/40 px-1.5 py-0.5 font-medium text-brand-700 dark:text-brand-300"
            >
              {label}
              <button
                type="button"
                onClick={(e) => remove(v, e)}
                className="text-brand-400 hover:text-brand-600 dark:hover:text-brand-200"
              >
                <X size={10} />
              </button>
            </span>
          )
        })}
        <ChevronDown
          size={11}
          className={`ml-auto shrink-0 text-slate-400 transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </div>

      {open && createPortal(
        <div
          ref={dropdownRef}
          style={dropdownStyle}
          className="w-max min-w-[13rem] max-w-[320px] rounded-lg border border-slate-200 dark:border-[#222226] bg-white dark:bg-[#111118] shadow-lg"
        >
          <div className="border-b border-slate-100 dark:border-[#1c1c24] p-1.5">
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={e => setQuery(e.target.value)}
              placeholder="Search…"
              className="w-full rounded-md border border-slate-200 bg-slate-50 px-2.5 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226] dark:bg-[#0f0f11]"
            />
          </div>
          <div className="max-h-52 overflow-y-auto py-0.5">
            {filtered.length === 0 ? (
              <p className="px-3 py-2.5 text-sm text-slate-400">No matches</p>
            ) : (
              filtered.map(o => {
                const checked = values.includes(o.value)
                return (
                  <button
                    key={o.value}
                    type="button"
                    data-value={o.value}
                    onClick={() => toggle(o.value)}
                    className={`flex w-full items-center gap-2 px-3 py-2.5 text-left text-sm transition-colors hover:bg-slate-50 dark:hover:bg-[#1c1c24] ${checked ? 'text-brand-600 dark:text-brand-400' : 'text-slate-700 dark:text-[#c8d0de]'}`}
                  >
                    <span className={`flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border ${checked ? 'border-brand-500 bg-brand-500 text-white' : 'border-slate-300 dark:border-[#3a4258]'}`}>
                      {checked && (
                        <svg viewBox="0 0 8 8" className="h-2 w-2 fill-current">
                          <path d="M1 4l2 2 4-4" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      )}
                    </span>
                    {o.label}
                  </button>
                )
              })
            )}
          </div>
        </div>,
        document.body
      )}
    </div>
  )
}
