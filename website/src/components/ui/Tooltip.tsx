import { useState, useRef, useEffect } from 'react'
import { createPortal } from 'react-dom'

interface TooltipProps {
  text: string
  children: React.ReactElement
}

export function Tooltip({ text, children }: TooltipProps) {
  const [visible, setVisible] = useState(false)
  const [style, setStyle] = useState<React.CSSProperties>({})
  const ref = useRef<HTMLSpanElement>(null)

  function show() {
    const rect = ref.current?.getBoundingClientRect()
    if (!rect) return
    setStyle({
      position: 'fixed',
      top: rect.top - 6,
      left: rect.left + rect.width / 2,
      transform: 'translate(-50%, -100%)',
      zIndex: 9999,
    })
    setVisible(true)
  }

  useEffect(() => {
    if (!visible) return
    function hide() { setVisible(false) }
    document.addEventListener('scroll', hide, true)
    return () => document.removeEventListener('scroll', hide, true)
  }, [visible])

  return (
    <span
      ref={ref}
      onMouseEnter={show}
      onMouseLeave={() => setVisible(false)}
      className="inline-flex"
    >
      {children}
      {visible && createPortal(
        <span
          style={style}
          className="pointer-events-none rounded-md bg-slate-800 dark:bg-slate-700 px-2 py-1 text-[11px] text-white shadow-md whitespace-nowrap"
        >
          {text}
        </span>,
        document.body,
      )}
    </span>
  )
}
