import { useMemo, useState } from 'react'
import { RotateCcw, ChevronDown } from 'lucide-react'
import { Combobox } from '../ui/Combobox'
import { Tooltip } from '../ui/Tooltip'

export interface ErrorInfoState {
  error_code: string
  error_message: string
  issuer_error_code: string
  card_network: string
}

export interface GsmOptionRow {
  connector: string
  flow: string
  subFlow: string
  errorCode: string
  errorMessage: string
  errorCategory?: string
  decision: string
}

export const DEFAULT_ERROR_INFO: ErrorInfoState = {
  error_code: '',
  error_message: '',
  issuer_error_code: '',
  card_network: '',
}

interface ErrorInfoFieldsProps {
  info: ErrorInfoState
  onChange: (updates: Partial<ErrorInfoState>) => void
  rules: GsmOptionRow[]
  connector?: string
}

const inputClass =
  'w-full bg-slate-50 dark:bg-[#0d0d13] border border-slate-200 dark:border-[#222226] rounded-lg px-2.5 py-1.5 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500 text-slate-800 dark:text-slate-100 placeholder-slate-400'
const labelClass = 'block text-[10px] font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-1'

export function ErrorInfoFields({ info, onChange, rules, connector }: ErrorInfoFieldsProps) {
  const [open, setOpen] = useState(false)

  const errorCodes = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r => !connector || r.connector === connector)
          .map(r => r.errorCode)
          .filter(Boolean),
      )].sort(),
    [rules, connector],
  )

  const errorMessages = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r =>
            (!connector || r.connector === connector) &&
            (!info.error_code || r.errorCode === info.error_code),
          )
          .map(r => r.errorMessage)
          .filter(Boolean),
      )].sort(),
    [rules, connector, info.error_code],
  )

  const hasValues = Object.values(info).some(Boolean)

  return (
    <div className="rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden">
      {/* Header row */}
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        className="w-full flex items-center justify-between px-2.5 py-2 bg-slate-50 dark:bg-[#0d0d13] hover:bg-slate-100 dark:hover:bg-[#111118] transition-colors"
      >
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 shrink-0">
            Decline Error
          </span>
          {hasValues && info.error_code && (
            <span className="truncate font-mono text-[10px] text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900/30 px-1.5 py-0.5 rounded">
              {info.error_code}
            </span>
          )}
          {!hasValues && (
            <span className="text-[10px] text-slate-400 dark:text-slate-600">not set</span>
          )}
        </div>
        <div className="flex items-center gap-1.5 shrink-0">
          {hasValues && (
            <Tooltip text="Clear error info">
              <span
                role="button"
                onClick={e => { e.stopPropagation(); onChange({ error_code: '', error_message: '', issuer_error_code: '', card_network: '' }) }}
                className="rounded p-0.5 text-slate-400 hover:text-red-400 transition-colors"
              >
                <RotateCcw size={11} />
              </span>
            </Tooltip>
          )}
          <ChevronDown size={13} className={`text-slate-400 transition-transform duration-200 ${open ? 'rotate-180' : ''}`} />
        </div>
      </button>

      {/* Expanded fields */}
      {open && (
        <div className="px-2.5 py-2.5 space-y-2 border-t border-slate-200 dark:border-[#222226]">
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className={labelClass}>Error Code</label>
              <Combobox
                value={info.error_code}
                onChange={v => onChange({ error_code: v, error_message: '' })}
                options={errorCodes}
                placeholder="e.g. declined"
                className={inputClass}
              />
            </div>
            <div>
              <label className={labelClass}>Issuer Code</label>
              <input
                value={info.issuer_error_code}
                onChange={e => onChange({ issuer_error_code: e.target.value })}
                placeholder="e.g. 51"
                className={inputClass}
              />
            </div>
          </div>
          <div>
            <label className={labelClass}>Error Message</label>
            <Combobox
              value={info.error_message}
              onChange={v => onChange({ error_message: v })}
              options={errorMessages}
              placeholder="e.g. Insufficient funds"
              className={inputClass}
            />
          </div>
          <div>
            <label className={labelClass}>Card Network</label>
            <input
              value={info.card_network}
              onChange={e => onChange({ card_network: e.target.value })}
              placeholder="e.g. Visa"
              className={inputClass}
            />
          </div>
        </div>
      )}
    </div>
  )
}
