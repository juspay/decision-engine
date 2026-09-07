import { RefObject, useLayoutEffect, useState } from 'react'
import { LaneDef } from './model'

interface LanePath {
  d: string
  color: string
  width: number
  winner: boolean
}

interface LaneLabel {
  x: number
  y: number
  kind: 'dot' | 'chip' | 'rank'
  text?: string
  color?: string
}

interface Drawn {
  width: number
  height: number
  paths: LanePath[]
  labels: LaneLabel[]
}

const LANE_X0 = 46
const LANE_STEP = 76

const laneX = (index: number) => LANE_X0 + index * LANE_STEP

function laneWidth(lane: LaneDef) {
  if (lane.share == null) return 2.5
  return Math.min(6, 1.5 + lane.share * 5.5)
}

/**
 * Draws each connector as one continuous ribbon behind the stage cards. The page renders spacer
 * divs marked `data-lane-gap="fan" | "straight" | "converge"` between its rows; this component
 * measures them relative to the container and routes every lane through all of them — curving
 * apart at the fan, running straight through the card regions (where the opaque cards hide them,
 * which is what makes the ribbons read as passing behind), and optionally converging on the
 * configured first choice at the end.
 */
export function LaneCanvas({
  containerRef,
  lanes,
  ghost,
  deterministicHead,
  overflow = 0,
}: {
  containerRef: RefObject<HTMLDivElement>
  lanes: LaneDef[]
  ghost: boolean
  deterministicHead: string | null
  /** Connectors the strategy references beyond the drawn lanes — rendered as a "+N more" chip. */
  overflow?: number
}) {
  const [drawn, setDrawn] = useState<Drawn | null>(null)

  useLayoutEffect(() => {
    const container = containerRef.current
    if (!container) return

    const measure = () => {
      const containerRect = container.getBoundingClientRect()
      const gaps = Array.from(container.querySelectorAll<HTMLElement>('[data-lane-gap]')).map((el) => {
        const rect = el.getBoundingClientRect()
        return {
          kind: el.dataset.laneGap as 'fan' | 'straight' | 'converge',
          top: rect.top - containerRect.top,
          height: rect.height,
        }
      })
      if (!gaps.length || !lanes.length) {
        setDrawn(null)
        return
      }

      const paths: LanePath[] = []
      const labels: LaneLabel[] = []

      lanes.forEach((lane, i) => {
        const x = laneX(i)
        let d = ''
        let alive = true
        let winner = false
        for (const gap of gaps) {
          if (!alive) break
          const y0 = gap.top
          const y1 = gap.top + gap.height
          const mid = y0 + gap.height / 2
          if (gap.kind === 'fan') {
            const originX = 24
            const originY = y0 + 6
            d = `M ${originX} ${originY} C ${originX} ${y0 + gap.height * 0.55}, ${x} ${y0 + gap.height * 0.35}, ${x} ${y1}`
            if (i === 0) labels.push({ x: originX, y: originY, kind: 'dot' })
            labels.push({ x, y: y1 - 22, kind: 'chip', text: lane.name, color: lane.color })
            if (i === lanes.length - 1 && overflow > 0) {
              labels.push({ x: laneX(lanes.length), y: y1 - 22, kind: 'chip', text: `+${overflow} more` })
            }
          } else if (gap.kind === 'converge' && deterministicHead && !ghost) {
            d += ` L ${x} ${y0}`
            if (lane.name === deterministicHead) {
              d += ` C ${x} ${mid}, ${laneX(0)} ${mid}, ${laneX(0)} ${y1 - 8}`
              winner = true
            } else {
              d += ` L ${x} ${y0 + gap.height * 0.42}`
              labels.push({ x, y: y0 + gap.height * 0.5, kind: 'rank', text: `#${i + 1}`, color: lane.color })
              alive = false
            }
          } else {
            // A converge gap without a deterministic winner draws straight through: the winner
            // depends on the payment, and pretending otherwise would be a lie.
            d += ` L ${x} ${y0} L ${x} ${y1}`
          }
        }
        if (d) paths.push({ d, color: lane.color, width: laneWidth(lane) * (winner ? 1.5 : 1), winner })
      })

      setDrawn({
        width: container.clientWidth,
        // Never scrollHeight: the previously-rendered absolute SVG contributes to it, so a
        // collapse after an expansion would ratchet the canvas to the tallest height seen.
        height: containerRect.height,
        paths,
        labels,
      })
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(container)
    return () => observer.disconnect()
  }, [containerRef, lanes, ghost, deterministicHead, overflow])

  if (!drawn) return null

  return (
    <>
      <svg
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-0"
        width={drawn.width}
        height={drawn.height}
        viewBox={`0 0 ${drawn.width} ${drawn.height}`}
      >
        {drawn.paths.map((path, i) => (
          <g key={i}>
            <path d={path.d} fill="none" stroke={path.color} strokeWidth={path.width + 7} strokeLinecap="round" opacity={ghost ? 0.06 : 0.14} />
            <path
              d={path.d}
              fill="none"
              stroke={path.color}
              strokeWidth={path.width}
              strokeLinecap="round"
              opacity={ghost ? 0.55 : 0.95}
              pathLength={100}
              // The draw-in class sets stroke-dasharray:100, which would override the ghost dash
              // pattern (stylesheet beats presentation attribute) — ghost lanes skip the animation.
              className={ghost ? undefined : 'de-lane-draw'}
              style={ghost ? { strokeDasharray: '4 7' } : { animationDelay: `${i * 90}ms` }}
            />
          </g>
        ))}
      </svg>
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-[1]">
        {drawn.labels.map((label, i) => {
          if (label.kind === 'dot') {
            return (
              <span
                key={i}
                className="absolute h-[11px] w-[11px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-brand-500 shadow-[0_0_12px_2px_rgba(59,130,246,0.55)]"
                style={{ left: label.x, top: label.y }}
              />
            )
          }
          if (label.kind === 'chip') {
            return (
              <span
                key={i}
                title={label.text}
                className="absolute flex max-w-[74px] -translate-x-1/2 items-center gap-1.5 rounded-full border border-slate-200 bg-white px-2 py-0.5 font-mono text-[10px] text-slate-600 dark:border-[#1e2535] dark:bg-[#0d1118] dark:text-[#9ca7ba]"
                style={{ left: label.x, top: label.y }}
              >
                {label.color ? (
                  <span className="h-[7px] w-[7px] flex-shrink-0 rounded-[3px]" style={{ background: label.color }} />
                ) : null}
                <span className="min-w-0 truncate">{label.text}</span>
              </span>
            )
          }
          return (
            <span
              key={i}
              className="absolute -translate-x-1/2 font-mono text-[10px]"
              style={{ left: label.x, top: label.y, color: label.color }}
            >
              {label.text}
            </span>
          )
        })}
      </div>
    </>
  )
}
