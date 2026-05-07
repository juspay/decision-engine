import { useState, useRef, useEffect, useMemo, useCallback } from 'react'
import { createPortal } from 'react-dom'

interface ComboboxProps {
  value: string
  onChange: (value: string) => void
  options: string[]
  placeholder?: string
  className?: string
}

export function Combobox({ value, onChange, options, placeholder, className = '' }: ComboboxProps) {
  const [open, setOpen] = useState(false)
  const [highlightedIndex, setHighlightedIndex] = useState(-1)
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({})

  const inputRef = useRef<HTMLInputElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)

  const filtered = useMemo(
    () => value
      ? options.filter(o => o.toLowerCase().includes(value.toLowerCase()))
      : options,
    [options, value],
  )

  const close = useCallback(() => {
    setOpen(false)
    setHighlightedIndex(-1)
  }, [])

  function openDropdown() {
    const rect = inputRef.current?.getBoundingClientRect()
    if (!rect) return
    setDropdownStyle({
      position: 'fixed',
      top: rect.bottom + 4,
      left: rect.left,
      width: rect.width,
      zIndex: 9999,
    })
    setHighlightedIndex(-1)
    setOpen(true)
  }

  useEffect(() => {
    if (!open) return
    function onOutside(e: MouseEvent) {
      const target = e.target as Node
      if (!inputRef.current?.contains(target) && !dropdownRef.current?.contains(target)) {
        close()
      }
    }
    document.addEventListener('mousedown', onOutside)
    return () => document.removeEventListener('mousedown', onOutside)
  }, [open, close])

  // reset highlight when filtered list changes
  useEffect(() => { setHighlightedIndex(-1) }, [filtered.length])

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (!open && e.key !== 'Escape' && e.key !== 'Tab') {
      openDropdown()
      return
    }
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        setHighlightedIndex(i => (i < filtered.length - 1 ? i + 1 : 0))
        break
      case 'ArrowUp':
        e.preventDefault()
        setHighlightedIndex(i => (i > 0 ? i - 1 : filtered.length - 1))
        break
      case 'Enter':
        if (highlightedIndex >= 0 && filtered[highlightedIndex] !== undefined) {
          e.preventDefault()
          onChange(filtered[highlightedIndex])
          close()
        }
        break
      case 'Escape':
        close()
        break
    }
  }

  return (
    <div className="relative">
      <input
        ref={inputRef}
        type="text"
        value={value}
        placeholder={placeholder}
        className={className}
        onChange={e => { onChange(e.target.value); openDropdown() }}
        onFocus={openDropdown}
        onKeyDown={handleKeyDown}
        autoComplete="off"
      />

      {open && filtered.length > 0 && createPortal(
        <div
          ref={dropdownRef}
          style={dropdownStyle}
          className="rounded-lg border border-slate-200 dark:border-[#222226] bg-white dark:bg-[#111118] shadow-lg overflow-hidden"
        >
          <div
            className="overflow-y-auto py-0.5"
            style={{ maxHeight: `${Math.min(filtered.length * 32 + 8, 240)}px` }}
          >
            {filtered.map((option, i) => (
              <button
                key={option}
                type="button"
                onMouseDown={e => e.preventDefault()}
                onClick={() => { onChange(option); close() }}
                className={`w-full text-left px-3 py-2 text-xs transition-colors ${
                  i === highlightedIndex
                    ? 'bg-brand-500 text-white'
                    : option === value
                    ? 'bg-brand-50/50 dark:bg-brand-900/10 text-brand-600 dark:text-brand-400 font-medium'
                    : 'hover:bg-slate-50 dark:hover:bg-[#1c1c24] text-slate-700 dark:text-slate-200'
                }`}
              >
                {option}
              </button>
            ))}
          </div>
        </div>,
        document.body,
      )}
    </div>
  )
}
