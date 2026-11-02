import { useState } from 'react'
import { Plus, X } from 'lucide-react'
import { useDynamicRoutingConfig } from '../../hooks/useDynamicRoutingConfig'
import { SearchableSelect } from '../../components/ui/SearchableSelect'

/**
 * The audit list's payment field filter: routing keys (`card_bin`, `billing_country`, …) and
 * metadata keys (`metadata.segment`), each matched against the value a payment's routing requests
 * carried. It travels as the `payment_fields` param, a JSON object of field names to values.
 */

const METADATA_PREFIX = 'metadata.'
const METADATA_OPTION = '__metadata'

export function parsePaymentFields(raw: string): Record<string, string> {
  if (!raw.trim()) return {}
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {}
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>)
        .filter((entry): entry is [string, string] => typeof entry[1] === 'string')
        .map(([field, value]) => [field.trim(), value.trim()])
        .filter(([field, value]) => field && value),
    )
  } catch {
    return {}
  }
}

/** The param value: '' when no field is set, so an unfiltered URL carries no `payment_fields`. */
export function serializePaymentFields(fields: Record<string, string>): string {
  return Object.keys(fields).length ? JSON.stringify(fields) : ''
}

type Props = {
  value: string
  onApply: (next: string) => void
  fieldClassName: (active?: boolean) => string
}

export function PaymentFieldFilter({ value, onApply, fieldClassName }: Props) {
  const fields = parsePaymentFields(value)
  const { keys } = useDynamicRoutingConfig()
  const [field, setField] = useState('')
  const [metadataKey, setMetadataKey] = useState('')
  const [fieldValue, setFieldValue] = useState('')

  // Amounts filter by the min/max inputs, and `metadata` is matched by key.
  const routingKeys = keys.filter((key) => key.type !== 'integer' && key.type !== 'udf')
  const options = [
    ...routingKeys.map((key) => ({ value: key.key, label: key.key })),
    { value: METADATA_OPTION, label: 'Metadata key…' },
  ]
  const enumValues = keys.find((key) => key.key === field)?.values ?? []
  const name = field === METADATA_OPTION ? (metadataKey.trim() ? `${METADATA_PREFIX}${metadataKey.trim()}` : '') : field
  const canAdd = Boolean(name && fieldValue.trim())

  function add() {
    if (!canAdd) return
    onApply(serializePaymentFields({ ...fields, [name]: fieldValue.trim() }))
    setFieldValue('')
    setMetadataKey('')
  }

  function remove(removed: string) {
    const next = { ...fields }
    delete next[removed]
    onApply(serializePaymentFields(next))
  }

  return (
    <div className="col-span-full flex flex-wrap items-center gap-3" data-testid="payment-field-filter">
      <div className="w-56">
        <SearchableSelect
          value={field}
          options={options}
          onChange={setField}
          triggerClassName={fieldClassName(false)}
          dataCy="payment-field-name"
          placeholder="Payment field"
        />
      </div>
      {field === METADATA_OPTION ? (
        <input
          className={`${fieldClassName(false)} !w-48`}
          value={metadataKey}
          onChange={(event) => setMetadataKey(event.target.value)}
          placeholder="Metadata key"
          aria-label="Metadata key"
        />
      ) : null}
      <input
        className={`${fieldClassName(false)} !w-56`}
        value={fieldValue}
        list={enumValues.length ? 'payment-field-values' : undefined}
        onChange={(event) => setFieldValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key !== 'Enter') return
          // The bar's form submits on Enter; this field adds itself instead.
          event.preventDefault()
          add()
        }}
        placeholder={field ? 'Value' : 'Pick a payment field'}
        aria-label="Payment field value"
        disabled={!field}
      />
      {enumValues.length ? (
        <datalist id="payment-field-values">
          {enumValues.map((option) => (
            <option key={option} value={option} />
          ))}
        </datalist>
      ) : null}
      <button
        type="button"
        onClick={add}
        disabled={!canAdd}
        className="flex h-11 items-center gap-1.5 rounded-2xl border border-slate-200 px-4 text-[13px] font-medium text-slate-700 transition hover:border-brand-500 disabled:cursor-not-allowed disabled:opacity-50 dark:border-[#2a303a] dark:text-[#e5ecf7]"
      >
        <Plus className="h-4 w-4" />
        Add field
      </button>
      {Object.entries(fields).map(([name, fieldValue]) => (
        <span
          key={name}
          className="flex h-8 items-center gap-1.5 rounded-full border border-brand-300 bg-brand-50 pl-3 pr-1.5 text-[12px] font-medium text-brand-700 dark:border-brand-500/50 dark:bg-brand-500/10 dark:text-brand-300"
        >
          {name} = {fieldValue}
          <button
            type="button"
            onClick={() => remove(name)}
            aria-label={`Remove ${name} filter`}
            className="rounded-full p-0.5 hover:bg-brand-100 dark:hover:bg-brand-500/20"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </span>
      ))}
    </div>
  )
}
