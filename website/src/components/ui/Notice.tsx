import type { ReactNode } from 'react'
import { AlertTriangle, CheckCircle2, Info, XCircle } from 'lucide-react'

export type NoticeTone = 'info' | 'success' | 'warning' | 'danger'

interface NoticeProps {
  /** What the message is: neutral context, a good outcome, a caution, or a failure. */
  tone?: NoticeTone
  children: ReactNode
  className?: string
}

const TONES: Record<NoticeTone, { box: string; icon: string; Glyph: typeof Info }> = {
  info: {
    box: 'border-slate-200 bg-slate-50 text-slate-700 dark:border-[#2a303a] dark:bg-[#111820] dark:text-[#c7cfdd]',
    icon: 'text-slate-500 dark:text-[#8d96aa]',
    Glyph: Info,
  },
  success: {
    box: 'border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300',
    icon: 'text-emerald-700 dark:text-emerald-400',
    Glyph: CheckCircle2,
  },
  warning: {
    box: 'border-amber-200 bg-amber-50 text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300',
    icon: 'text-amber-700 dark:text-amber-400',
    Glyph: AlertTriangle,
  },
  danger: {
    box: 'border-red-200 bg-red-50 text-red-800 dark:border-red-500/30 dark:bg-red-500/10 dark:text-red-300',
    icon: 'text-red-700 dark:text-red-400',
    Glyph: XCircle,
  },
}

/**
 * A short message about the state of the screen — a conflict, a result, a prerequisite.
 *
 * Set at 13px with the same padding and leading as body copy, because a notice is read rather
 * than glanced at; the tone carries the urgency, so the text does not have to shrink to signal
 * that it is secondary. The glyph is decorative — the sentence must stand on its own for anyone
 * who cannot see the colour.
 */
export function Notice({ tone = 'info', children, className = '' }: NoticeProps) {
  const { box, icon, Glyph } = TONES[tone]
  return (
    <div
      role="status"
      className={`flex items-start gap-2.5 rounded-lg border px-4 py-3 text-[13px] leading-[18px] ${box} ${className}`}
    >
      <Glyph size={16} aria-hidden className={`mt-px shrink-0 ${icon}`} />
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  )
}
