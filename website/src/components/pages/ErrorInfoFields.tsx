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
  unifiedCode?: string
  unifiedMessage?: string
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
  showClassification?: boolean
}

const inputClass =
  'w-full bg-slate-50 dark:bg-[#0d0d13] border border-slate-200 dark:border-[#222226] rounded-lg px-2.5 py-1.5 text-xs focus:outline-none focus:ring-1 focus:ring-brand-500 text-slate-800 dark:text-slate-100 placeholder-slate-400'
const labelClass = 'block text-[10px] font-semibold uppercase tracking-wider text-slate-400 dark:text-slate-500 mb-1'

export function penalizedByUnifiedMessage(message: string | undefined | null): boolean | null {
  switch (message) {
    // UE_1000 — card/user error (expired card, wrong CVV, FRM decline).
    // Gateway processed correctly; penalty is skipped on the backend.
    case 'Issue with Payment Method details':
      return false
    // UE_2000 / UE_3000 / UE_4000 — config, PSP, or integration fault.
    // Backend applies full penalty for all of these.
    case 'Issue with Integration':
    case 'Issue with Configurations':
    case 'Technical issue with PSP':
    case 'Something went wrong':
      return true
    default:
      return null
  }
}

function decisionBadgeClass(decision: string) {
  switch (decision.toLowerCase()) {
    case 'decline': return 'bg-red-50 text-red-600 dark:bg-red-950/40 dark:text-red-400'
    case 'retry': return 'bg-amber-50 text-amber-600 dark:bg-amber-950/40 dark:text-amber-400'
    case 'step_up':
    case 'challenge': return 'bg-blue-50 text-blue-600 dark:bg-blue-950/40 dark:text-blue-400'
    case 'success':
    case 'force_2ds_pass': return 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300'
    default: return 'bg-slate-100 text-slate-500 dark:bg-[#1a1a24] dark:text-slate-400'
  }
}

export function ErrorInfoFields({ info, onChange, rules, connector, showClassification = true }: ErrorInfoFieldsProps) {
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

  const classification = useMemo(() => {
    if (!info.error_code) return null
    // Prefer subFlow=Authorize (payment auth path, newer rules with unified data)
    // over flow=Authorize (general auth flow) to resolve duplicate error codes correctly.
    const rule =
      rules.find(r => (!connector || r.connector === connector) && r.errorCode === info.error_code && r.subFlow === 'Authorize') ??
      rules.find(r => (!connector || r.connector === connector) && r.errorCode === info.error_code && r.flow === 'Authorize') ??
      rules.find(r => (!connector || r.connector === connector) && r.errorCode === info.error_code)
    if (!rule) return null
    return {
      unifiedMessage: rule.unifiedMessage ?? null,
      decision: rule.decision || null,
      penalized: penalizedByUnifiedMessage(rule.unifiedMessage),
    }
  }, [rules, connector, info.error_code])

  const hasValues = Object.values(info).some(Boolean)

  return (
    <div className="rounded-lg border border-slate-200 dark:border-[#222226] overflow-hidden">
      {/* Header row */}
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        className="w-full flex items-center justify-between gap-2 px-2.5 py-2 bg-slate-50 dark:bg-[#0d0d13] hover:bg-slate-100 dark:hover:bg-[#111118] transition-colors"
      >
        <div className="flex items-center gap-1.5 min-w-0 flex-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400 shrink-0">
            Decline Error
          </span>
          {info.error_code ? (
            <span className="font-mono text-[10px] text-brand-600 dark:text-brand-400 bg-brand-50 dark:bg-brand-900/30 px-1.5 py-0.5 rounded shrink-0">
              {info.error_code}
            </span>
          ) : (
            <span className="text-[10px] text-slate-400 dark:text-slate-600">not set</span>
          )}
          {showClassification && classification?.decision && (
            <span className={`inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-semibold shrink-0 ${decisionBadgeClass(classification.decision)}`}>
              {classification.decision}
            </span>
          )}
          {showClassification && classification?.penalized != null && (
            <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-semibold shrink-0 ${
              classification.penalized
                ? 'bg-red-50 text-red-600 dark:bg-red-950/40 dark:text-red-400'
                : 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300'
            }`}>
              <span className={`h-1.5 w-1.5 rounded-full shrink-0 ${classification.penalized ? 'bg-red-500' : 'bg-emerald-500'}`} />
              {classification.penalized ? 'Penalized' : 'Penalty skipped'}
            </span>
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
          {/* Unified message strip */}
          {classification?.unifiedMessage && (
            <div className={`flex items-center gap-2 px-2.5 py-1.5 rounded-lg border-l-2 text-[10px] ${
              classification.penalized === true
                ? 'bg-red-50 border-red-400 text-red-700 dark:bg-red-950/20 dark:border-red-600 dark:text-red-400'
                : classification.penalized === false
                  ? 'bg-emerald-50 border-emerald-400 text-emerald-700 dark:bg-emerald-950/20 dark:border-emerald-600 dark:text-emerald-300'
                  : 'bg-slate-50 border-slate-300 text-slate-500 dark:bg-[#111118] dark:border-slate-600 dark:text-slate-400'
            }`}>
              <span>{classification.unifiedMessage}</span>
            </div>
          )}

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
