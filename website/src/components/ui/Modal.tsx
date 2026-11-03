import { useEffect, useRef, type ReactNode, type RefObject } from 'react'
import { createPortal } from 'react-dom'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

interface ModalProps {
  /** Id of the element that names the dialog, usually its heading. */
  labelledBy: string
  onClose: () => void
  /** While false, Escape and the backdrop leave the dialog open — for work that is in flight. */
  dismissible?: boolean
  /** Focused when the dialog opens; the panel itself when absent. */
  initialFocus?: RefObject<HTMLElement | null>
  children: ReactNode
}

/**
 * The shell every dialog sits in: a backdrop over the whole app, and a panel that holds keyboard
 * focus while it is open and hands it back to whatever had it when it closes.
 *
 * Rendered at the document root: inside the page it would sit in the content column's stacking
 * context, below the sidebar, and its backdrop could not cover it.
 */
export function Modal({ labelledBy, onClose, dismissible = true, initialFocus, children }: ModalProps) {
  const panelRef = useRef<HTMLDivElement>(null)
  // Read through refs, so a caller passing a fresh callback each render does not re-run the
  // effect below and pull focus back to the start.
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose
  const dismissibleRef = useRef(dismissible)
  dismissibleRef.current = dismissible

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    ;(initialFocus?.current ?? panelRef.current)?.focus()

    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        if (dismissibleRef.current) onCloseRef.current()
        return
      }
      const panel = panelRef.current
      if (e.key !== 'Tab' || !panel) return
      const focusable = Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE))
      if (focusable.length === 0) {
        e.preventDefault()
        panel.focus()
        return
      }
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const inside = panel.contains(document.activeElement)
      if (e.shiftKey && (!inside || document.activeElement === first || document.activeElement === panel)) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && (!inside || document.activeElement === last)) {
        e.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('keydown', onKey)
      if (opener?.isConnected) opener.focus()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- runs once per opening
  }, [])

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-[2px]"
        onClick={() => {
          if (dismissibleRef.current) onCloseRef.current()
        }}
      />
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
        tabIndex={-1}
        className="relative w-full max-w-sm rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl outline-none dark:border-[#2a303a] dark:bg-[#0d1118]"
      >
        {children}
      </div>
    </div>,
    document.body,
  )
}
