import { ButtonHTMLAttributes } from 'react'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
}

const variantClasses = {
  primary:
    'bg-brand-500 text-white hover:bg-brand-600 disabled:opacity-40 shadow-[0_0_20px_rgba(22,104,227,0.25)] hover:shadow-[0_0_24px_rgba(22,104,227,0.35)]',
  secondary:
    'bg-[#111118] text-gray-800 border border-[#28282f] hover:bg-[#18181f] hover:border-[#35353e] disabled:opacity-40',
  ghost:
    'text-gray-600 hover:text-gray-800 hover:bg-[#111118] disabled:opacity-40',
  danger:
    'bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/15 disabled:opacity-40',
}

const sizeClasses = {
  sm: 'px-3 py-1.5 text-xs',
  md: 'px-4 py-2 text-sm',
}

export function Button({
  variant = 'primary',
  size = 'md',
  className = '',
  ...props
}: ButtonProps) {
  return (
    <button
      className={`inline-flex items-center gap-1.5 rounded-lg font-medium transition-all duration-150 focus:outline-none focus:ring-2 focus:ring-brand-500/40 focus:ring-offset-1 focus:ring-offset-[#07070b] ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    />
  )
}
