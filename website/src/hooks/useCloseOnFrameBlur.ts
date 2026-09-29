import { useEffect, useRef } from 'react'
import { isEmbedded } from '../lib/embedMode'

/**
 * Close a floating layer when the iframe loses focus.
 *
 * Outside-click dismissal listens on this document, which never sees clicks landing on the
 * dashboard's own chrome around the frame. Embedded, that leaves a menu hanging open over the
 * page while the user is already working somewhere else.
 *
 * Armed only in embed mode: standalone, a window blur means the user switched tab or app, and
 * every other dropdown on the web stays open across that.
 */
export function useCloseOnFrameBlur(open: boolean, close: () => void): void {
  // Keep the latest callback without resubscribing when an inline arrow changes identity.
  const closeRef = useRef(close)
  closeRef.current = close

  useEffect(() => {
    if (!open || !isEmbedded()) return
    const onBlur = () => closeRef.current()
    window.addEventListener('blur', onBlur)
    return () => window.removeEventListener('blur', onBlur)
  }, [open])
}
