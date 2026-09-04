import { useCallback, useEffect, useRef, useState, type RefObject } from 'react'

// The header is 78px of the viewport that never earns its keep while you are reading down a
// page. This is the "scroll-away app bar" pattern: it slides out on the way down and comes
// straight back on the way up, so the control you want is one flick away rather than behind an
// invisible edge trigger.
const HEADER_HEIGHT = 78

// Scrolling is noisy — trackpad inertia and rubber-banding both emit small deltas in the wrong
// direction. Requiring a few pixels of committed movement stops the bar from flickering.
const DOWN_DELTA = 6
const UP_DELTA = 4

interface HeaderAutoHide<S extends HTMLElement, H extends HTMLElement> {
  scrollRef: RefObject<S>
  headerRef: RefObject<H>
  hidden: boolean
}

/**
 * Hides a sticky header while the attached scroll container moves down, and reveals it on any
 * upward movement. The header is always shown within its own height of the top, and is forced
 * back whenever focus sits inside it — a keyboard user tabbing into search must not be chasing
 * a bar that has slid off screen, and a click-opened popover keeps its anchor on screen for the
 * same reason.
 */
export function useHeaderAutoHide<
  S extends HTMLElement = HTMLDivElement,
  H extends HTMLElement = HTMLElement,
>(): HeaderAutoHide<S, H> {
  const scrollRef = useRef<S>(null)
  const headerRef = useRef<H>(null)
  const lastY = useRef(0)
  const frame = useRef<number | null>(null)
  const [hidden, setHidden] = useState(false)

  const holdsFocus = useCallback(
    () => !!headerRef.current && headerRef.current.contains(document.activeElement),
    [],
  )

  useEffect(() => {
    const el = scrollRef.current
    if (!el) return

    function evaluate() {
      frame.current = null
      const node = scrollRef.current
      if (!node) return

      const y = node.scrollTop
      const delta = y - lastY.current

      if (y <= HEADER_HEIGHT || holdsFocus()) {
        setHidden(false)
      } else if (delta > DOWN_DELTA) {
        setHidden(true)
      } else if (delta < -UP_DELTA) {
        setHidden(false)
      }

      // Only advance the reference point once a gesture has cleared a threshold, so slow drags
      // still accumulate into a decision instead of being read as a series of zero-deltas.
      if (Math.abs(delta) > DOWN_DELTA) lastY.current = y
    }

    function onScroll() {
      if (frame.current !== null) return
      frame.current = requestAnimationFrame(evaluate)
    }

    lastY.current = el.scrollTop
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      el.removeEventListener('scroll', onScroll)
      if (frame.current !== null) cancelAnimationFrame(frame.current)
    }
  }, [holdsFocus])

  // Tabbing into the header from the page below reveals it without needing a scroll first.
  useEffect(() => {
    function onFocusIn() {
      if (holdsFocus()) setHidden(false)
    }
    document.addEventListener('focusin', onFocusIn)
    return () => document.removeEventListener('focusin', onFocusIn)
  }, [holdsFocus])

  return { scrollRef, headerRef, hidden }
}
