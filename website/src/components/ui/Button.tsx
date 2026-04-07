import { ButtonHTMLAttributes } from 'react'

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
  size?: 'sm' | 'md'
}

const variantClasses = {
  primary:
    'bg-white text-black hover:bg-[#e4e4e7] disabled:opacity-50 shadow-sm border border-transparent',
  secondary:
    'bg-[#121214] text-[#a1a1aa] border border-[#27272a] hover:bg-[#18181b] hover:text-white disabled:opacity-40',
  ghost:
    'text-[#a1a1aa] hover:text-white hover:bg-[#121214] disabled:opacity-40',
  danger:
    'bg-[#2a0505] text-red-500 hover:bg-[#380808] border border-[#5c1c1c] disabled:opacity-40',
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
      className={`relative inline-flex items-center justify-center gap-2 rounded-full transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-[#3f3f46] focus:border-transparent ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {props.children}
    </button>
  )
}
