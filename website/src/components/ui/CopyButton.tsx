import { useState } from 'react'
import { Copy, Check } from 'lucide-react'

interface CopyButtonProps {
  text: string
  label?: string
  size?: number
  className?: string
}

export function CopyButton({ text, label, size = 10, className = '' }: CopyButtonProps) {
  const [copied, setCopied] = useState(false)

  function handleCopy(e: React.MouseEvent) {
    e.stopPropagation()
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <button
      type="button"
      onClick={handleCopy}
      title={copied ? 'Copied!' : (label ?? 'Copy to clipboard')}
      className={`shrink-0 transition-colors text-slate-300 hover:text-slate-500 dark:text-[#3a4258] dark:hover:text-[#8090a8] ${className}`}
    >
      {copied
        ? <Check size={size} className="text-emerald-500" />
        : <Copy size={size} />
      }
    </button>
  )
}
