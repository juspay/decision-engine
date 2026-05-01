interface BadgeProps {
  variant?: 'green' | 'gray' | 'blue' | 'red' | 'orange' | 'purple'
  children: React.ReactNode
}

const variantClasses: Record<string, string> = {
  green:  'bg-emerald-500/10 text-emerald-600 ring-1 ring-inset ring-emerald-500/20 dark:text-emerald-300',
  gray:   'bg-slate-900/[0.04] text-slate-700 ring-1 ring-inset ring-slate-900/8 dark:bg-white/[0.05] dark:text-slate-300 dark:ring-white/8',
  blue:   'bg-sky-500/12 text-sky-700 ring-1 ring-inset ring-sky-500/22 dark:bg-sky-400/14 dark:text-sky-200 dark:ring-sky-400/28',
  red:    'bg-red-500/10 text-red-600 ring-1 ring-inset ring-red-500/20 dark:text-red-300',
  orange: 'bg-orange-500/10 text-orange-600 ring-1 ring-inset ring-orange-500/20 dark:text-orange-300',
  purple: 'bg-purple-500/10 text-purple-600 ring-1 ring-inset ring-purple-500/20 dark:text-purple-300',
}

export function Badge({ variant = 'gray', children }: BadgeProps) {
  return (
    <span
      className={`inline-flex max-w-full min-w-0 items-center gap-1 overflow-hidden rounded-md px-2 py-0.5 text-xs font-medium tracking-wide ${variantClasses[variant]}`}
    >
      <span className="min-w-0 truncate">{children}</span>
    </span>
  )
}
