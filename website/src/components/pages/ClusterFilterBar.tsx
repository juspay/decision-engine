import { useEffect, useMemo, useRef, useState } from 'react'
import { Check, ChevronDown, X } from 'lucide-react'
import { Button } from '../ui/Button'
import {
  countActiveFilters,
  type ClusterFacet,
  type ClusterFilters,
} from '../../hooks/useCostRouting'

/**
 * Shared cluster filter bar — used by BOTH the read-only Ingested Data table and the editable Costs
 * table, so the two never drift on what's filterable or how a value is spelled.
 *
 * Every column is a multi-select: values within a column are OR'd, columns are AND'd. Options come
 * from the merchant's own data (highest-traffic first), which matters because these are not friendly
 * enums — nobody types `mcsuperpremiumcredit` or
 * `Interregional Consumer US Issued EEA Merchant CP Credit [EEAINT CN CR]` twice.
 *
 * Filtering is server-side; this component only owns the selection.
 */

/** The filterable columns, in table order, with the heading each sits under. */
export const FILTER_COLUMNS: { key: FilterKey; label: string }[] = [
  { key: 'card_network', label: 'Network' },
  { key: 'variant', label: 'Program' },
  { key: 'funding', label: 'Type' },
  { key: 'issuer_country', label: 'Country' },
  { key: 'currency', label: 'Currency' },
  { key: 'ic_category', label: 'Category' },
  { key: 'verdict', label: 'Fit' },
]

/** Keys of `ClusterFilters` that hold a value set (everything except the free-text `q`). */
export type FilterKey = Exclude<keyof ClusterFilters, 'q'>

const controlClass =
  'flex w-full items-center justify-between gap-1 rounded-md border border-slate-200 bg-white px-2 py-1 ' +
  'text-[12px] text-slate-700 hover:border-slate-300 focus:border-brand-500 focus:outline-none ' +
  'dark:border-[#232833] dark:bg-[#0b1017] dark:text-[#c7cede] dark:hover:border-[#2e3545]'

const searchClass =
  'w-full rounded-md border border-slate-200 bg-white px-2 py-1 text-[12px] text-slate-900 ' +
  'placeholder:text-slate-500 focus:border-brand-500 focus:outline-none ' +
  'dark:border-[#232833] dark:bg-[#0b1017] dark:text-white dark:placeholder:text-[#5c6577]'

/**
 * One column's multi-select. A popover of checkboxes rather than a native `<select multiple>`: the
 * native control needs ctrl-click to add a value and shows no counts, and these lists run to hundreds
 * of options (one per interchange category), so it carries its own type-ahead.
 */
function MultiSelect({
  label,
  selected,
  options,
  onChange,
}: {
  label: string
  selected: string[]
  options: ClusterFacet[]
  onChange: (next: string[]) => void
}) {
  const [open, setOpen] = useState(false)
  const [search, setSearch] = useState('')
  const ref = useRef<HTMLDivElement>(null)

  // Close on outside click / Escape, so several of these can sit side by side without trapping focus.
  useEffect(() => {
    if (!open) return
    function onDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])

  const visible = useMemo(() => {
    const q = search.trim().toLowerCase()
    const list = q ? options.filter((o) => o.value.toLowerCase().includes(q)) : options
    // Selected values stay visible even when they don't match the search, so a click can undo them.
    const sel = new Set(selected)
    const missing = selected
      .filter((v) => !list.some((o) => o.value === v))
      .map((v) => ({ dim: '', value: v, txns: 0 }))
    return [...missing, ...list].slice(0, 300).map((o) => ({ ...o, on: sel.has(o.value) }))
  }, [options, search, selected])

  function toggle(value: string) {
    onChange(selected.includes(value) ? selected.filter((v) => v !== value) : [...selected, value])
  }

  // Always name the column. Showing a lone selected value on its own made the bar unreadable: seven
  // unlabelled controls sit in a row, so "alipay" under Program looked like it belonged to Network.
  const summary =
    selected.length === 0
      ? label
      : selected.length === 1
        ? `${label}: ${selected[0]}`
        : `${label} · ${selected.length}`

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        className={controlClass}
        aria-label={`Filter by ${label}`}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className={`truncate ${selected.length ? 'font-medium' : 'text-slate-500 dark:text-[#78849a]'}`}>
          {summary}
        </span>
        <ChevronDown size={13} className="shrink-0 opacity-60" />
      </button>

      {open && (
        <div className="absolute z-30 mt-1 w-[260px] rounded-lg border border-slate-200 bg-white p-2 shadow-lg dark:border-[#232833] dark:bg-[#0b0e14]">
          <input
            autoFocus
            className={searchClass}
            value={search}
            placeholder={`Search ${label.toLowerCase()}…`}
            onChange={(e) => setSearch(e.target.value)}
          />
          {selected.length > 0 && (
            <button
              type="button"
              className="mt-1 w-full rounded px-1 py-0.5 text-left text-[11px] text-brand-600 hover:underline dark:text-brand-400 leading-4"
              onClick={() => onChange([])}
            >
              Clear {selected.length} selected
            </button>
          )}
          <div className="mt-1 max-h-56 overflow-y-auto">
            {visible.length === 0 && (
              <p className="px-1 py-2 text-[12px] text-slate-500 leading-4">No matching values.</p>
            )}
            {visible.map((o) => (
              <button
                key={o.value}
                type="button"
                className="flex w-full items-center gap-2 rounded px-1 py-1 text-left text-[12px] text-slate-700 hover:bg-slate-100 dark:text-[#c7cede] dark:hover:bg-[#141a24] leading-4"
                onClick={() => toggle(o.value)}
              >
                <span
                  className={`flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded border ${
                    o.on
                      ? 'border-brand-500 bg-brand-500 text-white'
                      : 'border-slate-300 dark:border-[#39414f]'
                  }`}
                >
                  {o.on && <Check size={10} strokeWidth={3} />}
                </span>
                <span className="min-w-0 flex-1 truncate" title={o.value}>
                  {o.value}
                </span>
                {o.txns > 0 && (
                  <span className="shrink-0 text-[11px] text-slate-500 leading-4">
                    {o.txns.toLocaleString()}
                  </span>
                )}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

export function ClusterFilterBar({
  filters,
  facets,
  onChange,
  rightSlot,
}: {
  filters: ClusterFilters
  /** Facet options grouped by column, from `useClusterFacets`. */
  facets: Record<string, ClusterFacet[]>
  onChange: (next: ClusterFilters) => void
  /** Optional trailing content (result counts, paging hints). */
  rightSlot?: React.ReactNode
}) {
  const active = countActiveFilters(filters)
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        {active > 0 && (
          <Button variant="ghost" onClick={() => onChange({})}>
            <X size={14} /> Clear {active} filter{active === 1 ? '' : 's'}
          </Button>
        )}
        {rightSlot}
      </div>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-7">
        {FILTER_COLUMNS.map((col) => (
          <MultiSelect
            key={col.key}
            label={col.label}
            selected={filters[col.key] ?? []}
            options={facets[col.key] ?? []}
            onChange={(next) => onChange({ ...filters, [col.key]: next })}
          />
        ))}
      </div>
    </div>
  )
}
