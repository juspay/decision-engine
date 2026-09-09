import { RefObject, useEffect, useLayoutEffect, useRef, useState } from 'react'
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

interface GapRect {
  kind: GapKind
  top: number
  height: number
}

interface Drawn {
  width: number
  height: number
  paths: LanePath[]
  /** Clearly-illustrative side branches (dashed) — e.g. the eligibility "if unsupported" fork. */
  extras: Array<{ d: string }>
  labels: LaneLabel[]
}

type GapKind = 'fan' | 'straight' | 'converge' | 'filter' | 'split' | 'sort'

const LANE_X0 = 46
const LANE_STEP = 76

const laneX = (index: number) => LANE_X0 + index * LANE_STEP

function laneWidth(lane: LaneDef) {
  if (lane.share == null) return 2.5
  return Math.min(6, 1.5 + lane.share * 5.5)
}

const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)

/**
 * Draws each connector as one continuous ribbon behind the stage cards. The page renders spacer
 * divs marked `data-lane-gap` between its rows; this component measures them and routes every
 * lane through all of them — fanning apart at the entry, showing split shares, dropping 0% legs,
 * and — at the `sort` gap below success-rate scoring — periodically shuffling the lanes into a
 * new order with animated crossings: the ranking genuinely changes payment to payment, and the
 * continuous re-sort is that fact made visible.
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
  const geomRef = useRef<{ gaps: GapRect[]; aliveAtSort: boolean[] } | null>(null)
  const laneGroupRefs = useRef<Array<SVGGElement | null>>([])
  const belowXRef = useRef<number[]>([])

  /** One shared path builder, used for the initial render and for every re-sort frame. */
  const buildLanePath = (
    lane: LaneDef,
    i: number,
    gaps: GapRect[],
    belowX: number[],
    collect?: { labels: LaneLabel[]; extras: Array<{ d: string }>; sortEnabled: boolean },
  ): { d: string; winner: boolean; aliveAtSort: boolean } => {
    const last = i === lanes.length - 1
    let d = ''
    let alive = true
    let winner = false
    let afterSort = false
    let aliveAtSort = false
    for (const gap of gaps) {
      if (!alive) break
      const x = afterSort ? belowX[i] : laneX(i)
      const y0 = gap.top
      const y1 = gap.top + gap.height
      const mid = y0 + gap.height / 2
      if (gap.kind === 'fan') {
        const originX = 24
        const originY = y0 + 6
        d = `M ${originX} ${originY} C ${originX} ${y0 + gap.height * 0.55}, ${x} ${y0 + gap.height * 0.35}, ${x} ${y1}`
        if (collect) {
          if (i === 0) collect.labels.push({ x: originX, y: originY, kind: 'dot' })
          collect.labels.push({ x, y: y1 - 22, kind: 'chip', text: lane.name, color: lane.color })
          if (last && overflow > 0) {
            collect.labels.push({ x: laneX(lanes.length), y: y1 - 22, kind: 'chip', text: `+${overflow} more` })
          }
          if (last && ghost) {
            collect.labels.push({
              x: laneX(lanes.length) - LANE_STEP / 2 + 10,
              y: y1 - 20,
              kind: 'note',
              text: 'example set — activate a strategy to see yours',
            })
          }
        }
      } else if (gap.kind === 'split') {
        if (lane.share === 0) {
          // A 0% leg is configured but receives nothing: the lane ends right here.
          d += ` L ${x} ${y0} L ${x} ${y0 + gap.height * 0.4}`
          if (collect) {
            collect.labels.push({ x, y: y0 + gap.height * 0.5, kind: 'rank', text: '✕ 0% — no traffic', color: lane.color })
          }
          alive = false
        } else {
          d += ` L ${x} ${y0} L ${x} ${y1}`
          if (collect && lane.share != null) {
            collect.labels.push({ x, y: mid - 8, kind: 'pct', text: `${Math.round(lane.share * 100)}%`, color: lane.color })
          }
        }
      } else if (gap.kind === 'filter') {
        // Eligibility can drop connectors, but which ones depends on the payment. The example
        // set demonstrates it for real; real lanes continue past a dashed illustrative fork.
        if (ghost && last && lanes.length > 2) {
          d += ` L ${x} ${y0} C ${x} ${y0 + gap.height * 0.3}, ${x + 64} ${y0 + gap.height * 0.25}, ${x + 64} ${y0 + gap.height * 0.6}`
          if (collect) {
            collect.labels.push({ x: x + 64, y: y0 + gap.height * 0.66, kind: 'rank', text: `✕ ${lane.name}`, color: lane.color })
          }
          alive = false
        } else {
          d += ` L ${x} ${y0} L ${x} ${y1}`
          if (collect && last && !ghost) {
            collect.extras.push({
              d: `M ${x} ${y0 + 2} C ${x} ${y0 + gap.height * 0.3}, ${x + 58} ${y0 + gap.height * 0.25}, ${x + 58} ${y0 + gap.height * 0.62}`,
            })
            collect.labels.push({ x: x + 58, y: y0 + gap.height * 0.68, kind: 'rank', text: '✕ if not eligible', color: '#8d96aa' })
          }
        }
      } else if (gap.kind === 'sort') {
        aliveAtSort = true
        d += ` L ${x} ${y0} C ${x} ${mid}, ${belowX[i]} ${mid}, ${belowX[i]} ${y1}`
        afterSort = true
      } else if (gap.kind === 'converge' && deterministicHead && !ghost) {
        d += ` L ${x} ${y0}`
        if (lane.name === deterministicHead) {
          d += ` C ${x} ${mid}, ${laneX(0)} ${mid}, ${laneX(0)} ${y1 - 8}`
          winner = true
          if (collect) collect.labels.push({ x: laneX(0), y: y1 - 8, kind: 'windot', color: lane.color })
        } else {
          d += ` L ${x} ${y0 + gap.height * 0.42}`
          // Rank labels would drift once the sort animation moves lanes around — skip them then.
          if (collect && !collect.sortEnabled) {
            collect.labels.push({ x, y: y0 + gap.height * 0.5, kind: 'rank', text: `#${i + 1}`, color: lane.color })
          }
          alive = false
        }
      } else {
        // A converge gap without a deterministic winner draws straight through: the winner
        // depends on the payment, and pretending otherwise would be a lie.
        d += ` L ${x} ${y0} L ${x} ${y1}`
      }
    }
    return { d, winner, aliveAtSort }
  }

  useLayoutEffect(() => {
    const container = containerRef.current
    if (!container) return

    const measure = () => {
      const containerRect = container.getBoundingClientRect()
      const gaps: GapRect[] = Array.from(container.querySelectorAll<HTMLElement>('[data-lane-gap]')).map((el) => {
        const rect = el.getBoundingClientRect()
        return {
          kind: el.dataset.laneGap as GapKind,
          top: rect.top - containerRect.top,
          height: rect.height,
        }
      })
      if (!gaps.length || !lanes.length) {
        geomRef.current = null
        setDrawn(null)
        return
      }

      const sortEnabled = gaps.some((gap) => gap.kind === 'sort')
      const identity = lanes.map((_, i) => laneX(i))
      belowXRef.current = identity
      const collect = { labels: [] as LaneLabel[], extras: [] as Array<{ d: string }>, sortEnabled }
      const paths: LanePath[] = []
      const aliveAtSort: boolean[] = []
      lanes.forEach((lane, i) => {
        const built = buildLanePath(lane, i, gaps, identity, collect)
        aliveAtSort.push(built.aliveAtSort)
        if (built.d) {
          const flowDuration = lane.share != null ? Math.min(6, 0.85 / Math.max(lane.share, 0.12)) : 2.6
          paths.push({
            d: built.d,
            color: lane.color,
            width: laneWidth(lane) * (built.winner ? 1.5 : 1),
            winner: built.winner,
            flowDuration,
            noFlow: lane.share === 0,
          })
        }
      })
      geomRef.current = { gaps, aliveAtSort }

      setDrawn({
        width: container.clientWidth,
        // Never scrollHeight: the previously-rendered absolute SVG contributes to it, so a
        // collapse after an expansion would ratchet the canvas to the tallest height seen.
        height: containerRect.height,
        paths,
        extras: collect.extras,
        labels: collect.labels,
      })
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(container)
    return () => observer.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef, lanes, ghost, deterministicHead, overflow])

  /* The re-sort loop: every few seconds, tween the below-sort lane positions into a new
     permutation. Path data is mutated directly on the DOM so the draw-in animation doesn't
     replay; particles get their motion paths refreshed after each swap (they restart from the
     top, reading as fresh payments entering the reordered pipeline). */
  useEffect(() => {
    const geom = geomRef.current
    if (!drawn || !geom) return
    if (typeof window === 'undefined') return
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    if (!geom.gaps.some((gap) => gap.kind === 'sort')) return
    const movable = lanes.map((_, i) => i).filter((i) => geom.aliveAtSort[i])
    if (movable.length < 2) return

    let raf = 0
    const applyBelowX = (belowX: number[]) => {
      lanes.forEach((lane, i) => {
        const group = laneGroupRefs.current[i]
        if (!group) return
        const { d } = buildLanePath(lane, i, geom.gaps, belowX)
        group.querySelectorAll<SVGPathElement>('path[data-lane-ribbon]').forEach((el) => el.setAttribute('d', d))
      })
    }
    const refreshParticles = () => {
      lanes.forEach((lane, i) => {
        const group = laneGroupRefs.current[i]
        if (!group) return
        const { d } = buildLanePath(lane, i, geom.gaps, belowXRef.current)
        group.querySelectorAll('animateMotion').forEach((el) => el.setAttribute('path', d))
      })
    }
    const interval = window.setInterval(() => {
      const from = [...belowXRef.current]
      // A rotation among the movable lanes guarantees every re-sort visibly changes the order.
      const slots = movable.map((i) => from[i])
      const rotated = [...slots.slice(1), slots[0]]
      const to = [...from]
      movable.forEach((laneIndex, j) => {
        to[laneIndex] = rotated[j]
      })
      const started = performance.now()
      const duration = 950
      const step = (now: number) => {
        const t = Math.min(1, (now - started) / duration)
        const eased = easeInOut(t)
        const current = from.map((value, i) => value + (to[i] - value) * eased)
        applyBelowX(current)
        if (t < 1) {
          raf = requestAnimationFrame(step)
        } else {
          belowXRef.current = to
          refreshParticles()
        }
      }
      raf = requestAnimationFrame(step)
    }, 4600)

    return () => {
      window.clearInterval(interval)
      cancelAnimationFrame(raf)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drawn, lanes])

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
          <g key={i} ref={(el) => (laneGroupRefs.current[i] = el)}>
            <path
              data-lane-ribbon
              d={path.d}
              fill="none"
              stroke={path.color}
              strokeWidth={path.width + 7}
              strokeLinecap="round"
              opacity={ghost ? 0.09 : 0.14}
            />
            <path
              data-lane-ribbon
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
                data-lane-ribbon
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
            {/* "Payments" riding the lane — educational moving objects, share-weighted. */}
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
