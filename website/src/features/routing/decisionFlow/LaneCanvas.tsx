import { RefObject, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { LaneDef } from './model'

interface LanePath {
  color: string
  width: number
  winner: boolean
  /** Seconds per flow-dash cycle — volume shares read as flow speed (bigger share, faster). */
  flowDuration: number
  /** No traffic ever rides this lane (a 0% split leg): draw the ribbon, skip flow and particles. */
  noFlow: boolean
  d: string
}

interface LaneLabel {
  x: number
  y: number
  kind: 'dot' | 'chip' | 'rank' | 'note' | 'pct' | 'windot' | 'endchip'
  text?: string
  color?: string
  /** endchip/windot only: which lane this belongs to, so the wave engine can slide/hide it. */
  laneIndex?: number
  /** endchip only: the deterministic winner's chip stays at the converge point. */
  pinned?: boolean
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

/**
 * A stage marker: one per stage at most (keyed), so a cut can never stack duplicates. Persistent
 * markers stay while their connector is out; transient ones fade themselves away.
 */
interface Marker {
  /** Unique per appearance: React remounts the node, so its entrance animation replays. */
  id: number
  key: 'filter' | 'health' | 'restored'
  laneIndex: number
  gap: 'filter' | 'demote'
  text: string
  tone: 'cut' | 'warn' | 'ok'
  transient: boolean
}

type GapKind = 'fan' | 'straight' | 'converge' | 'filter' | 'split' | 'sort' | 'demote'

interface Tween {
  /** Only one tween per channel is ever live: pushing supersedes, so two can't fight one value. */
  channel: string
  t0: number
  dur: number
  apply: (eased: number) => void
  done?: () => void
}

const LANE_X0 = 46
const LANE_STEP = 76

const REVEAL_MS = 1100
const CUT_MS = 1200
const SORT_MS = 1400
const WAVE_MS = 9500

const laneX = (index: number) => LANE_X0 + index * LANE_STEP

function laneWidth(lane: LaneDef) {
  if (lane.share == null) return 2.5
  return Math.min(6, 1.5 + lane.share * 5.5)
}

const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2)

/** Mix a hex colour toward black — the lane palette is tuned for dark, and needs depth on white. */
function shade(hex: string, amount: number) {
  const value = hex.replace('#', '')
  const num = parseInt(value.length === 3 ? value.replace(/./g, (c) => c + c) : value, 16)
  const mix = (channel: number) => Math.round(channel * (1 - amount))
  const r = mix((num >> 16) & 255)
  const g = mix((num >> 8) & 255)
  const b = mix(num & 255)
  return `#${((1 << 24) | (r << 16) | (g << 8) | b).toString(16).slice(1)}`
}

/**
 * Draws each connector as one continuous ribbon behind the stage cards and keeps the diagram
 * alive with a wave that travels down the page: eligibility removes a connector and later
 * re-admits it, the re-rank stages re-sort the survivors, health penalties take one out for a
 * while and let it back in.
 *
 * Every visual property of a ribbon — its geometry and how much of it is visible — is written by
 * `renderLanes()` from one piece of state on every animated frame. Nothing else touches a lane:
 * no CSS transitions on the ribbons, no per-cut classes, no leftover inline styles. That is what
 * keeps a cut lane genuinely gone below its break instead of lingering, and markers are React
 * state keyed by stage, so two can never stack.
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
  const [markers, setMarkers] = useState<Marker[]>([])
  const [isDark, setIsDark] = useState(
    () => typeof document !== 'undefined' && document.documentElement.classList.contains('dark'),
  )

  const geomRef = useRef<{ gaps: GapRect[]; animatable: boolean[]; sortGapCount: number } | null>(null)
  const laneGroupRefs = useRef<Array<SVGGElement | null>>([])
  const svgRef = useRef<SVGSVGElement | null>(null)
  const overlayRef = useRef<HTMLDivElement | null>(null)
  /** Per sort-gap (success-rate, cost), each lane's x below that gap. */
  const slotSetsRef = useRef<number[][]>([])
  /** Visible fraction of each lane, 0-100 against pathLength=100. The single mask authority. */
  const visRef = useRef<number[]>([])
  const cutsRef = useRef<{ filter: number | null; health: number | null }>({ filter: null, health: null })
  const tweensRef = useRef<Tween[]>([])
  const dirtyRef = useRef(true)
  const markerIdRef = useRef(0)
  const nextMarkerId = () => ++markerIdRef.current
  /** Set by the wave effect so renderLanes can place demote markers under the right sort gap. */
  const markerXRef = useRef<{ demoteX: (i: number) => number; laneX: (i: number) => number } | null>(null)
  /** Identity of the diagram itself, so a pure resize never resets a running animation. */
  const signatureRef = useRef<string | null>(null)
  /** Geometry moved under a live cut: the wave effect re-anchors its break point next frame. */
  const pendingRemeasureRef = useRef(false)

  useEffect(() => {
    if (typeof document === 'undefined') return
    const root = document.documentElement
    const observer = new MutationObserver(() => setIsDark(root.classList.contains('dark')))
    observer.observe(root, { attributes: true, attributeFilter: ['class'] })
    return () => observer.disconnect()
  }, [])

  const laneColor = (color: string) => (isDark ? color : shade(color, 0.26))
  const coreOpacity = ghost ? (isDark ? 0.7 : 0.6) : isDark ? 0.95 : 0.9
  const haloOpacity = ghost ? 0.08 : isDark ? 0.14 : 0.13
  const flowOpacity = ghost ? 0.5 : isDark ? 0.9 : 0.75

  /** Path geometry only — cuts are a mask, never a change of shape. */
  const buildLanePath = (
    lane: LaneDef,
    i: number,
    gaps: GapRect[],
    slotSets: number[][],
    collect?: { labels: LaneLabel[]; sortEnabled: boolean },
  ): { d: string; winner: boolean; animatable: boolean } => {
    const last = i === lanes.length - 1
    let d = ''
    let alive = true
    let winner = false
    let orderStep = 0
    let currentSlotX: number | null = null
    let animatable = false
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
        animatable = true
        d += ` L ${x} ${y0} L ${x} ${y1}`
      } else if (gap.kind === 'sort') {
        animatable = true
        const target: number = slotSets[orderStep]?.[i] ?? x
        d += ` L ${x} ${y0} C ${x} ${y0 + gap.height * 0.62}, ${target} ${y0 + gap.height * 0.38}, ${target} ${y1}`
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
            collect.labels.push({ x: laneX(0), y: y1 - 8, kind: 'windot', color: lane.color, laneIndex: i })
            collect.labels.push({ x: laneX(0), y: y1 + 4, kind: 'endchip', text: lane.name, color: lane.color, laneIndex: i, pinned: true })
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
    return { d, winner, animatable }
  }

  /** The one place a ribbon's geometry, mask, flow and marker positions are written. */
  const renderLanes = () => {
    const geom = geomRef.current
    if (!geom) return
    const gaps = geom.gaps
    lanes.forEach((lane, i) => {
      const group = laneGroupRefs.current[i]
      if (!group) return
      const { d } = buildLanePath(lane, i, gaps, slotSetsRef.current)
      const visible = visRef.current[i] ?? 100
      const full = visible >= 99.5
      group.querySelectorAll<SVGPathElement>('path[data-lane-mask]').forEach((el) => {
        el.setAttribute('d', d)
        // The mask is the only dash pattern a ribbon ever carries, so nothing can restore a
        // stale full-length dash over a cut.
        el.style.strokeDasharray = `${visible.toFixed(2)} 100`
      })
      const flow = group.querySelector<SVGPathElement>('path.de-lane-flow')
      if (flow) {
        // The bead pattern can't also carry the mask, so the current simply stops while a lane
        // is anything less than whole.
        flow.setAttribute('d', d)
        flow.style.opacity = String(full ? flowOpacity : 0)
      }
      group.classList.toggle('de-lane-cut', !full)
    })
    // Markers ride their lane's x at the stage that produced them.
    overlayRef.current?.querySelectorAll<HTMLElement>('[data-marker-lane]').forEach((el) => {
      const laneIndex = Number(el.dataset.markerLane)
      if (Number.isNaN(laneIndex)) return
      const x =
        el.dataset.markerGap === 'demote'
          ? markerXRef.current?.demoteX(laneIndex) ?? laneX(laneIndex)
          : laneX(laneIndex)
      el.style.left = `${x}px`
    })
    const lastSet = slotSetsRef.current[slotSetsRef.current.length - 1]
    const isCut = (laneIndex: number) =>
      cutsRef.current.filter === laneIndex || cutsRef.current.health === laneIndex
    overlayRef.current?.querySelectorAll<HTMLElement>('.de-end-chip').forEach((chip) => {
      const laneIndex = Number(chip.dataset.lane)
      if (Number.isNaN(laneIndex)) return
      chip.style.opacity = isCut(laneIndex) ? '0' : '1'
      // A deterministic winner's ribbon always converges to the first slot, so its chip stays
      // there too rather than chasing the lane's sorted position.
      chip.style.left = `${
        chip.dataset.pinned === 'true' ? laneX(0) : lastSet?.[laneIndex] ?? laneX(laneIndex)
      }px`
    })
    // The win dot marks a decision that this lane no longer reaches while it is cut.
    overlayRef.current?.querySelectorAll<HTMLElement>('[data-windot-lane]').forEach((el) => {
      const laneIndex = Number(el.dataset.windotLane)
      el.style.opacity = Number.isNaN(laneIndex) || !isCut(laneIndex) ? '1' : '0'
    })
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
      // A resize or an expanded stage re-measures the same diagram; only a genuinely different
      // lane set or stage structure may reset cuts, markers and tweens mid-flight.
      const signature = JSON.stringify([lanes.map((l) => [l.name, l.color, l.share]), gaps.map((g) => g.kind), ghost, deterministicHead])
      const reseed = signature !== signatureRef.current
      signatureRef.current = signature
      if (reseed) {
        slotSetsRef.current = Array.from({ length: sortGapCount }, () => [...identity])
        cutsRef.current = { filter: null, health: null }
        setMarkers([])
      } else if (slotSetsRef.current.length !== sortGapCount) {
        slotSetsRef.current = Array.from({ length: sortGapCount }, () => [...identity])
      }
      const collect = { labels: [] as LaneLabel[], sortEnabled: sortGapCount > 0 }
      const paths: LanePath[] = []
      const animatable: boolean[] = []
      lanes.forEach((lane, i) => {
        const built = buildLanePath(lane, i, gaps, slotSetsRef.current, collect)
        animatable.push(built.animatable)
        if (built.d) {
          const flowDuration = lane.share != null ? Math.min(7, 1.1 / Math.max(lane.share, 0.12)) : 3.4
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
      geomRef.current = { gaps, animatable, sortGapCount }

      const reduced =
        typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches
      // A hidden tab gets no animation frames, so skip the reveal rather than stage lanes at zero.
      const canReveal = !reduced && typeof document !== 'undefined' && document.visibilityState === 'visible'
      if (reseed) {
        visRef.current = lanes.map(() => (canReveal ? 0 : 100))
        tweensRef.current = []
        if (canReveal) {
          const now = performance.now()
          lanes.forEach((_, i) => {
            tweensRef.current.push({
              channel: `vis:${i}`,
              t0: now + i * 90,
              dur: REVEAL_MS,
              apply: (eased) => {
                visRef.current[i] = eased * 100
              },
            })
          })
        }
      } else {
        // Same diagram, new box: keep every cut and tween, just re-anchor the break points.
        pendingRemeasureRef.current = true
      }
      dirtyRef.current = true

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

  /* One frame loop drives every tween and then rewrites the lanes from current state. */
  useEffect(() => {
    if (!drawn) return
    // Theme changes recompute flowOpacity/laneColor, which only reach the DOM via renderLanes.
    dirtyRef.current = true
    let raf = 0
    const loop = (now: number) => {
      const tweens = tweensRef.current
      if (tweens.length) {
        // Oldest first, so when several resolve in one frame the newest intent lands last.
        const finished: Tween[] = []
        for (let i = 0; i < tweens.length; i++) {
          const tween = tweens[i]
          if (now < tween.t0) continue
          const progress = tween.dur <= 0 ? 1 : Math.min(1, (now - tween.t0) / tween.dur)
          tween.apply(easeInOut(progress))
          if (progress >= 1) finished.push(tween)
        }
        if (finished.length) {
          tweensRef.current = tweens.filter((tween) => !finished.includes(tween))
          finished.forEach((tween) => tween.done?.())
        }
        dirtyRef.current = true
      }
      if (dirtyRef.current) {
        renderLanes()
        dirtyRef.current = tweens.length > 0
      }
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drawn, lanes, isDark, ghost])

  /* The wave: eligibility removes / re-admits, success-rate rotates, health takes one out or
     lets one back, cost swaps the leaders. */
  useEffect(() => {
    const geom = geomRef.current
    if (!drawn || !geom) return
    if (typeof window === 'undefined') return
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const movable = lanes.map((_, i) => i).filter((i) => geom.animatable[i])
    if (movable.length < 2) return

    const timeouts: number[] = []
    const later = (fn: () => void, ms: number) => timeouts.push(window.setTimeout(fn, ms))
    const pushTween = (tween: Tween) => {
      tweensRef.current = tweensRef.current.filter((t) => t.channel !== tween.channel)
      tweensRef.current.push(tween)
    }
    /** Gaps are read lazily: a re-measure moves them, and a stale snapshot cuts at the wrong y. */
    const gapOf = (kind: GapKind) => geomRef.current?.gaps.find((gap) => gap.kind === kind)
    /** Slot set of the last sort ABOVE the demote gap — not blindly index 0. */
    const demoteSlotIndex = () => {
      const gaps = geomRef.current?.gaps ?? []
      let sorts = 0
      for (const gap of gaps) {
        if (gap.kind === 'demote') break
        if (gap.kind === 'sort') sorts++
      }
      return sorts - 1
    }
    const demoteX = (laneIndex: number) => {
      const idx = demoteSlotIndex()
      return (idx >= 0 ? slotSetsRef.current[idx]?.[laneIndex] : undefined) ?? laneX(laneIndex)
    }
    markerXRef.current = { demoteX, laneX }
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

    /** Where a lane's ribbon meets a stage, as a fraction of its length — the exact break point. */
    const cutFraction = (laneIndex: number, y: number) => {
      const core = laneGroupRefs.current[laneIndex]?.querySelector<SVGPathElement>('path[data-lane-core]')
      if (!core) return 55
      const total = core.getTotalLength()
      let lo = 0
      let hi = total
      for (let step = 0; step < 16; step++) {
        const mid = (lo + hi) / 2
        if (core.getPointAtLength(mid).y < y) lo = mid
        else hi = mid
      }
      return (lo / total) * 100
    }
    const tweenVisible = (laneIndex: number, to: number, onDone?: () => void) => {
      // `from` is captured on the first applied frame, not at push time, so a superseding tween
      // starts from where the lane actually is.
      let from: number | null = null
      pushTween({
        channel: `vis:${laneIndex}`,
        t0: performance.now(),
        dur: CUT_MS,
        apply: (eased) => {
          if (from == null) from = visRef.current[laneIndex] ?? 100
          visRef.current[laneIndex] = from + (to - from) * eased
        },
        done: onDone,
      })
    }

    const toggleCut = (kind: 'filter' | 'health', probability: number) => {
      const gap = kind === 'filter' ? gapOf('filter') : gapOf('demote')
      if (!gap) return
      const cuts = cutsRef.current
      const current = cuts[kind]
      const other = kind === 'filter' ? cuts.health : cuts.filter
      if (current == null) {
        if (Math.random() > probability) return
        const candidates = movable.filter((i) => i !== other && (visRef.current[i] ?? 100) > 99)
        if (!candidates.length) return
        const victim = candidates[Math.floor(Math.random() * candidates.length)]
        cuts[kind] = victim
        tweenVisible(victim, cutFraction(victim, gap.top + gap.height * 0.34))
        // The name lands as the retracting tip reaches the stage.
        later(() => {
          setMarkers((prev) => [
            ...prev.filter((m) => m.key !== kind),
            {
              key: kind,
              laneIndex: victim,
              gap: kind === 'filter' ? 'filter' : 'demote',
              text: kind === 'filter' ? `✕ ${lanes[victim].name} not eligible` : `▼ ${lanes[victim].name} penalized`,
              tone: kind === 'filter' ? 'cut' : 'warn',
              transient: false,
              id: nextMarkerId(),
            },
          ])
        }, CUT_MS * 0.72)
      } else {
        cuts[kind] = null
        setMarkers((prev) => [
          ...prev.filter((m) => m.key !== kind && m.key !== 'restored'),
          {
            key: 'restored',
            laneIndex: current,
            gap: kind === 'filter' ? 'filter' : 'demote',
            text: `↩ ${lanes[current].name} back`,
            tone: 'ok',
            transient: true,
            id: nextMarkerId(),
          },
        ])
        later(() => setMarkers((prev) => prev.filter((m) => m.key !== 'restored')), 2600)
        tweenVisible(current, 100)
      }
    }

    const runSort = (sortIndex: number, from: number[][], to: number[][], onDone: () => void) => {
      const fromSets = toSlotSets(from)
      const toSets = toSlotSets(to)
      pushTween({
        channel: 'slots',
        t0: performance.now(),
        dur: SORT_MS,
        apply: (eased) => {
          slotSetsRef.current = toSets.map((set, k) => {
            if (k < sortIndex) return set
            if (k > sortIndex) return fromSets[k]
            return set.map((value, i) => fromSets[k][i] + (value - fromSets[k][i]) * eased)
          })
        },
        done: () => {
          slotSetsRef.current = toSets.map((set, k) => (k <= sortIndex ? set : fromSets[k]))
          // Geometry moved, so a lane that is currently out needs its break point refreshed.
          reanchorCuts()
          onDone()
        },
      })
    }

    /** Re-derive break points after geometry moved (a sort, a resize) so cuts stay on their stage. */
    const reanchorCuts = () => {
      const cuts = cutsRef.current
      const filterGap = gapOf('filter')
      const demoteGap = gapOf('demote')
      if (cuts.filter != null && filterGap) {
        visRef.current[cuts.filter] = cutFraction(cuts.filter, filterGap.top + filterGap.height * 0.34)
      }
      if (cuts.health != null && demoteGap) {
        visRef.current[cuts.health] = cutFraction(cuts.health, demoteGap.top + demoteGap.height * 0.34)
      }
      dirtyRef.current = true
    }
    const remeasurePoll = window.setInterval(() => {
      if (!pendingRemeasureRef.current) return
      pendingRemeasureRef.current = false
      reanchorCuts()
    }, 250)

    const refreshParticles = () => {
      lanes.forEach((lane, i) => {
        const group = laneGroupRefs.current[i]
        if (!group) return
        const { d } = buildLanePath(lane, i, geom.gaps, slotSetsRef.current)
        group.querySelectorAll('animateMotion').forEach((el) => el.setAttribute('path', d))
      })
    }

    const interval = window.setInterval(() => {
      // A hidden tab gets no frames, so a wave would only queue tweens that all resolve at once
      // on return. Skip the beat entirely and pick up on the next one.
      if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return
      const svg = svgRef.current
      svg?.classList.add('de-wave')
      const prev = orders
      const next = prev.map((order, k) => {
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
        later(() => svg?.classList.remove('de-wave'), 350)
      }

      toggleCut('filter', 0.45)
      if (geom.sortGapCount === 0) {
        later(() => {
          toggleCut('health', 0.55)
          finish()
        }, 1300)
        return
      }
      later(() => {
        runSort(0, prev, [next[0], ...prev.slice(1)], () => {
          later(() => {
            toggleCut('health', 0.55)
            if (geom.sortGapCount > 1) {
              later(() => runSort(1, [next[0], ...prev.slice(1)], next, finish), 950)
            } else {
              finish()
            }
          }, 850)
        })
      }, 1100)
    }, WAVE_MS)

    return () => {
      window.clearInterval(interval)
      window.clearInterval(remeasurePoll)
      timeouts.forEach((id) => window.clearTimeout(id))
      svgRef.current?.classList.remove('de-wave')
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drawn, lanes])

  if (!drawn) return null

  const markerTone = (tone: Marker['tone'], laneIndex: number) => {
    if (tone === 'ok') return isDark ? '#34d399' : '#047857'
    if (tone === 'warn') return isDark ? '#fbbf24' : '#b45309'
    return laneColor(lanes[laneIndex]?.color ?? '#8d96aa')
  }

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
              data-lane-mask
              d={path.d}
              fill="none"
              stroke={laneColor(path.color)}
              strokeWidth={path.width + 7}
              strokeLinecap="round"
              opacity={haloOpacity}
              pathLength={100}
              style={{ strokeDasharray: '100 100' }}
            />
            <path
              data-lane-mask
              data-lane-core
              d={path.d}
              fill="none"
              stroke={laneColor(path.color)}
              strokeWidth={path.width}
              strokeLinecap="round"
              opacity={coreOpacity}
              pathLength={100}
              style={{ strokeDasharray: '100 100' }}
            />
            {/* Ambient downstream flow; on volume splits its speed encodes the share. */}
            {path.noFlow ? null : (
              <path
                d={path.d}
                fill="none"
                stroke={laneColor(path.color)}
                strokeWidth={Math.max(1.2, path.width * 0.55)}
                strokeLinecap="round"
                opacity={0}
                className="de-lane-flow"
                style={{ animationDuration: `${path.flowDuration}s`, animationDelay: `${0.9 + i * 0.15}s` }}
              />
            )}
            {/* "Payments" riding the lane — educational moving objects, share-weighted. */}
            {Array.from({ length: path.noFlow ? 0 : path.width > 3.5 ? 2 : 1 }, (_, p) => {
              const travel = Math.min(13, Math.max(5.5, path.flowDuration * 2.8))
              return (
                <circle
                  key={p}
                  className="de-lane-particle"
                  r={Math.max(2.4, path.width * 0.75)}
                  fill={laneColor(path.color)}
                  opacity={0}
                  style={{ filter: `drop-shadow(0 0 6px ${laneColor(path.color)})` }}
                >
                  <animateMotion
                    dur={`${travel}s`}
                    begin={`${1.6 + i * 0.7 + p * (travel / 2)}s`}
                    repeatCount="indefinite"
                    path={path.d}
                  />
                  <animate
                    attributeName="opacity"
                    values="0;0.95;0.95;0"
                    keyTimes="0;0.06;0.94;1"
                    dur={`${travel}s`}
                    begin={`${1.6 + i * 0.7 + p * (travel / 2)}s`}
                    repeatCount="indefinite"
                  />
                </circle>
              )
            })}
          </g>
        ))}
      </svg>
      <div ref={overlayRef} aria-hidden="true" className="pointer-events-none absolute inset-0 z-[1]">
        {markers.map((marker) => {
          const gap = geomRef.current?.gaps.find((g) => g.kind === marker.gap)
          if (!gap) return null
          const color = markerTone(marker.tone, marker.laneIndex)
          return (
            <span
              key={marker.id}
              data-marker-lane={marker.laneIndex}
              data-marker-gap={marker.gap}
              className={`${marker.transient ? 'de-demote-flash' : 'de-cut-marker'} absolute -translate-x-1/2 whitespace-nowrap rounded-md border px-1.5 py-px font-mono text-[10px] font-semibold shadow-[0_8px_20px_-10px_rgba(15,23,42,0.55)]`}
              style={{
                left: marker.gap === 'demote'
                  ? slotSetsRef.current[0]?.[marker.laneIndex] ?? laneX(marker.laneIndex)
                  : laneX(marker.laneIndex),
                top: gap.top + gap.height * (marker.transient ? 0.46 : 0.52),
                color,
                borderColor: `${color}66`,
                background: isDark ? `${color}1f` : `${color}14`,
              }}
            >
              {marker.text}
            </span>
          )
        })}
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
            const color = laneColor(label.color ?? '#3b82f6')
            return (
              <span
                key={i}
                data-windot-lane={label.laneIndex}
                className="de-dot-breathe absolute h-[13px] w-[13px] -translate-x-1/2 -translate-y-1/2 rounded-full"
                style={{
                  left: label.x,
                  top: label.y,
                  background: color,
                  boxShadow: `0 0 14px 3px ${color}66`,
                  transition: 'opacity 0.5s',
                }}
              />
            )
          }
          if (label.kind === 'chip' || label.kind === 'endchip') {
            const isEnd = label.kind === 'endchip'
            return (
              <span
                key={i}
                data-lane={isEnd ? label.laneIndex : undefined}
                data-pinned={isEnd && label.pinned ? 'true' : undefined}
                title={label.text}
                className={`${isEnd ? 'de-end-chip ' : ''}absolute flex max-w-[80px] -translate-x-1/2 items-center gap-1 rounded-full border border-slate-200 bg-white px-1.5 py-0.5 font-mono text-[10px] text-slate-600 shadow-sm dark:border-[#1e2535] dark:bg-[#0d1118] dark:text-[#9ca7ba] dark:shadow-none`}
                style={{
                  left: label.x,
                  top: label.y,
                  transition: isEnd ? 'left 1s cubic-bezier(0.33, 0, 0.15, 1), opacity 0.5s' : undefined,
                }}
              >
                {label.color ? (
                  <span
                    className="h-[7px] w-[7px] flex-shrink-0 rounded-[3px]"
                    style={{ background: laneColor(label.color) }}
                  />
                ) : null}
                <span className="min-w-0 truncate">{label.text}</span>
              </span>
            )
          }
          if (label.kind === 'pct') {
            return (
              <span
                key={i}
                className="absolute -translate-x-1/2 rounded-md border border-slate-200 bg-white px-1.5 font-mono text-[10px] font-semibold tabular-nums shadow-sm dark:border-[#1e2535] dark:bg-[#0d1118] dark:shadow-none"
                style={{ left: label.x, top: label.y, color: laneColor(label.color ?? '#3b82f6') }}
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
              style={{ left: label.x, top: label.y, color: laneColor(label.color ?? '#8d96aa') }}
            >
              {label.text}
            </span>
          )
        })}
      </div>
    </>
  )
}
