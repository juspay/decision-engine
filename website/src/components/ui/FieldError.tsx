import { AlertCircle } from 'lucide-react'

/**
 * A validation message for the control it sits under. Errors belong beside the field that caused
 * them, not collected at the foot of the form where the reader has to work out which input to fix.
 */
export function FieldError({ message }: { message?: string | null }) {
  if (!message) return null

  return (
    <p role="alert" className="mt-1.5 flex items-start gap-1.5 text-[13px] text-red-600 dark:text-red-400">
      <AlertCircle size={14} className="mt-px shrink-0" />
      <span>{message}</span>
    </p>
  )
}

/** Border and focus ring for an input that failed validation. */
export function invalidFieldClass(hasError: boolean) {
  return hasError
    ? '!border-red-400 focus:!ring-red-400 dark:!border-red-500/60'
    : ''
}
