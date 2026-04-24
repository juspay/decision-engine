import { ButtonHTMLAttributes } from 'react'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
}

const variantClasses = {
  primary:
    'bg-brand-600 text-white hover:bg-brand-700 disabled:opacity-50 shadow-sm border border-transparent',
  secondary:
    'bg-white text-slate-700 border border-[#e6e6ee] hover:bg-slate-50 hover:text-slate-900 disabled:opacity-40 shadow-sm dark:bg-[#121218] dark:text-[#a1a1aa] dark:border-[#1a1a24] dark:hover:bg-[#18181f] dark:hover:text-white',
  ghost:
    'text-slate-500 hover:text-slate-900 hover:bg-slate-100 disabled:opacity-40 dark:text-[#a1a1aa] dark:hover:text-white dark:hover:bg-[#13131a]',
  danger:
    'bg-red-50 text-red-600 hover:bg-red-100 border border-red-200 disabled:opacity-40 dark:bg-[#2a0505] dark:text-red-500 dark:hover:bg-[#380808] dark:border-[#5c1c1c]',
}

const sizeClasses = {
  sm: 'px-3.5 py-1.5 text-xs font-medium',
  md: 'px-4 py-2 text-sm font-medium',
}

export function Button({
  variant = 'primary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps) {
  return (
    <button
      className={`relative inline-flex items-center justify-center gap-2 rounded-lg transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-brand-500/30 focus:ring-offset-1 ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {props.children}
    </button>
  )
}
