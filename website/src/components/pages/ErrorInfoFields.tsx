import { useMemo, useState } from 'react'
import { RefreshCw } from 'lucide-react'
import { Combobox } from '../ui/Combobox'
import { Tooltip } from '../ui/Tooltip'

export interface ErrorInfoState {
  connector: string
  flow: string
  sub_flow: string
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
}

export const DEFAULT_ERROR_INFO: ErrorInfoState = {
  connector: '',
  flow: '',
  sub_flow: '',
  error_code: '',
  error_message: '',
  issuer_error_code: '',
  card_network: '',
}

interface ErrorInfoFieldsProps {
  info: ErrorInfoState
  onChange: (updates: Partial<ErrorInfoState>) => void
  rules: GsmOptionRow[]
}

const inputClass =
  'w-full border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500'
const labelClass = 'block text-xs font-medium text-slate-600 dark:text-slate-400 mb-1'

export function ErrorInfoFields({ info, onChange, rules }: ErrorInfoFieldsProps) {
  const [open, setOpen] = useState(false)

  const connectors = useMemo(
    () => [...new Set(rules.map(r => r.connector))].sort(),
    [rules],
  )

  const flows = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r => !info.connector || r.connector === info.connector)
          .map(r => r.flow),
      )].sort(),
    [rules, info.connector],
  )

  const subFlows = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r =>
            (!info.connector || r.connector === info.connector) &&
            (!info.flow || r.flow === info.flow),
          )
          .map(r => r.subFlow),
      )].sort(),
    [rules, info.connector, info.flow],
  )

  const errorCodes = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r =>
            (!info.connector || r.connector === info.connector) &&
            (!info.flow || r.flow === info.flow) &&
            (!info.sub_flow || r.subFlow === info.sub_flow),
          )
          .map(r => r.errorCode)
          .filter(Boolean),
      )].sort(),
    [rules, info.connector, info.flow, info.sub_flow],
  )

  const errorMessages = useMemo(
    () =>
      [...new Set(
        rules
          .filter(r =>
            (!info.connector || r.connector === info.connector) &&
            (!info.flow || r.flow === info.flow) &&
            (!info.sub_flow || r.subFlow === info.sub_flow) &&
            (!info.error_code || r.errorCode === info.error_code),
          )
          .map(r => r.errorMessage)
          .filter(Boolean),
      )].sort(),
    [rules, info.connector, info.flow, info.sub_flow, info.error_code],
  )

  return (
    <div className="border-t border-slate-200 dark:border-[#1c1c24] pt-3 mt-1">
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={() => setOpen(o => !o)}
          className="flex flex-1 items-center justify-between text-xs font-medium text-slate-700 dark:text-slate-300 hover:text-slate-900 dark:hover:text-slate-100"
        >
          <span>
            Error Info{' '}
            <span className="font-normal text-slate-400">
              (connector + flow + sub-flow required for GSM lookup)
            </span>
          </span>
          <svg
            className={`h-3.5 w-3.5 shrink-0 text-slate-400 transition-transform duration-200 ${open ? 'rotate-180' : ''}`}
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path fillRule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clipRule="evenodd" />
          </svg>
        </button>
        {Object.values(info).some(Boolean) && (
          <Tooltip text="Clear all fields">
            <button
              type="button"
              onClick={() => onChange({
                connector: '', flow: '', sub_flow: '', error_code: '',
                error_message: '', issuer_error_code: '', card_network: '',
              })}
              className="ml-1 rounded p-0.5 text-slate-400 hover:bg-slate-100 dark:hover:bg-[#1c1c24] hover:text-slate-600 dark:hover:text-slate-200"
            >
              <RefreshCw size={12} />
            </button>
          </Tooltip>
        )}
      </div>

      {open && <div className="space-y-2 mt-2">
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className={labelClass}>Connector</label>
          <Combobox
            value={info.connector}
            onChange={v => onChange({
              connector: v,
              flow: '', sub_flow: '', error_code: '', error_message: '',
            })}
            options={connectors}
            placeholder="adyen"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>Flow</label>
          <Combobox
            value={info.flow}
            onChange={v => onChange({
              flow: v,
              sub_flow: '', error_code: '', error_message: '',
            })}
            options={flows}
            placeholder="Payment"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>Sub-flow</label>
          <Combobox
            value={info.sub_flow}
            onChange={v => onChange({
              sub_flow: v,
              error_code: '', error_message: '',
            })}
            options={subFlows}
            placeholder="Authorize"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>Card Network</label>
          <input
            value={info.card_network}
            onChange={e => onChange({ card_network: e.target.value })}
            placeholder="Visa"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>Error Code</label>
          <Combobox
            value={info.error_code}
            onChange={v => onChange({ error_code: v, error_message: '' })}
            options={errorCodes}
            placeholder="declined"
            className={inputClass}
          />
        </div>
        <div>
          <label className={labelClass}>Issuer Error Code</label>
          <input
            value={info.issuer_error_code}
            onChange={e => onChange({ issuer_error_code: e.target.value })}
            placeholder="51"
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
          placeholder="Insufficient funds"
          className={inputClass}
        />
      </div>
      </div>}
    </div>
  )
}
