import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { apiPost } from '../../lib/api'
import { Handshake, RotateCcw } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { useVolumeCommitmentDashboard } from '../../hooks/useVolumeCommitment'
import {
  CommitmentPacingChart,
  PacingStatus,
  PacingWindowPicker,
  pacingAxis,
  pacingWindows,
} from './CommitmentPacingChart'
import { VolumeContractFeatureNotice } from './VolumeContractFeatureNotice'
import type { BlockedCommitment, SteerBlock } from '../../types/api'
import {
  SECS_PER_DAY,
  SolidSwatch,
  bucketsPerDay,
  firstEliminationByConnector,
  formatAchieved,
  formatMoney,
  pctOfGoal,
  isTestCycle as isTestCycleOf,
  toMajorUnits,
} from './volumeCommitmentChartBits'

/** The slice of a simulator row this card reads. */
export type SteerableResult = {
  decidedGateway: string
  /** Whether the volume-commitment engine moved this payment, from `volume_steer_info.outcome`. */
  steerOutcome?: 'STEERED' | 'SR_PREVAILED' | null
  /** The PSP approval-rate routing had picked, when the payment was steered elsewhere. */
  steerSrHead?: string | null
  /** Commitments that wanted this payment and the gate that stopped each. */
  steerBlocked?: BlockedCommitment[] | null
}

/**
 * Why a commitment took no payments, in the reader's terms.
 *
 * A steer rate is a share of the payments a commitment is *allowed* to take, and most payments
 * reach none of them. Without this, "Steering · 62% of eligible" beside zero steered payments
 * reads as a broken engine rather than an empty eligible set.
 */
const GATE_REASONS: Record<SteerBlock, (n: number) => string> = {
  ALREADY_CHOSEN: (n) => `routing already chose it ${n === 1 ? 'once' : `${n} times`}`,
  NOT_OFFERED: (n) => `routing did not offer it on ${n} payment${n === 1 ? '' : 's'}`,
  OUTSIDE_TOLERANCE: (n) => `${n} payment${n === 1 ? '' : 's'} outside its approval-rate budget`,
  CYCLE_CLOSED: (n) => `its cycle had closed on ${n} payment${n === 1 ? '' : 's'}`,
  LOST_ROLL: (n) => `${n} eligible, none drawn`,
  UNKNOWN: (n) => `${n} payment${n === 1 ? '' : 's'} held back`,
}

/** Tighter than the default polls while someone is watching a run; a test cycle is only minutes. */
const RUN_POLL_MS = 5_000

/**
 * A steer rate at or above this takes every payment its PSP is allowed to take, so nothing is left
 * over for the commitments ranked below it. Not exactly 1 — the rate is a computed ratio, and a
 * hair under one still leaves no practical share for anyone behind.
 */
const SATURATED_STEER_RATE = 0.995

/** Extra payments over the seconds left, so latency cannot end the run before the cycle does. */
const RUN_HEADROOM = 3

type Row = {
  name: string
  color: string
  auth: number
  steered: number
  ceded: number
  status: PacingStatus
  steerRate: number
  achieved: number
  goal: number
  reason?: string
  /** Why this commitment took nothing, when it was set to steer and took nothing. */
  noneEligible?: string
  /** A richer commitment that is already taking every payment it may, leaving this one nothing.
   *  The engine rolls its plan in reward order, so a saturated PSP above this one in that order
   *  wins every contested payment — this one's own steer rate is never actually spent. */
  blockedBy?: string
}

function StatusBadge({ row }: { row: Row }) {
  switch (row.status) {
    case 'met':
      return <Badge variant="green">Met</Badge>
    case 'eliminated':
      return (
        <span title={row.reason}>
          <Badge variant="red">Eliminated</Badge>
        </span>
      )
    case 'steering':
      // Marked for steering but out-ranked: reporting its share would read the same as a PSP
      // actually taking that share, when in practice it takes nothing.
      return row.blockedBy ? (
        <span
          title={`${row.blockedBy} pays more and is already taking every payment it may, so it wins every contested payment. ${row.name} would take ${(row.steerRate * 100).toFixed(0)}% of what is eligible, but only gets a payment ${row.blockedBy} cannot.`}
        >
          <Badge variant="gray">Queued behind {row.blockedBy}</Badge>
        </span>
      ) : (
        <Badge variant="orange">Steering · {(row.steerRate * 100).toFixed(0)}% of eligible</Badge>
      )
    case 'on_pace':
      return <Badge variant="green">On pace</Badge>
    case 'missed':
      return <Badge variant="gray">Missed</Badge>
    default:
      return <Badge variant="gray">Pending forecast</Badge>
  }
}

/**
 * The volume contract on the simulator page: where each commitment stands, and the controls for
 * driving traffic at it.
 *
 * These were two cards. The second held a countdown, two buttons, and a copy of the standings this
 * one already showed in more detail — so a reader comparing them found the same PSP twice, once
 * with its verdict and once without. There is one subject here, and it reads as one card.
 */
export function VolumeCommitmentRunChart({
  merchantId,
  results,
  colorFor,
  tps,
  isSimulating,
  onLoad,
  onContractGone,
  onCycleEnded,
}: {
  merchantId: string | null
  results: SteerableResult[]
  colorFor: (gateway: string) => string
  /** Payments the run fires per second — the divisor that turns a daily total into a ticket size. */
  tps: number
  isSimulating: boolean
  onLoad: (preset: {
    gateways: string[]
    amount: number
    totalPayments: number
    paceMs: number
  }) => void
  /** Called when no contract is available, so the page can drop a pace set by an earlier Load. */
  onContractGone: () => void
  /** Called when the cycle closes mid-run: volume sent past it lands in the next period. */
  onCycleEnded: () => void
}) {
  // One request for pacing, series and audit. The bucket size depends on the contract-day length,
  // which arrives in the same response, so the first fetch comes back at whole-day resolution and
  // the next poll refines it — `keepPreviousData` keeps the chart from blanking in between.
  const [daySecs, setDaySecs] = useState<number | null | undefined>(undefined)
  const dashboard = useVolumeCommitmentDashboard(merchantId ?? undefined, {
    perDay: bucketsPerDay(daySecs),
    refreshInterval: RUN_POLL_MS,
  })
  useEffect(() => {
    if (dashboard.pacing?.daySecs !== undefined) setDaySecs(dashboard.pacing.daySecs)
  }, [dashboard.pacing?.daySecs])
  const pacing = { data: dashboard.pacing }
  const audit = { runs: dashboard.runs, events: dashboard.events }
  const active = Boolean(dashboard.pacing?.active)
  const connectors = useMemo(() => dashboard.series?.connectors ?? [], [dashboard.series])
  const currency = dashboard.series?.currency

  // How much of the cycle is left. It lived beside the simulator's controls, which is where you
  // press things, not where you read where the commitments stand — and every verdict on this card
  // is a statement about the time remaining, so the countdown belongs next to them.
  const [, setClockTick] = useState(0)
  const cycleEndMs = dashboard.pacing?.cycleEnd ? Date.parse(dashboard.pacing.cycleEnd) : 0
  const isTest = isTestCycleOf(dashboard.pacing?.daySecs ?? SECS_PER_DAY)
  useEffect(() => {
    // Seconds matter on a test cycle, where a contract day is a minute. On a calendar cycle the
    // countdown reads in days and a minute's resolution is already more than it can show.
    if (!cycleEndMs) return undefined
    const id = window.setInterval(() => setClockTick((t) => t + 1), isTest ? 1_000 : 60_000)
    return () => window.clearInterval(id)
  }, [cycleEndMs, isTest])
  const secondsLeft = cycleEndMs ? Math.max(0, Math.round((cycleEndMs - Date.now()) / 1000)) : 0
  // Null where the document mixes billing cycles: there is then no single end to count down to.
  const cycleDays = dashboard.pacing?.daysTotal ?? null

  // The window control belongs with the title, not stranded above the plot, so this card owns the
  // value and hands it to the chart.
  const seriesDaySecs = dashboard.series?.daySecs ?? daySecs
  const axisDays = useMemo(
    () => pacingAxis(connectors, seriesDaySecs).daysTotal,
    [connectors, seriesDaySecs],
  )
  const [chartWindow, setChartWindow] = useState<number | 'all' | null>(null)
  const [restarting, setRestarting] = useState(false)

  /**
   * Start the cycle again from day 0.
   *
   * A `test_minutes` cycle is anchored to the contract rule's `modified_at`, so a restart is a
   * re-stamp of it. `activate` does that stamping — but only on the path that actually changes
   * which rule is live; re-activating the rule already in the slot returns early without touching
   * it, which is right for an idempotent activate and useless here. Deactivating first clears the
   * slot, so the activate that follows takes the insert path and stamps.
   *
   * Two existing endpoints rather than a third for this alone. The cost is a moment with no
   * contract live, which is a restart's own semantics anyway.
   *
   * Offered only on a test cycle: a calendar cycle is anchored to a day of the month, which no
   * amount of re-stamping moves, so the control would claim to do something it cannot.
   */
  async function restartCycle() {
    const ruleId = dashboard.pacing?.ruleId
    if (!ruleId || !merchantId || restarting) return
    setRestarting(true)
    try {
      const body = { created_by: merchantId, routing_algorithm_id: ruleId }
      await apiPost('/routing/deactivate', body)
      await apiPost('/routing/activate', body)
      await dashboard.mutate()
    } finally {
      setRestarting(false)
    }
  }
  // `null` until the series arrives: the default depends on how long the cycle is, which is not
  // known at mount.
  const windowValue = chartWindow ?? pacingWindows(axisDays).initial

  const steeredCount = useMemo(
    () => results.filter((r) => r.steerOutcome === 'STEERED').length,
    [results],
  )

  // Drop times for *this* cycle only; an old cycle's elimination would pin the marker at minute 0.
  const currentRunId = audit.runs.find((r) => r.isCurrent)?.runId
  const eliminatedAtMs = useMemo(
    () => (currentRunId ? firstEliminationByConnector(audit.events, currentRunId) : new Map<string, number>()),
    [audit.events, currentRunId],
  )

  const rows = useMemo<Row[]>(() => {
    const psps = pacing.data?.psps ?? []
    const eliminated = pacing.data?.eliminated ?? []
    const names = [...new Set<string>([...connectors.map((c) => c.connector), ...psps.map((p) => p.connector), ...eliminated.map((e) => e.connector)])]
    return names.map((name) => {
      let auth = 0
      let steered = 0
      let ceded = 0
      for (const r of results) {
        const wasSteered = r.steerOutcome === 'STEERED'
        if (r.decidedGateway === name) {
          if (wasSteered) steered += 1
          else auth += 1
        }
        if (wasSteered && r.steerSrHead === name) ceded += 1
      }
      const live = psps.find((p) => p.connector === name)
      const dropped = eliminated.find((e) => e.connector === name)
      const seriesFor = connectors.find((c) => c.connector === name)
      const goal = live?.goal ?? seriesFor?.goal ?? 0
      const achieved =
        live?.achieved ??
        dropped?.achieved ??
        seriesFor?.points.reduce((s, p) => s + p.total, 0) ??
        0
      const status: PacingStatus = goal > 0 && achieved >= goal
        ? 'met'
        : dropped
          ? 'eliminated'
          : live
            ? live.steering
              ? 'steering'
              : 'on_pace'
            : 'pending'
      // The gate that stopped this commitment most often. One reason, not a tally: the row is a
      // line of text, and the dominant gate is the one that explains the zero.
      let noneEligible: string | undefined
      if (steered === 0) {
        const byGate = new Map<SteerBlock, number>()
        for (const r of results) {
          for (const b of r.steerBlocked ?? []) {
            if (b.connector !== name) continue
            byGate.set(b.gate, (byGate.get(b.gate) ?? 0) + 1)
          }
        }
        const top = [...byGate.entries()].sort((a, b) => b[1] - a[1])[0]
        if (top) noneEligible = GATE_REASONS[top[0]]?.(top[1]) ?? GATE_REASONS.UNKNOWN(top[1])
      }

      // Who, if anyone, is soaking up every payment this one is waiting for.
      const blockedBy =
        live?.steering
          ? psps
              .filter(
                (p) =>
                  p.steering &&
                  p.reward > live.reward &&
                  p.steerRate >= SATURATED_STEER_RATE,
              )
              .sort((a, b) => b.reward - a.reward)[0]?.connector
          : undefined

      return {
        name,
        color: colorFor(name),
        auth,
        steered,
        ceded,
        status,
        steerRate: live?.steerRate ?? 0,
        achieved,
        goal,
        reason: dropped?.reason,
        noneEligible,
        blockedBy,
      }
    })
  }, [pacing.data, connectors, results, colorFor])

  const statusFor = useCallback(
    (name: string): PacingStatus => rows.find((r) => r.name === name)?.status ?? 'pending',
    [rows],
  )
  const reasons = useMemo(
    () => new Map((pacing.data?.eliminated ?? []).map((e) => [e.connector, e.reason])),
    [pacing.data],
  )

  // ── Driving traffic at the contract ─────────────────────────────────────────────────────
  const cycleOver = Boolean(cycleEndMs) && secondsLeft === 0
  const contractDaySecs = dashboard.pacing?.daySecs ?? SECS_PER_DAY
  const expectedDaily = dashboard.pacing?.expectedDailyTraffic ?? 0
  // Volume rate is a contract term: `expectedDaily` per contract day, however many payments carry
  // it. TPS only decides how finely that is chopped — more payments of proportionally less each.
  const paymentsPerDay = Math.max(1, Math.round(tps * contractDaySecs))
  // The rate is a contract figure in canonical minor units; the ticket is what a payment carries,
  // so it goes on the wire in the major units `/decide-gateway` reads amounts in.
  const perPayment =
    expectedDaily > 0
      ? Math.max(0.01, Math.round((toMajorUnits(expectedDaily, currency) / paymentsPerDay) * 100) / 100)
      : 1000
  const paceMs = Math.max(1, Math.round((contractDaySecs * 1000) / paymentsPerDay))
  // Over-provisioned so the cycle, never the count, ends the run; `onCycleEnded` discards the rest.
  const totalPayments = Math.max(1, Math.round(tps * secondsLeft * RUN_HEADROOM))

  // The page's callbacks change identity every render; the effects below key on facts, not on them.
  const callbacks = useRef({ onContractGone, onCycleEnded })
  callbacks.current = { onContractGone, onCycleEnded }

  const hasContract = active && rows.length > 0
  useEffect(() => {
    if (!hasContract) callbacks.current.onContractGone()
  }, [hasContract])

  // A run outliving its cycle delivers into the next period, against goals that have just reset.
  useEffect(() => {
    if (cycleOver && isSimulating) callbacks.current.onCycleEnded()
  }, [cycleOver, isSimulating])

  // Refetch on cycle close so the countdown does not sit at "Cycle over" until the next poll.
  const { mutate } = dashboard
  useEffect(() => {
    if (!cycleOver) return
    void mutate()
  }, [cycleOver, mutate])

  // The simulator takes its settings from the contract rather than waiting to be pointed at it.
  // Ticket size, pace and the eligible gateways are all contract terms — a run driven by anything
  // else measures nothing about the commitments on this card, and the numbers under each promise
  // only mean what they say because the traffic matches the rate the document declares.
  //
  // Applied once per cycle, not once per render: the values it carries move with the clock
  // (`totalPayments` shrinks as the cycle runs down), so they are read from a ref and the effect
  // keys on the contract and cycle alone. A new cycle re-applies, which is what makes a closed
  // one's run stop and the next one start at its own full length.
  const preset = useRef({ gateways: [] as string[], amount: 0, totalPayments: 0, paceMs: 0 })
  preset.current = { gateways: rows.map((r) => r.name), amount: perPayment, totalPayments, paceMs }
  const onLoadRef = useRef(onLoad)
  onLoadRef.current = onLoad
  const appliedCycle = useRef<string | null>(null)
  const liveCycle =
    active && !cycleOver
      ? `${dashboard.pacing?.ruleId ?? ''}|${dashboard.pacing?.cycleStart ?? ''}`
      : ''
  useEffect(() => {
    // Never mid-run: rewriting the ticket size under a run in flight would split its results
    // across two different sets of terms.
    if (!liveCycle || isSimulating || appliedCycle.current === liveCycle) return
    if (preset.current.gateways.length === 0) return
    appliedCycle.current = liveCycle
    onLoadRef.current({ ...preset.current })
  }, [liveCycle, isSimulating])

  // The contract is activated but the feature flag is off, so nothing is paced and the endpoints
  // report no plan. Explain that rather than rendering nothing — this card vanishing is otherwise
  // indistinguishable from the merchant having no contract at all.
  if (dashboard.pacing?.contractConfigured && !dashboard.pacing.featureEnabled) {
    return (
      <Card>
        <CardBody className="space-y-3">
          <div className="flex items-center gap-2">
            <Handshake size={15} className="text-brand-500" />
            <span className="text-sm font-medium text-slate-800 dark:text-white">
              Active volume contract
            </span>
            <Badge variant="orange">Not routing</Badge>
          </div>
          <VolumeContractFeatureNotice merchantId={merchantId} onEnabled={() => void mutate()} />
        </CardBody>
      </Card>
    )
  }

  // Nothing to say until a contract is live or this run has actually steered something.
  if (!active && steeredCount === 0) return null
  if (rows.length === 0) return null

  const total = results.length

  return (
    <Card>
      <CardBody className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Handshake size={15} className="text-brand-500" />
            <span className="text-sm font-medium text-slate-800 dark:text-white">Cumulative volume vs. each promise</span>
            {cycleEndMs && secondsLeft === 0 ? (
              <Badge variant="red">Cycle over</Badge>
            ) : cycleDays == null ? (
              <Badge variant="gray">Mixed cycles</Badge>
            ) : isTest ? (
              <Badge variant="orange">
                Test cycle · {secondsLeft}s left of {cycleDays} min
              </Badge>
            ) : (
              <Badge variant="blue">
                {cycleDays}-day cycle
                {cycleEndMs ? ` · ${Math.ceil(secondsLeft / 86_400)} days left` : ''}
              </Badge>
            )}
          </div>
          <div className="ml-auto flex shrink-0 items-center gap-2">
            {isTest && dashboard.pacing?.ruleId && (
              <button
                type="button"
                onClick={() => void restartCycle()}
                disabled={restarting}
                title="Restart this test cycle from day 0. Delivered volume resets and every commitment is forecast afresh."
                aria-label="Restart cycle"
                className="inline-flex items-center justify-center rounded-md border border-slate-200 p-1.5 text-slate-500 transition-colors hover:text-slate-800 disabled:opacity-50 dark:border-[#1f1f29] dark:text-slate-400 dark:hover:text-white"
              >
                <RotateCcw size={13} className={restarting ? 'animate-spin' : undefined} />
              </button>
            )}
            {connectors.length > 0 && (
                <PacingWindowPicker
                daysTotal={axisDays}
                daySecs={seriesDaySecs}
                value={windowValue}
                onChange={setChartWindow}
              />
            )}
          </div>
        </div>

        <span className="block text-xs tabular-nums text-slate-500 dark:text-slate-400">
          {steeredCount.toLocaleString()} of {total.toLocaleString()} payments steered
          {total > 0 ? ` · ${((steeredCount / total) * 100).toFixed(1)}%` : ''}
          {currentRunId && <span className="ml-2 font-mono text-[10px] opacity-60">{currentRunId}</span>}
        </span>

        {connectors.length > 0 ? (
          <CommitmentPacingChart
            connectors={connectors}
            currency={currency}
            daySecs={seriesDaySecs}
            window={windowValue}
            colorFor={colorFor}
            statusFor={statusFor}
            eliminatedAtMs={eliminatedAtMs}
            eliminationReasons={reasons}
            height={460}
          />
        ) : (
          <p className="text-xs text-slate-500 dark:text-slate-400">Waiting for the contract&apos;s first measurements.</p>
        )}

        {/* One line per PSP: its standing, and where this run's payments to it came from. */}
        <div className="grid gap-x-6 gap-y-1.5 sm:grid-cols-2">
          {rows.map((r) => (
            <div key={r.name} className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs">
              <SolidSwatch color={r.color} />
              <span className="min-w-0 truncate font-medium text-slate-800 dark:text-white">{r.name}</span>
              <StatusBadge row={r} />
              <span className="ml-auto whitespace-nowrap tabular-nums text-slate-500 dark:text-slate-400">
                {r.goal > 0 && (
                  <>
                    {formatAchieved(r.achieved, r.goal, currency)}/{formatMoney(r.goal, currency)}
                    {r.status !== 'eliminated' && ` · ${pctOfGoal(r.achieved, r.goal)}%`}
                    {' · '}
                  </>
                )}
                {r.auth} by approval · {r.steered} steered in
                {r.ceded > 0 ? ` · ${r.ceded} ceded` : ''}
                {r.noneEligible && (
                  <span className="text-amber-600 dark:text-amber-500">
                    {' · '}0 eligible: {r.noneEligible}
                  </span>
                )}
              </span>
            </div>
          ))}
        </div>

        {cycleOver && (
          <p className="text-xs text-slate-500 dark:text-slate-400">
            This cycle has closed — its goals are settled and a new one has begun, with delivery
            back at zero. Any run still going was stopped, because volume sent now counts toward the
            next period. The simulator picks up the fresh cycle on its own; deactivate the contract
            from the Volume Contracts page to stop.
          </p>
        )}
      </CardBody>
    </Card>
  )
}
