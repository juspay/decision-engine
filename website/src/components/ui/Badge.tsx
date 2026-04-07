interface BadgeProps {
  variant?: 'green' | 'gray' | 'blue' | 'red' | 'orange' | 'purple'
  children: React.ReactNode
}

const variantClasses: Record<string, string> = {
  green:  'bg-emerald-500/10 text-emerald-400 ring-1 ring-inset ring-emerald-500/20',
  gray:   'bg-white/5 text-gray-600 ring-1 ring-inset ring-white/8',
  blue:   'bg-blue-500/10 text-blue-400 ring-1 ring-inset ring-blue-500/20',
  red:    'bg-red-500/10 text-red-400 ring-1 ring-inset ring-red-500/20',
  orange: 'bg-orange-500/10 text-orange-400 ring-1 ring-inset ring-orange-500/20',
  purple: 'bg-purple-500/10 text-purple-400 ring-1 ring-inset ring-purple-500/20',
}

export function Badge({ variant = 'gray', children }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md text-xs font-medium tracking-wide ${variantClasses[variant]}`}
    >
      {children}
    </span>
  )
}
