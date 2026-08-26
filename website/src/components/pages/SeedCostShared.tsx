import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Pencil, Trash2, X } from 'lucide-react'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { type SeedCostRow } from '../../hooks/useCostRouting'

// Shared building blocks for the merchant's seed cost table (contract rates for the multi-objective
// simulator): what each PSP charges per card scenario, split into interchange / scheme / markup /
// fixed. These used to back a standalone panel; they now compose the Contract-baseline layer of the
// connector cards in `CostsPanel`, which owns the table + persistence.

// Flat pastel chips for the PSP / Network cells, matching the Contract-rates design: soft fill,
// coloured text, no border ring (the shared Badge uses a ring, so this is intentionally local).
type ChipVariant = 'blue' | 'purple' | 'green' | 'teal' | 'orange' | 'red' | 'gray'
const CHIP_CLASSES: Record<ChipVariant, string> = {
  blue: 'bg-blue-50 text-blue-600 dark:bg-blue-500/12 dark:text-blue-300',
  purple: 'bg-violet-50 text-violet-600 dark:bg-violet-500/12 dark:text-violet-300',
  green: 'bg-emerald-50 text-emerald-700 dark:bg-emerald-500/12 dark:text-emerald-300',
  teal: 'bg-teal-50 text-teal-700 dark:bg-teal-500/12 dark:text-teal-300',
  orange: 'bg-orange-50 text-orange-700 dark:bg-orange-500/12 dark:text-orange-300',
  red: 'bg-rose-50 text-rose-600 dark:bg-rose-500/12 dark:text-rose-300',
  gray: 'bg-slate-100 text-slate-600 dark:bg-white/[0.06] dark:text-slate-300',
}
export function Chip({ variant, children }: { variant: ChipVariant; children: ReactNode }) {
  return (
    <span
      className={`inline-flex max-w-full items-center truncate rounded-md px-2 py-0.5 text-xs font-semibold ${CHIP_CLASSES[variant]}`}
    >
      {children}
    </span>
  )
}

// Full-width input used inside the scenario modal.
const modalInput =
  'w-full rounded-md border border-slate-200 bg-white px-2 py-1.5 text-sm ' +
  'text-slate-900 placeholder:text-slate-500 focus:border-brand-500 focus:outline-none ' +
  'dark:border-[#232833] dark:bg-[#0b1017] dark:text-white disabled:opacity-50'

// Colour a PSP / card-network chip. Known names get an on-brand colour; anything else falls back to
// a deterministic colour so the same PSP always reads the same across the table.
const PSP_VARIANTS: Record<string, ChipVariant> = {
  adyen: 'blue',
  stripe: 'purple',
  checkout: 'green',
  'checkout.com': 'green',
  braintree: 'orange',
  authorizenet: 'teal',
  'authorize.net': 'teal',
  chase: 'blue',
  paypal: 'blue',
  worldpay: 'red',
}
const NETWORK_VARIANTS: Record<string, ChipVariant> = {
  visa: 'blue',
  mastercard: 'red',
  amex: 'purple',
  'american express': 'purple',
  discover: 'orange',
  ideal: 'orange',
  jcb: 'green',
  diners: 'gray',
  rupay: 'green',
  unionpay: 'red',
}
const FALLBACK_PALETTE: ChipVariant[] = ['blue', 'purple', 'green', 'teal', 'orange', 'red']
function hashVariant(s: string): ChipVariant {
  let h = 0
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0
  return FALLBACK_PALETTE[h % FALLBACK_PALETTE.length]
}
export function pspVariant(psp: string): ChipVariant {
  const key = psp.toLowerCase().trim()
  return PSP_VARIANTS[key] ?? hashVariant(key)
}
export function networkVariant(net: string): ChipVariant {
  return NETWORK_VARIANTS[net.toLowerCase().trim()] ?? 'gray'
}

// Common currency symbols; unknown codes render as the bare code ("SGD 0.10").
const CCY_SYMBOLS: Record<string, string> = {
  usd: '$', eur: '€', gbp: '£', cad: '$', aud: '$', inr: '₹', jpy: '¥', aed: 'AED ',
}
export function ccySymbol(code?: string | null): string {
  if (!code) return '$'
  return CCY_SYMBOLS[code.toLowerCase()] ?? `${code.toUpperCase()} `
}

export function totalRate(r: SeedCostRow): number {
  return r.interchange_bps + r.scheme_bps + r.markup_bps
}

/** A bps fee rendered as a percentage: 180 bps → "1.80%". */
export function pctText(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`
}

/** "2.05% + $0.10" — the contract's headline effective rate for a row. */
export function formatEffective(r: SeedCostRow, currency?: string | null): string {
  const pct = pctText(totalRate(r))
  if (!r.fixed) return pct
  return `${pct} + ${ccySymbol(currency ?? r.transaction_currency)}${r.fixed.toFixed(2)}`
}

/**
 * Card programs a contract tier can price separately, in the vocabulary the resolver matches on.
 * These are the values `derive_cluster_key` produces from `txn_card_info.card_program`, and a tier's
 * `card_type` is compared to them by exact case-insensitive equality — so this list, the config's
 * seed tiers, and the decide-time field have to stay in step. Interchange varies severalfold across
 * them (a premium consumer credit card can cost 2-4x a standard one), which is why it's a dimension.
 */
export const CARD_PROGRAMS = ['standard', 'premium', 'ultra_premium', 'commercial'] as const

/** A blank scenario row for a PSP (defaults to matching nothing — the merchant fills it in). */
export function blankRow(psp: string): SeedCostRow {
  return {
    psp,
    card_network: '',
    payment_method_type: '',
    card_type: '',
    transaction_currency: '',
    card_issuing_country: '',
    interchange_bps: 0,
    scheme_bps: 0,
    markup_bps: 0,
    fixed: 0,
    label: '',
    example_amount: null,
    is_default: false,
    effective_pct_bps: 0,
  }
}

/**
 * One read-only contract-rate row. The pencil opens the scenario modal to edit it; editing is
 * disabled while another scenario's modal is already open.
 */
export function SeedRow({
  row,
  busy,
  disabled,
  onEdit,
}: {
  row: SeedCostRow
  busy: boolean
  disabled: boolean
  onEdit: () => void
}) {
  return (
    <tr className="border-b border-slate-100 transition-colors last:border-0 hover:bg-slate-50 dark:border-[#1c1c23] dark:hover:bg-[#0f131b]">
      <td className="px-5 py-3 text-slate-700 dark:text-[#c7cfdd]">
        {row.is_default ? <Badge variant="gray">Default</Badge> : row.label || '—'}
      </td>
      <td className="py-3 pr-3">
        {row.psp ? <Chip variant={pspVariant(row.psp)}>{row.psp}</Chip> : '—'}
      </td>
      <td className="py-3 pr-3">
        {row.card_network ? (
          <Chip variant={networkVariant(row.card_network)}>{row.card_network}</Chip>
        ) : (
          <span className="text-slate-500">Any</span>
        )}
      </td>
      <td className="py-3 pr-3 capitalize text-slate-600 dark:text-[#c7cfdd]">
        {row.payment_method_type || <span className="text-slate-500">Any</span>}
      </td>
      <td className="py-3 pr-3 text-slate-600 dark:text-[#c7cfdd]">
        {/* Underscores read badly next to the other columns: "ultra_premium" → "ultra premium". */}
        {row.card_type ? (
          row.card_type.replace(/_/g, ' ')
        ) : (
          <span className="text-slate-500">Any</span>
        )}
      </td>
      <td className="py-3 pr-3 tabular-nums text-slate-600 dark:text-[#c7cfdd]">
        {row.transaction_currency?.toUpperCase() || <span className="text-slate-500">Any</span>}
      </td>
      <td className="py-3 pr-3 text-slate-600 dark:text-[#c7cfdd]">
        {row.card_issuing_country || <span className="text-slate-500">Any</span>}
      </td>
      <td className="py-3 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
        {pctText(row.interchange_bps)}
      </td>
      <td className="py-3 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
        {pctText(row.scheme_bps)}
      </td>
      <td className="py-3 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
        {pctText(row.markup_bps)}
      </td>
      <td className="py-3 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
        {ccySymbol(row.transaction_currency)}
        {row.fixed.toFixed(2)}
      </td>
      <td className="bg-brand-500/[0.06] py-3 pr-3 text-right tabular-nums font-semibold text-brand-700 dark:bg-brand-500/[0.1] dark:text-brand-300">
        {formatEffective(row)}
      </td>
      <td className="bg-brand-500/[0.06] py-3 pr-5 text-right dark:bg-brand-500/[0.1]">
        <button
          type="button"
          onClick={onEdit}
          disabled={disabled || busy}
          title="Edit scenario"
          aria-label="Edit scenario"
          className="inline-flex h-7 w-7 items-center justify-center rounded-md text-emerald-600 transition-colors hover:bg-emerald-500/10 disabled:opacity-40 dark:text-emerald-400"
        >
          <Pencil size={14} />
        </button>
      </td>
    </tr>
  )
}

/**
 * Add / edit a single contract-rate scenario in a modal (more room and clearer grouping than the
 * cramped inline table row). Holds its own buffer; Save hands the completed row back to the caller.
 */
export function ScenarioModal({
  initial,
  isNew,
  busy,
  saveError,
  onSave,
  onRemove,
  onClose,
}: {
  initial: SeedCostRow
  isNew: boolean
  busy: boolean
  saveError: string | null
  onSave: (updated: SeedCostRow) => void
  onRemove: () => void
  onClose: () => void
}) {
  const [buf, setBuf] = useState<SeedCostRow>(initial)
  const [localError, setLocalError] = useState<string | null>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const set = (patch: Partial<SeedCostRow>) => setBuf((b) => ({ ...b, ...patch }))

  useEffect(() => {
    panelRef.current?.focus()
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape' && !busy) onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [busy, onClose])

  function handleSave() {
    if (!buf.psp.trim()) {
      setLocalError('Enter a connector (PSP).')
      return
    }
    for (const v of [buf.interchange_bps, buf.scheme_bps, buf.markup_bps, buf.fixed]) {
      if (!Number.isFinite(v) || v < 0) {
        setLocalError('Fees must be non-negative numbers.')
        return
      }
    }
    setLocalError(null)
    onSave(buf)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/40 backdrop-blur-[2px]" onClick={() => !busy && onClose()} />
      <div
        ref={panelRef}
        tabIndex={-1}
        className="relative w-full max-w-lg rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl outline-none dark:border-[#2a303a] dark:bg-[#0d1118]"
      >
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
            {isNew ? 'Add scenario' : 'Edit scenario'}
          </h3>
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            aria-label="Close"
            className="text-slate-500 transition-colors hover:text-slate-700 disabled:opacity-40 dark:hover:text-white"
          >
            <X size={16} />
          </button>
        </div>

        <div className="mt-4 space-y-5">
          <div className="grid grid-cols-2 gap-3">
            <ModalField label="Scenario name">
              {buf.is_default ? (
                <div className="flex h-[34px] items-center">
                  <Badge variant="gray">Default</Badge>
                </div>
              ) : (
                <input
                  className={modalInput}
                  value={buf.label ?? ''}
                  placeholder="e.g. Visa credit — US"
                  onChange={(e) => set({ label: e.target.value })}
                />
              )}
            </ModalField>
            <ModalField label="PSP">
              <input
                className={`${modalInput} capitalize`}
                value={buf.psp}
                placeholder="e.g. adyen"
                onChange={(e) => set({ psp: e.target.value })}
              />
            </ModalField>
          </div>

          <div>
            <p className="mb-2 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">
              Applies to <span className="font-normal text-slate-500">(blank = any card)</span>
            </p>
            <div className="grid grid-cols-2 gap-3">
              <ModalField label="Network">
                <input
                  className={modalInput}
                  value={buf.card_network ?? ''}
                  placeholder="any"
                  disabled={buf.is_default}
                  onChange={(e) => set({ card_network: e.target.value })}
                />
              </ModalField>
              <ModalField label="Funding">
                <input
                  className={modalInput}
                  value={buf.payment_method_type ?? ''}
                  placeholder="any"
                  disabled={buf.is_default}
                  onChange={(e) => set({ payment_method_type: e.target.value })}
                />
              </ModalField>
              <ModalField label="Program">
                {/* A closed list, not free text: the resolver matches `card_type` by exact
                    (case-insensitive) string against what `derive_cluster_key` reads off
                    `card_program`, so a typo here would save a tier that silently never fires. */}
                <select
                  className={modalInput}
                  value={buf.card_type ?? ''}
                  disabled={buf.is_default}
                  onChange={(e) => set({ card_type: e.target.value })}
                >
                  <option value="">any</option>
                  {CARD_PROGRAMS.map((p) => (
                    <option key={p} value={p}>
                      {p}
                    </option>
                  ))}
                </select>
              </ModalField>
              <ModalField label="Currency">
                <input
                  className={`${modalInput} uppercase`}
                  value={buf.transaction_currency ?? ''}
                  placeholder="any"
                  disabled={buf.is_default}
                  onChange={(e) => set({ transaction_currency: e.target.value })}
                />
              </ModalField>
              <ModalField label="Region">
                <input
                  className={modalInput}
                  value={buf.card_issuing_country ?? ''}
                  placeholder="any"
                  disabled={buf.is_default}
                  onChange={(e) => set({ card_issuing_country: e.target.value })}
                />
              </ModalField>
            </div>
          </div>

          <div>
            <p className="mb-2 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">
              Fees <span className="font-normal text-slate-500">(rates in %, fixed per transaction)</span>
            </p>
            <div className="grid grid-cols-4 gap-3">
              <ModalField label="Interchange">
                <PctInput bps={buf.interchange_bps} onChange={(v) => set({ interchange_bps: v })} />
              </ModalField>
              <ModalField label="Scheme">
                <PctInput bps={buf.scheme_bps} onChange={(v) => set({ scheme_bps: v })} />
              </ModalField>
              <ModalField label="Markup">
                <PctInput bps={buf.markup_bps} onChange={(v) => set({ markup_bps: v })} />
              </ModalField>
              <ModalField label="Fixed">
                <NumInput value={buf.fixed} step={0.01} onChange={(v) => set({ fixed: v })} />
              </ModalField>
            </div>
          </div>

          <div className="flex items-center justify-between rounded-lg bg-slate-50 px-3 py-2 text-sm dark:bg-[#0b1017]">
            <span className="text-slate-500 dark:text-[#8d96aa]">Effective rate</span>
            <span className="tabular-nums font-medium text-slate-800 dark:text-white">
              {formatEffective(buf)}
            </span>
          </div>

          <ErrorMessage error={localError ?? saveError} />
        </div>

        <div className="mt-6 flex items-center justify-between">
          {isNew ? (
            <span />
          ) : (
            <Button variant="danger" size="sm" onClick={onRemove} disabled={busy}>
              <Trash2 size={13} /> Remove
            </Button>
          )}
          <div className="flex gap-2">
            <Button variant="secondary" size="sm" onClick={onClose} disabled={busy}>
              Cancel
            </Button>
            <Button variant="primary" size="sm" onClick={handleSave} disabled={busy}>
              {busy ? <Spinner size={13} /> : null} Save
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}

/** A labeled field wrapper for the scenario modal. */
function ModalField({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="block text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] leading-4">{label}</span>
      {children}
    </label>
  )
}

/** A fixed-amount input that parses to a number (blank → 0). */
function NumInput({
  value,
  step = 1,
  onChange,
}: {
  value: number
  step?: number
  onChange: (v: number) => void
}) {
  return (
    <input
      className={`${modalInput} text-right tabular-nums`}
      type="number"
      min="0"
      step={step}
      value={Number.isFinite(value) ? value : 0}
      onChange={(e) => {
        const v = parseFloat(e.target.value)
        onChange(Number.isFinite(v) ? v : 0)
      }}
    />
  )
}

/** A percentage input over a bps-stored fee: shows 1.80 for 180 bps, writes back bps on change. */
function PctInput({ bps, onChange }: { bps: number; onChange: (bps: number) => void }) {
  const pct = Number.isFinite(bps) ? Math.round((bps / 100) * 10000) / 10000 : 0
  return (
    <div className="relative">
      <input
        className={`${modalInput} pr-6 text-right tabular-nums`}
        type="number"
        min="0"
        step={0.01}
        value={pct}
        onChange={(e) => {
          const p = parseFloat(e.target.value)
          onChange(Number.isFinite(p) ? Math.round(p * 100 * 100) / 100 : 0)
        }}
      />
      <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-xs text-slate-500">
        %
      </span>
    </div>
  )
}
