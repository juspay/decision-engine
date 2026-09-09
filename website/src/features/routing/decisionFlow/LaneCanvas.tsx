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
  kind: 'dot' | 'chip' | 'rank' | 'note' | 'pct' | 'windot' | 'endchip'
  text?: string
  color?: string
  /** endchip only: which lane this names, so the wave engine can slide/hide it. */
  laneIndex?: number
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
  labels: LaneLabel[]
}

/** Which lanes are currently cut out of the flow, by the stage that removed them. */
interface Cuts {
  filter: number | null
  health: number | null
}

type GapKind = 'fan' | 'straight' | 'converge' | 'filter' | 'split' | 'sort' | 'demote'

const LANE_X0 = 46
const LANE_STEP = 76

const laneX = (index: number) => LANE_X0 + index * LANE_STEP

function laneWidth(lane: LaneDef) {
  if (lane.share == null) return 2.5
  return Math.min(6, 1.5 + lane.share * 5.5)
}

const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)

/**
 * Draws each connector as one continuous ribbon behind the stage cards, then keeps the diagram
 * alive with a wave that travels down the page each cycle:
 *
 * - the `filter` gap (eligibility) sometimes removes a connector and later re-admits it —
 *   across payments, different connectors genuinely drop here;
 * - `sort` gaps (success-rate, cost) re-sort the surviving lanes with animated crossings;
 * - the `demote` gap (health penalties) sometimes takes a connector out for a while and then
 *   lets it back in as its scores recover.
 *
 * Payment particles hide while lanes re-shape (so dots never float off-ribbon) and re-enter on
 * the new geometry. Everything gates off under prefers-reduced-motion.
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
  const geomRef = useRef<{ gaps: GapRect[]; aliveAnim: boolean[]; sortGapCount: number } | null>(null)
  const laneGroupRefs = useRef<Array<SVGGElement | null>>([])
  const svgRef = useRef<SVGSVGElement | null>(null)
  const overlayRef = useRef<HTMLDivElement | null>(null)
  /** Per sort-gap (success-rate, cost), each lane's x below that gap. */
  const slotSetsRef = useRef<number[][]>([])
  const cutsRef = useRef<Cuts>({ filter: null, health: null })

  /** One shared path builder, used for the initial render and for every animation frame. Cuts
      never truncate geometry — they are smooth dash-mask overlays applied by the wave engine. */
  const buildLanePath = (
    lane: LaneDef,
    i: number,
    gaps: GapRect[],
    slotSets: number[][],
    collect?: { labels: LaneLabel[]; sortEnabled: boolean },
  ): { d: string; winner: boolean; aliveAnim: boolean } => {
    const last = i === lanes.length - 1
    let d = ''
    let alive = true
    let winner = false
    let orderStep = 0
    let currentSlotX: number | null = null
    let aliveAnim = false
    for (const gap of gaps) {
      if (!alive) break
      const x: number = currentSlotX ?? laneX(i)
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
        aliveAnim = true
        d += ` L ${x} ${y0} L ${x} ${y1}`
      } else if (gap.kind === 'sort') {
        aliveAnim = true
        const target: number = slotSets[orderStep]?.[i] ?? x
        d += ` L ${x} ${y0} C ${x} ${mid}, ${target} ${mid}, ${target} ${y1}`
        currentSlotX = target
        orderStep++
      } else if (gap.kind === 'demote') {
        d += ` L ${x} ${y0} L ${x} ${y1}`
      } else if (gap.kind === 'converge' && deterministicHead && !ghost) {
        d += ` L ${x} ${y0}`
        if (lane.name === deterministicHead) {
          d += ` C ${x} ${mid}, ${laneX(0)} ${mid}, ${laneX(0)} ${y1 - 8}`
          winner = true
          if (collect) {
            collect.labels.push({ x: laneX(0), y: y1 - 8, kind: 'windot', color: lane.color })
            collect.labels.push({ x: laneX(0), y: y1 + 4, kind: 'endchip', text: lane.name, color: lane.color, laneIndex: i })
          }
        } else {
          d += ` L ${x} ${y0 + gap.height * 0.42}`
          // Rank labels would drift once the animation moves lanes around — skip them then.
          if (collect && !collect.sortEnabled) {
            collect.labels.push({ x, y: y0 + gap.height * 0.5, kind: 'rank', text: `#${i + 1}`, color: lane.color })
          }
          alive = false
        }
      } else if (gap.kind === 'converge') {
        // Without a deterministic winner the lanes run through — the winner depends on the
        // payment — but the arriving candidates get named, leftmost slot = current leader.
        d += ` L ${x} ${y0} L ${x} ${y1}`
        if (collect) {
          collect.labels.push({ x, y: y1 - 20, kind: 'endchip', text: lane.name, color: lane.color, laneIndex: i })
        }
      } else {
        d += ` L ${x} ${y0} L ${x} ${y1}`
      }
    }
    return { d, winner, aliveAnim }
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

      const sortGapCount = gaps.filter((gap) => gap.kind === 'sort').length
      const identity = lanes.map((_, i) => laneX(i))
      const identitySets = Array.from({ length: sortGapCount }, () => [...identity])
      slotSetsRef.current = identitySets
      cutsRef.current = { filter: null, health: null }
      const collect = { labels: [] as LaneLabel[], sortEnabled: sortGapCount > 0 }
      const paths: LanePath[] = []
      const aliveAnim: boolean[] = []
      lanes.forEach((lane, i) => {
        const built = buildLanePath(lane, i, gaps, identitySets, collect)
        aliveAnim.push(built.aliveAnim)
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
      geomRef.current = { gaps, aliveAnim, sortGapCount }

      setDrawn({
        width: container.clientWidth,
        // Never scrollHeight: the previously-rendered absolute SVG contributes to it, so a
        // collapse after an expansion would ratchet the canvas to the tallest height seen.
        height: containerRect.height,
        paths,
        labels: collect.labels,
      })
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(container)
    return () => observer.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [containerRef, lanes, ghost, deterministicHead, overflow])

  /* The wave: filter removes/re-admits → success-rate rotates → health takes one out or lets
     one back → cost swaps the leaders. Path data is mutated on the DOM so the draw-in never
     replays; particles hide during the wave and re-enter on the final geometry. */
  useEffect(() => {
    const geom = geomRef.current
    if (!drawn || !geom) return
    if (typeof window === 'undefined') return
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const movable = lanes.map((_, i) => i).filter((i) => geom.aliveAnim[i])
    if (movable.length < 2) return
    const hasAnimGaps = geom.gaps.some((gap) => gap.kind === 'filter' || gap.kind === 'sort' || gap.kind === 'demote')
    if (!hasAnimGaps) return

    const filterRect = geom.gaps.find((gap) => gap.kind === 'filter')
    const demoteRect = geom.gaps.find((gap) => gap.kind === 'demote')
    const slotPositions = movable.map((i) => laneX(i))
    let orders: number[][] = Array.from({ length: geom.sortGapCount }, () => [...movable])
    const toSlotSets = (orderList: number[][]) =>
      orderList.map((order) => {
        const xs = lanes.map((_, i) => laneX(i))
        order.forEach((laneIndex, position) => {
          xs[laneIndex] = slotPositions[position]
        })
        return xs
      })

    let raf = 0
    const timeouts: number[] = []
    const cutMarkers: Partial<Record<'filter' | 'health', HTMLElement>> = {}

    const rebuild = () => {
      lanes.forEach((lane, i) => {
        const group = laneGroupRefs.current[i]
        if (!group) return
        const { d } = buildLanePath(lane, i, geom.gaps, slotSetsRef.current)
        group.querySelectorAll<SVGPathElement>('path[data-lane-ribbon]').forEach((el) => el.setAttribute('d', d))
      })
    }
    const refreshParticles = () => {
      lanes.forEach((lane, i) => {
        const group = laneGroupRefs.current[i]
        if (!group) return
        const { d } = buildLanePath(lane, i, geom.gaps, slotSetsRef.current)
        group.querySelectorAll('animateMotion').forEach((el) => el.setAttribute('path', d))
      })
    }

    const spawnFlash = (x: number, y: number, text: string, color: string) => {
      const overlay = overlayRef.current
      if (!overlay) return
      const marker = document.createElement('span')
      marker.className = 'de-demote-flash absolute rounded-md border px-1.5 font-mono text-[10px] font-semibold'
      marker.style.left = `${x}px`
      marker.style.top = `${y}px`
      marker.style.color = color
      marker.style.borderColor = `${color}55`
      marker.style.background = `${color}14`
      marker.textContent = text
      overlay.appendChild(marker)
      timeouts.push(window.setTimeout(() => marker.remove(), 1700))
    }
    const setCutMarker = (kind: 'filter' | 'health', laneIndex: number | null) => {
      cutMarkers[kind]?.remove()
      delete cutMarkers[kind]
      if (laneIndex == null) return
      const overlay = overlayRef.current
      const rect = kind === 'filter' ? filterRect : demoteRect
      if (!overlay || !rect) return
      const lane = lanes[laneIndex]
      const x = kind === 'filter' ? laneX(laneIndex) : (slotSetsRef.current[0]?.[laneIndex] ?? laneX(laneIndex))
      const color = kind === 'filter' ? lane.color : '#fbbf24'
      const marker = document.createElement('span')
      marker.className = 'de-cut-marker absolute -translate-x-1/2 rounded-md border px-1.5 font-mono text-[10px] font-semibold'
      marker.style.left = `${x}px`
      marker.style.top = `${rect.top + rect.height * 0.52}px`
      marker.style.color = color
      marker.style.borderColor = `${color}55`
      marker.style.background = `${color}14`
      marker.textContent = kind === 'filter' ? `✕ ${lane.name} not eligible` : `▼ ${lane.name} penalized`
      overlay.appendChild(marker)
      cutMarkers[kind] = marker
    }
    /** Fraction (0..100, matching pathLength=100) of a lane's path above the given y — found by
        binary search over real arc length, so the dash mask breaks exactly at the gap. */
    const fractionAtY = (laneIndex: number, targetY: number): number => {
      const group = laneGroupRefs.current[laneIndex]
      const core = group?.querySelector<SVGPathElement>('path[data-lane-core]')
      if (!core) return 50
      const total = core.getTotalLength()
      let lo = 0
      let hi = total
      for (let step = 0; step < 14; step++) {
        const mid = (lo + hi) / 2
        if (core.getPointAtLength(mid).y < targetY) lo = mid
        else hi = mid
      }
      return (lo / total) * 100
    }
    const setLaneCut = (laneIndex: number, cutY: number | null) => {
      const group = laneGroupRefs.current[laneIndex]
      if (!group) return
      const masked = group.querySelectorAll<SVGPathElement>('path[data-lane-mask]')
      const flow = group.querySelector<SVGPathElement>('path.de-lane-flow')
      if (cutY == null) {
        // Regrow to full length; example lanes get their dash pattern back once regrown.
        masked.forEach((el) => (el.style.strokeDasharray = '100 100'))
        if (ghost) {
          timeouts.push(window.setTimeout(() => masked.forEach((el) => (el.style.strokeDasharray = '6 5')), 800))
        }
        if (flow) flow.style.opacity = String(ghost ? 0.5 : 0.9)
        group.classList.remove('de-lane-cut')
      } else {
        const fraction = fractionAtY(laneIndex, cutY)
        masked.forEach((el) => (el.style.strokeDasharray = `${fraction.toFixed(2)} 100`))
        if (flow) flow.style.opacity = '0'
        group.classList.add('de-lane-cut')
      }
    }
    const toggleCut = (kind: 'filter' | 'health', probability: number) => {
      const cuts = cutsRef.current
      const current = cuts[kind]
      const other = kind === 'filter' ? cuts.health : cuts.filter
      const rect = kind === 'filter' ? filterRect : demoteRect
      if (!rect) return
      if (current == null) {
        if (Math.random() > probability) return
        const candidates = movable.filter((i) => i !== other)
        if (!candidates.length) return
        const victim = candidates[Math.floor(Math.random() * candidates.length)]
        cuts[kind] = victim
        setLaneCut(victim, rect.top + rect.height * 0.38)
        // The name lands the moment the retracting tip reaches the break point.
        timeouts.push(window.setTimeout(() => setCutMarker(kind, victim), 550))
      } else {
        cuts[kind] = null
        setCutMarker(kind, null)
        setLaneCut(current, null)
        const lane = lanes[current]
        spawnFlash(
          kind === 'filter' ? laneX(current) : (slotSetsRef.current[0]?.[current] ?? laneX(current)),
          rect.top + rect.height * 0.4,
          `↩ ${lane.name} back`,
          '#34d399',
        )
      }
    }

    const runSortPhase = (sortIndex: number, fromOrders: number[][], toOrders: number[][], onDone: () => void) => {
      const fromSets = toSlotSets(fromOrders)
      const toSets = toSlotSets(toOrders)
      const started = performance.now()
      const duration = 850
      const step = (now: number) => {
        const t = Math.min(1, (now - started) / duration)
        const eased = easeInOut(t)
        // Only this phase's gap tweens; earlier ones already sit at their targets.
        const blended = toSets.map((set, k) => {
          if (k < sortIndex) return set
          if (k > sortIndex) return fromSets[k]
          return set.map((value, i) => fromSets[k][i] + (value - fromSets[k][i]) * eased)
        })
        slotSetsRef.current = blended
        rebuild()
        if (t < 1) raf = requestAnimationFrame(step)
        else {
          slotSetsRef.current = toSets.map((set, k) => (k <= sortIndex ? set : fromSets[k]))
          onDone()
        }
      }
      raf = requestAnimationFrame(step)
    }

    const interval = window.setInterval(() => {
      const svg = svgRef.current
      svg?.classList.add('de-wave')
      const prev = orders
      const next: number[][] = prev.map((order, k) => {
        if (k === 0) return [...order.slice(1), order[0]] // success-rate: rotate the field
        const reordered = [...order]
        if (Math.random() < 0.65 && reordered.length > 1) {
          ;[reordered[0], reordered[1]] = [reordered[1], reordered[0]] // cost: cheaper overtakes
        }
        return reordered
      })

      const finish = () => {
        orders = next
        refreshParticles()
        // Slide the arriving-connector chips to their final slots; hide the ones that are out.
        const lastSet = slotSetsRef.current[slotSetsRef.current.length - 1]
        overlayRef.current?.querySelectorAll<HTMLElement>('.de-end-chip').forEach((chip) => {
          const laneIndex = Number(chip.dataset.lane)
          if (Number.isNaN(laneIndex)) return
          const cut = cutsRef.current.filter === laneIndex || cutsRef.current.health === laneIndex
          chip.style.opacity = cut ? '0' : '1'
          chip.style.left = `${lastSet?.[laneIndex] ?? laneX(laneIndex)}px`
        })
        timeouts.push(window.setTimeout(() => svg?.classList.remove('de-wave'), 200))
      }
      // The wave travels top-down: filter → sort(sr) → health → sort(cost).
      toggleCut('filter', 0.45)
      if (geom.sortGapCount === 0) {
        timeouts.push(
          window.setTimeout(() => {
            toggleCut('health', 0.55)
            finish()
          }, 900),
        )
        return
      }
      timeouts.push(
        window.setTimeout(() => {
          runSortPhase(0, prev, [next[0], ...prev.slice(1)], () => {
            timeouts.push(
              window.setTimeout(() => {
                toggleCut('health', 0.55)
                if (geom.sortGapCount > 1) {
                  timeouts.push(
                    window.setTimeout(() => {
                      runSortPhase(1, [next[0], ...prev.slice(1)], next, finish)
                    }, 550),
                  )
                } else {
                  finish()
                }
              }, 450),
            )
          })
        }, 650),
      )
    }, 6200)

    return () => {
      window.clearInterval(interval)
      cancelAnimationFrame(raf)
      timeouts.forEach((id) => window.clearTimeout(id))
      Object.values(cutMarkers).forEach((marker) => marker?.remove())
      svgRef.current?.classList.remove('de-wave')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drawn, lanes])

  if (!drawn) return null

  return (
    <>
      <svg
        ref={svgRef}
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-0"
        width={drawn.width}
        height={drawn.height}
        viewBox={`0 0 ${drawn.width} ${drawn.height}`}
      >
        {drawn.paths.map((path, i) => (
          <g key={i} ref={(el) => (laneGroupRefs.current[i] = el)}>
            <path
              data-lane-ribbon
              data-lane-mask
              d={path.d}
              fill="none"
              stroke={path.color}
              strokeWidth={path.width + 7}
              strokeLinecap="round"
              opacity={ghost ? 0.09 : 0.14}
              pathLength={100}
            />
            <path
              data-lane-ribbon
              data-lane-mask
              data-lane-core
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
      <div ref={overlayRef} aria-hidden="true" className="pointer-events-none absolute inset-0 z-[1]">
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
          if (label.kind === 'endchip') {
            return (
              <span
                key={i}
                data-lane={label.laneIndex}
                title={label.text}
                className="de-end-chip absolute flex max-w-[80px] -translate-x-1/2 items-center gap-1 rounded-full border border-slate-200 bg-white px-1.5 py-0.5 font-mono text-[10px] text-slate-600 dark:border-[#1e2535] dark:bg-[#0d1118] dark:text-[#9ca7ba]"
                style={{
                  left: label.x,
                  top: label.y,
                  transition: 'left 0.55s cubic-bezier(0.65, 0, 0.35, 1), opacity 0.3s',
                }}
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
