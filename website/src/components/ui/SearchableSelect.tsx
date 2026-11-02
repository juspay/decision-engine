import { useState, useRef, useEffect } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown, X } from 'lucide-react'
import { useCloseOnFrameBlur } from '../../hooks/useCloseOnFrameBlur'

interface Option {
  value: string
  label: string
}

/** Matches the list's `max-w-[320px]`. */
const LIST_MAX_WIDTH = 320

interface CommonProps {
  options: Option[]
  className?: string
  /**
   * Replaces the trigger's default styling entirely — pass a full input-style class set (padding,
   * border, text size) to render the trigger like a regular form field. The chevron is drawn here.
   */
  triggerClassName?: string
  /** Called when the list closes, e.g. to act on a whole selection at once rather than on every toggle. */
  onClose?: () => void
}

interface SingleProps extends CommonProps {
  multiple?: false
  value: string
  onChange: (value: string) => void
  disabled?: boolean
  dataCy?: string
}

interface MultipleProps extends CommonProps {
  multiple: true
  values: string[]
  onChange: (values: string[]) => void
  placeholder?: string
  /** One-line summary ("Card +1") instead of a chip per value, so the trigger keeps its height. */
  compact?: boolean
}

export type SearchableSelectProps = SingleProps | MultipleProps

/**
 * A trigger that opens a searchable list in a portal. Single-select closes on pick and lists options
 * alphabetically; `multiple` toggles values with checkboxes, keeps the list open and keeps the
 * caller's option order.
 */
export function SearchableSelect(props: SearchableSelectProps) {
  const { options, className = '', triggerClassName, onClose } = props
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({})
  const triggerRef = useRef<HTMLElement>(null)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const matching = query
    ? options.filter(o =>
        o.label.toLowerCase().includes(query.toLowerCase()) ||
        o.value.toLowerCase().includes(query.toLowerCase())
      )
    : options
  const filtered = props.multiple
    ? matching
    : matching.slice().sort((a, b) => a.label.localeCompare(b.label))

  function openDropdown() {
    const rect = triggerRef.current?.getBoundingClientRect()
    if (!rect) return
    // The list is up to LIST_MAX_WIDTH wide; near the right edge it opens leftwards from the
    // trigger's right edge instead of running off-screen.
    const fitsRightwards = rect.left + Math.max(rect.width, LIST_MAX_WIDTH) <= window.innerWidth
    setDropdownStyle({
      position: 'fixed',
      top: rect.bottom + 4,
      ...(fitsRightwards ? { left: rect.left } : { right: window.innerWidth - rect.right }),
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

  useCloseOnFrameBlur(open, close)

  // Read through a ref: the outside-click and Escape listeners are bound when the list opens and
  // would otherwise call the onClose of that render, before any toggles.
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose

  function close() {
    setOpen(false)
    setQuery('')
    onCloseRef.current?.()
  }

  const toggleOpen = () => (open ? close() : openDropdown())
  const labelOf = (v: string) => options.find(o => o.value === v)?.label ?? v

  let trigger: React.ReactNode
  let renderOption: (o: Option) => React.ReactNode
  let listStyle: { className: string; style?: React.CSSProperties }

  if (props.multiple) {
    const { values, onChange, placeholder = 'Select…', compact = false } = props
    const remove = (val: string, e: React.MouseEvent) => {
      e.stopPropagation()
      onChange(values.filter(v => v !== val))
    }

    trigger = (
      <div
        ref={triggerRef as React.RefObject<HTMLDivElement>}
        onClick={toggleOpen}
        className={
          triggerClassName
            ? `flex min-w-0 cursor-pointer items-center gap-1 ${triggerClassName}`
            : 'flex w-full min-w-0 cursor-pointer flex-wrap items-center gap-1 rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm dark:border-[#222226]'
        }
      >
        {values.length === 0 && (
          <span className={compact ? 'min-w-0 truncate' : 'text-slate-500'}>{placeholder}</span>
        )}
        {compact && values.length > 0 && (
          <span className="min-w-0 truncate" title={values.map(labelOf).join(', ')}>
            {labelOf(values[0])}
            {values.length > 1 ? ` +${values.length - 1}` : ''}
          </span>
        )}
        {!compact && values.map(v => (
          <span
            key={v}
            className="flex items-center gap-1 rounded-md bg-brand-100 dark:bg-brand-500/20 border border-brand-200 dark:border-brand-500/40 px-1.5 py-0.5 font-medium text-brand-700 dark:text-brand-300"
          >
            {labelOf(v)}
            <button
              type="button"
              onClick={(e) => remove(v, e)}
              className="text-brand-600 hover:text-brand-600 dark:hover:text-brand-200"
            >
              <X size={10} />
            </button>
          </span>
        ))}
        {/* Always drawn, so a styled trigger still reads as a dropdown. */}
        <ChevronDown
          size={triggerClassName ? 14 : 11}
          className={`ml-auto shrink-0 text-slate-400 transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </div>
    )

    renderOption = (o) => {
      const checked = values.includes(o.value)
      return (
        <button
          key={o.value}
          type="button"
          data-value={o.value}
          onClick={() => onChange(checked ? values.filter(v => v !== o.value) : [...values, o.value])}
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
    }

    listStyle = { className: 'max-h-52 overflow-y-auto py-0.5' }
  } else {
    const { value, onChange, disabled = false } = props
    const selectedLabel = labelOf(value)

    trigger = (
      <button
        ref={triggerRef as React.RefObject<HTMLButtonElement>}
        type="button"
        disabled={disabled}
        onClick={toggleOpen}
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
    )

    renderOption = (o) => (
      <button
        key={o.value}
        type="button"
        data-value={o.value}
        onClick={() => { onChange(o.value); close() }}
        className={`w-full px-3 py-2.5 text-left text-sm transition-colors hover:bg-slate-50 dark:hover:bg-[#1c1c24] ${
          o.value === value
            ? 'text-brand-600 dark:text-brand-400 font-medium bg-brand-50/50 dark:bg-brand-900/10'
            : ''
        }`}
      >
        {o.label}
      </button>
    )

    listStyle = {
      className: 'overflow-y-auto py-0.5',
      style: { maxHeight: `${Math.min(filtered.length * 32 + 8, 272)}px` },
    }
  }

  const wrapperClass = props.multiple
    ? `relative ${className}`
    : `relative ${className || (triggerClassName ? 'block' : 'inline-block')}`

  return (
    <div className={wrapperClass} data-cy={props.multiple ? undefined : props.dataCy}>
      {trigger}

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
          <div className={listStyle.className} style={listStyle.style}>
            {filtered.length === 0 ? (
              <p className="px-3 py-2.5 text-sm text-slate-500">No matches</p>
            ) : (
              filtered.map(renderOption)
            )}
          </div>
        </div>,
        document.body
      )}
    </div>
  )
}
