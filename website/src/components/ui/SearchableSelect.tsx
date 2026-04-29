import { useState, useRef, useEffect } from 'react'
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
  disabled?: boolean
  dataCy?: string
}

export function SearchableSelect({
  value,
  onChange,
  options,
  className = '',
  disabled = false,
  dataCy,
}: SearchableSelectProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const containerRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const selectedLabel = options.find(o => o.value === value)?.label ?? value

  const filtered = (query
    ? options.filter(o =>
        o.label.toLowerCase().includes(query.toLowerCase()) ||
        o.value.toLowerCase().includes(query.toLowerCase())
      )
    : options
  ).slice().sort((a, b) => a.label.localeCompare(b.label))

  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  useEffect(() => {
    if (!open) return
    function onOutside(e: MouseEvent) {
      if (!containerRef.current?.contains(e.target as Node)) close()
    }
    document.addEventListener('mousedown', onOutside)
    return () => document.removeEventListener('mousedown', onOutside)
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
    <div ref={containerRef} className={`relative inline-block ${className}`} data-cy={dataCy}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen(o => !o)}
        className="cond-select flex items-center gap-1 pr-2"
        style={{ backgroundImage: 'none', display: 'flex', alignItems: 'center' }}
        data-value={value}
      >
        <span className="truncate max-w-[10rem]">{selectedLabel || <span className="text-slate-400">select...</span>}</span>
        <ChevronDown
          size={11}
          className={`shrink-0 text-slate-400 transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <div className="absolute z-50 left-0 mt-1 min-w-full w-max max-w-[240px] rounded-lg border border-slate-200 dark:border-[#222226] bg-white dark:bg-[#111118] shadow-lg">
          <div className="p-1.5 border-b border-slate-100 dark:border-[#1c1c24]">
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={e => setQuery(e.target.value)}
              onKeyDown={e => e.key === 'Escape' && close()}
              placeholder="Search…"
              className="w-full rounded px-2 py-1 text-xs bg-slate-50 dark:bg-[#0f0f11] border border-slate-200 dark:border-[#222226] focus:outline-none focus:ring-1 focus:ring-brand-500"
            />
          </div>
          <div
            className="overflow-y-auto py-0.5"
            style={{ maxHeight: `${Math.min(filtered.length * 32 + 8, 272)}px` }}
          >
            {filtered.length === 0 ? (
              <p className="px-3 py-2 text-xs text-slate-400">No matches</p>
            ) : (
              filtered.map(o => (
                <button
                  key={o.value}
                  type="button"
                  onClick={() => select(o.value)}
                  className={`w-full text-left px-3 py-2 text-xs transition-colors hover:bg-slate-50 dark:hover:bg-[#1c1c24] ${
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
        </div>
      )}
    </div>
  )
}
