import { ReactNode } from 'react'

interface CardProps {
  children: ReactNode
  className?: string
  onClick?: () => void
}

export function Card({ children, className = '', onClick }: CardProps) {
  const baseClassName =
    `relative overflow-hidden rounded-[30px] border border-slate-200 bg-white shadow-[0_18px_60px_-42px_rgba(15,23,42,0.15)] dark:border-[#2a303a] dark:bg-[#11151d] dark:shadow-[0_18px_60px_-42px_rgba(0,0,0,0.7)] ${onClick ? 'cursor-pointer text-left transition duration-300 hover:-translate-y-0.5 hover:border-[#3b82f6]/35 hover:bg-slate-50 dark:hover:bg-[#141923]' : ''} ${className}`

  const inner = (
    <>
      <div className="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-[#3b82f6]/25 to-transparent dark:via-[#3b82f6]/30" />
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,0.55),transparent_26%)] dark:bg-[linear-gradient(180deg,rgba(255,255,255,0.02),transparent_26%)]" />
      <div className="relative">{children}</div>
    </>
  )

  if (onClick) {
    return (
      <button type="button" onClick={onClick} className={baseClassName}>
        {inner}
      </button>
    )
  }

  return (
    <div className={baseClassName}>{inner}</div>
  )
}

export function CardHeader({ children, className = '' }: CardProps) {
  return (
    <div className={`border-b border-slate-200 px-6 py-5 dark:border-[#2a303a] ${className}`}>
      {children}
    </div>
  )
}

export function CardBody({ children, className = '' }: CardProps) {
  return <div className={`px-6 py-5 ${className}`}>{children}</div>
}

export function SurfaceLabel({ children, className = '' }: CardProps) {
  return (
    <p className={`text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-500 dark:text-[#8390a7] ${className}`}>
      {children}
    </p>
  )
}

export function InsetPanel({ children, className = '' }: CardProps) {
  return (
    <div
      className={`rounded-[22px] border border-slate-200 bg-white/80 px-4 py-4 shadow-[0_14px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#161b24] dark:shadow-none ${className}`}
    >
      {children}
    </div>
  )
}
