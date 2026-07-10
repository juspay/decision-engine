import type { ReactNode } from 'react'

/** Connectors whose settlement report can be manually uploaded (their parser is implemented). The
 * automatic path is gated separately — Chase pulls via its poller (OAuth + reporting API), while
 * Braintree's webhook/download is still upload-only. */
export const UPLOAD_CONNECTORS = [
  { value: 'adyen', label: 'Adyen' },
  { value: 'braintree', label: 'Braintree' },
  { value: 'stripe', label: 'Stripe' },
  { value: 'chase', label: 'Chase' },
] as const

/** Shared input styling for the cost-routing forms (credentials + manual upload). */
export const inputClass =
  'w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 ' +
  'placeholder:text-slate-400 focus:border-brand-500 focus:outline-none ' +
  'dark:border-[#232833] dark:bg-[#0b1017] dark:text-white'

/** Labelled form field with an optional hint line, used by both cost-routing forms. */
export function Field({
  label,
  hint,
  children,
}: {
  label: string
  hint?: string
  children: ReactNode
}) {
  return (
    <label className="block space-y-1">
      <span className="text-sm font-medium text-slate-700 dark:text-[#c7cfdd]">{label}</span>
      {children}
      {hint && <span className="block text-xs text-slate-400">{hint}</span>}
    </label>
  )
}
