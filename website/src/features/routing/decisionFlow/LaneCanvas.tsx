import { RefObject, useLayoutEffect, useState } from 'react'
import { LaneDef } from './model'

interface LanePath {
  d: string
  color: string
  width: number
  winner: boolean
  /** Seconds per flow-dash cycle — volume shares read as flow speed (bigger share, faster). */
  flowDuration: number
  /** No traffic ever rides this lane (a 0% split leg): draw the ribbon, skip flow and particles. */
  noFlow: boolean
}

interface LaneLabel {
  x: number
  y: number
  kind: 'dot' | 'chip' | 'rank' | 'note' | 'pct' | 'windot'
  text?: string
  color?: string
}

interface Drawn {
  width: number
  height: number
  paths: LanePath[]
  /** Clearly-illustrative side branches (dashed) — e.g. the eligibility "if unsupported" fork. */
  extras: Array<{ d: string }>
  labels: LaneLabel[]
}

type GapKind = 'fan' | 'straight' | 'converge' | 'filter' | 'split'

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
          kind: el.dataset.laneGap as GapKind,
          top: rect.top - containerRect.top,
          height: rect.height,
        }
      })
      if (!gaps.length || !lanes.length) {
        setDrawn(null)
        return
      }

      const paths: LanePath[] = []
      const extras: Array<{ d: string }> = []
      const labels: LaneLabel[] = []

      lanes.forEach((lane, i) => {
        const x = laneX(i)
        const last = i === lanes.length - 1
        let d = ''
        let alive = true
        let winner = false
        let noFlow = false
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
            if (last && overflow > 0) {
              labels.push({ x: laneX(lanes.length), y: y1 - 22, kind: 'chip', text: `+${overflow} more` })
            }
            if (last && ghost) {
              labels.push({
                x: laneX(lanes.length) - LANE_STEP / 2 + 10,
                y: y1 - 20,
                kind: 'note',
                text: 'example set — activate a strategy to see yours',
              })
            }
          } else if (gap.kind === 'split') {
            if (lane.share === 0) {
              // A 0% leg is configured but receives nothing: the lane ends right here, and no
              // flow or particles ever ride it past the strategy.
              d += ` L ${x} ${y0} L ${x} ${y0 + gap.height * 0.4}`
              labels.push({ x, y: y0 + gap.height * 0.5, kind: 'rank', text: '✕ 0% — no traffic', color: lane.color })
              alive = false
              noFlow = true
            } else {
              d += ` L ${x} ${y0} L ${x} ${y1}`
              if (lane.share != null) {
                labels.push({ x, y: mid - 8, kind: 'pct', text: `${Math.round(lane.share * 100)}%`, color: lane.color })
              }
            }
          } else if (gap.kind === 'filter') {
            // Eligibility can drop connectors, but which ones depends on the payment. The example
            // set demonstrates it for real (its last lane peels off); real lanes continue and a
            // dashed illustrative fork shows the mechanism instead.
            if (ghost && last && lanes.length > 2) {
              d += ` L ${x} ${y0} C ${x} ${y0 + gap.height * 0.3}, ${x + 64} ${y0 + gap.height * 0.25}, ${x + 64} ${y0 + gap.height * 0.6}`
              labels.push({ x: x + 64, y: y0 + gap.height * 0.66, kind: 'rank', text: `✕ ${lane.name}`, color: lane.color })
              alive = false
            } else {
              d += ` L ${x} ${y0} L ${x} ${y1}`
              if (last && !ghost) {
                extras.push({
                  d: `M ${x} ${y0 + 2} C ${x} ${y0 + gap.height * 0.3}, ${x + 58} ${y0 + gap.height * 0.25}, ${x + 58} ${y0 + gap.height * 0.62}`,
                })
                labels.push({ x: x + 58, y: y0 + gap.height * 0.68, kind: 'rank', text: '✕ if not eligible', color: '#8d96aa' })
              }
            }
          } else if (gap.kind === 'converge' && deterministicHead && !ghost) {
            d += ` L ${x} ${y0}`
            if (lane.name === deterministicHead) {
              d += ` C ${x} ${mid}, ${laneX(0)} ${mid}, ${laneX(0)} ${y1 - 8}`
              winner = true
              labels.push({ x: laneX(0), y: y1 - 8, kind: 'windot', color: lane.color })
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
        if (d) {
          // Bigger volume shares flow faster; everything else drifts at a calm default.
          const flowDuration = lane.share != null ? Math.min(6, 0.85 / Math.max(lane.share, 0.12)) : 2.6
          paths.push({ d, color: lane.color, width: laneWidth(lane) * (winner ? 1.5 : 1), winner, flowDuration, noFlow })
        }
      })

      setDrawn({
        width: container.clientWidth,
        // Never scrollHeight: the previously-rendered absolute SVG contributes to it, so a
        // collapse after an expansion would ratchet the canvas to the tallest height seen.
        height: containerRect.height,
        paths,
        extras,
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
        {drawn.extras.map((extra, i) => (
          <path
            key={`extra-${i}`}
            d={extra.d}
            fill="none"
            stroke="#8d96aa"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeDasharray="3 4"
            opacity={0.55}
          />
        ))}
        {drawn.paths.map((path, i) => (
          <g key={i}>
            <path d={path.d} fill="none" stroke={path.color} strokeWidth={path.width + 7} strokeLinecap="round" opacity={ghost ? 0.09 : 0.14} />
            <path
              d={path.d}
              fill="none"
              stroke={path.color}
              strokeWidth={path.width}
              strokeLinecap="round"
              opacity={ghost ? 0.72 : 0.95}
              pathLength={100}
              // The draw-in class sets stroke-dasharray:100, which would override the example dash
              // pattern (stylesheet beats presentation attribute) — example lanes skip the animation.
              className={ghost ? undefined : 'de-lane-draw'}
              style={ghost ? { strokeDasharray: '6 5' } : { animationDelay: `${i * 90}ms` }}
            />
            {/* Ambient downstream flow; on volume splits its speed encodes the share. */}
            {path.noFlow ? null : (
            <path
              d={path.d}
              fill="none"
              stroke={path.color}
              strokeWidth={Math.max(1.2, path.width * 0.55)}
              strokeLinecap="round"
              opacity={ghost ? 0.5 : 0.9}
              className="de-lane-flow"
              style={{ animationDuration: `${path.flowDuration}s`, animationDelay: `${0.9 + i * 0.15}s` }}
            />
            )}
            {/* "Payments" riding the lane — one educational moving object per ribbon (two on
                busy lanes), traveling the full path on a loop. Share-weighted lanes carry
                faster particles, so a 60/30/10 split is visible as traffic, not just labels. */}
            {Array.from({ length: path.noFlow ? 0 : path.width > 3.5 ? 2 : 1 }, (_, p) => {
              const travel = Math.min(11, Math.max(4, path.flowDuration * 2.4))
              return (
                <circle
                  key={p}
                  className="de-lane-particle"
                  r={Math.max(2.4, path.width * 0.75)}
                  fill={path.color}
                  opacity={0}
                  style={{ filter: `drop-shadow(0 0 4px ${path.color})` }}
                >
                  <animateMotion
                    dur={`${travel}s`}
                    begin={`${1.2 + i * 0.7 + p * (travel / 2)}s`}
                    repeatCount="indefinite"
                    path={path.d}
                  />
                  <animate
                    attributeName="opacity"
                    values="0;0.95;0.95;0"
                    keyTimes="0;0.06;0.94;1"
                    dur={`${travel}s`}
                    begin={`${1.2 + i * 0.7 + p * (travel / 2)}s`}
                    repeatCount="indefinite"
                  />
                </circle>
              )
            })}
          </g>
        ))}
      </svg>
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 z-[1]">
        {drawn.labels.map((label, i) => {
          if (label.kind === 'dot') {
            return (
              <span
                key={i}
                className="de-dot-breathe absolute h-[11px] w-[11px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-brand-500 shadow-[0_0_12px_2px_rgba(59,130,246,0.55)]"
                style={{ left: label.x, top: label.y }}
              />
            )
          }
          if (label.kind === 'windot') {
            return (
              <span
                key={i}
                className="de-dot-breathe absolute h-[13px] w-[13px] -translate-x-1/2 -translate-y-1/2 rounded-full"
                style={{ left: label.x, top: label.y, background: label.color, boxShadow: `0 0 14px 3px ${label.color}66` }}
              />
            )
          }
          if (label.kind === 'chip') {
            return (
              <span
                key={i}
                title={label.text}
                className="absolute flex max-w-[80px] -translate-x-1/2 items-center gap-1 rounded-full border border-slate-200 bg-white px-1.5 py-0.5 font-mono text-[10px] text-slate-600 dark:border-[#1e2535] dark:bg-[#0d1118] dark:text-[#9ca7ba]"
                style={{ left: label.x, top: label.y }}
              >
                {label.color ? (
                  <span className="h-[7px] w-[7px] flex-shrink-0 rounded-[3px]" style={{ background: label.color }} />
                ) : null}
                <span className="min-w-0 truncate">{label.text}</span>
              </span>
            )
          }
          if (label.kind === 'pct') {
            return (
              <span
                key={i}
                className="absolute -translate-x-1/2 rounded-md border border-slate-200 bg-white px-1.5 font-mono text-[10px] font-semibold tabular-nums dark:border-[#1e2535] dark:bg-[#0d1118]"
                style={{ left: label.x, top: label.y, color: label.color }}
              >
                {label.text}
              </span>
            )
          }
          if (label.kind === 'note') {
            return (
              <span
                key={i}
                className="absolute whitespace-nowrap text-[10.5px] italic text-slate-400 dark:text-[#6d778a]"
                style={{ left: label.x, top: label.y }}
              >
                {label.text}
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
