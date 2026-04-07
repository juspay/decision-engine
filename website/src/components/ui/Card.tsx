import { ReactNode } from 'react'

interface CardProps {
  children: ReactNode
  className?: string
  onClick?: () => void
}

export function Card({ children, className = '', onClick }: CardProps) {
  return (
    <div
      className={`glass-panel rounded-[20px] ${onClick ? 'glass-panel-hover cursor-pointer' : ''} ${className}`}
      onClick={onClick}
      role={onClick ? "button" : undefined}
      tabIndex={onClick ? 0 : undefined}
    >
      {children}
    </div>
  )
}

export function CardHeader({ children, className = '' }: CardProps) {
  return (
    <div className={`px-6 py-5 border-b border-[#1c1c1f] bg-[#0c0c0e] rounded-t-[20px] ${className}`}>
      {children}
    </div>
  )
}

export function CardBody({ children, className = '' }: CardProps) {
  return <div className={`px-6 py-5 ${className}`}>{children}</div>
}
