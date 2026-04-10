import { ButtonHTMLAttributes } from 'react'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
}

const variantClasses = {
  primary:
    'bg-brand-600 text-white hover:bg-brand-700 disabled:opacity-50 shadow-sm border border-transparent dark:bg-white dark:text-black dark:hover:bg-slate-200',
  secondary:
    'bg-white text-slate-700 border border-slate-200 hover:bg-slate-50 hover:text-slate-900 disabled:opacity-40 shadow-sm dark:bg-[#121214] dark:text-[#a1a1aa] dark:border-[#27272a] dark:hover:bg-[#18181b] dark:hover:text-white',
  ghost:
    'text-slate-500 hover:text-slate-900 hover:bg-slate-100 disabled:opacity-40 dark:text-[#a1a1aa] dark:hover:text-white dark:hover:bg-[#121214]',
  danger:
    'bg-red-50 text-red-600 hover:bg-red-100 border border-red-200 disabled:opacity-40 dark:bg-[#2a0505] dark:text-red-500 dark:hover:bg-[#380808] dark:border-[#5c1c1c]',
}

const sizeClasses = {
  sm: 'px-4 py-1.5 text-xs font-semibold',
  md: 'px-5 py-2.5 text-sm font-semibold',
}

export function Button({
  variant = 'primary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps) {
  return (
    <button
      className={`relative inline-flex items-center justify-center gap-2 rounded-full transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-brand-500/50 focus:ring-offset-1 focus:ring-offset-transparent focus:border-transparent ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {props.children}
    </button>
  )
}
