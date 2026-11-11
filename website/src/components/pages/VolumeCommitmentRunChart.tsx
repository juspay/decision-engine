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
  toMajorUnits,
  isTestCycle as isTestCycleOf,
} from './volumeCommitmentChartBits'

/**
 * What a run has to be given to drive traffic at the contract: the PSPs the document names, so
 * every payment is one the commitments can compete for, and the ticket each payment carries.
 *
 * No pace. This used to carry one too, derived from the contract's declared daily volume, which
 * made a contract run at a rate no other simulation on this page runs at — one payment a second
 * where the rest go as fast as the backend answers. The contract's own figures are calibrated to
 * the ordinary rate instead, so a contract demo and a routing demo push traffic the same way and
 * only the promises differ.
 *
 * The ticket stays, because rate and ticket are not interchangeable here: how fast payments go is
 * this page's business, but how much each one is worth is what every target on this card is
 * measured in.
 */
export type ContractRunPreset = {
  gateways: string[]
  /** Major units per payment, or null when the contract declares no daily volume to derive it
   *  from — the page then keeps whatever amount it was already set to. */
  ticket: number | null
}

/** What this card counted for one PSP over the run. */
type ConnectorTally = {
  /** Payments approval-rate routing sent here of its own accord. */
  auth: number
  /** Payments the commitment engine moved here. */
  steered: number
  /** How often each gate stopped a steer to this PSP — the reason shown beside a zero. */
  blocked: Partial<Record<SteerBlock, number>>
}

/**
 * The run's payments as this card needs them: counted, not listed.
 *
 * It used to take the rows themselves and count them on every render — O(rows) per connector, on
 * a flush cadence that does not slow as rows accumulate, so the cost of a payment climbed with
 * the number already on screen. Measured across one cycle: 24ms of client time per payment early
 * in a run against 309ms by the end of it, while the backend answered in 48ms throughout. Folding
 * each row in once as it lands is O(1), which is what keeps a run's rate flat enough for a demo's
 * numbers to repeat.
 */
export type ContractRunTally = {
  /** Payments the run has completed. */
  total: number
  /** Of those, the ones the commitment engine moved. */
  steered: number
  byConnector: Record<string, ConnectorTally>
}

/** A tally with nothing counted yet — a run not started, or one just cleared. */
export function emptyRunTally(): ContractRunTally {
  return { total: 0, steered: 0, byConnector: {} }
}

/**
 * Count a whole set of rows at once — for rows that arrive already complete, restored from a
 * snapshot rather than counted as they landed. One pass, not one per render.
 */
export function tallyOf(
  rows: {
    decidedGateway: string
    steerOutcome?: 'STEERED' | 'SR_PREVAILED' | null
    steerBlocked?: BlockedCommitment[] | null
  }[],
): ContractRunTally {
  const tally = emptyRunTally()
  for (const row of rows) countPayment(tally, row)
  return tally
}

/** Fold one completed payment into a tally, in place. */
export function countPayment(
  tally: ContractRunTally,
  row: {
    decidedGateway: string
    steerOutcome?: 'STEERED' | 'SR_PREVAILED' | null
    steerBlocked?: BlockedCommitment[] | null
  },
) {
  const of = (connector: string) =>
    (tally.byConnector[connector] ??= { auth: 0, steered: 0, blocked: {} })

  tally.total += 1
  const wasSteered = row.steerOutcome === 'STEERED'
  if (wasSteered) tally.steered += 1
  if (row.decidedGateway) {
    const entry = of(row.decidedGateway)
    if (wasSteered) entry.steered += 1
    else entry.auth += 1
  }
  for (const block of row.steerBlocked ?? []) {
    const entry = of(block.connector)
    entry.blocked[block.gate] = (entry.blocked[block.gate] ?? 0) + 1
  }
}

/**
 * Why a commitment steered nothing, in the reader's terms. Each reads as a clause completing
 * "0 steered in — ...", so they all take the same shape.
 *
 * A steer rate is a share of the payments a commitment is *allowed* to take, and most payments
 * reach none of them. Without this, "Steering · 62% of eligible" beside zero steered payments
 * reads as a broken engine rather than an empty eligible set.
 *
 * `ALREADY_CHOSEN` has no entry, and is not counted towards the dominant gate: it is the one
 * block that is not a refusal — routing had picked that PSP already, so the commitment was being
 * served, not denied. It is reported by the payment it kept, in the row's "by approval" count.
 * Counted here it would win the tally on any PSP routing favours and hide the gate that does
 * explain the zero.
 */
type DenialGate = Exclude<SteerBlock, 'ALREADY_CHOSEN'>

const GATE_REASONS: Record<DenialGate, (n: number) => string> = {
  NOT_OFFERED: (n) => `not offered on ${n} payment${n === 1 ? '' : 's'}`,
  OUTSIDE_TOLERANCE: (n) =>
    `outside its approval-rate budget on ${n} payment${n === 1 ? '' : 's'}`,
  CYCLE_CLOSED: (n) => `its cycle had closed on ${n} payment${n === 1 ? '' : 's'}`,
  LOST_ROLL: (n) => `eligible on ${n}, none drawn`,
  UNKNOWN: (n) => `held back on ${n} payment${n === 1 ? '' : 's'}`,
}

/** A block that actually denied the payment; see `GATE_REASONS` for the one that does not. */
function isDenial(gate: SteerBlock): gate is DenialGate {
  return gate !== 'ALREADY_CHOSEN'
}

/**
 * Tighter than the default polls while someone is watching a run; a test cycle is only minutes,
 * and its series is bucketed by the second, so the poll is what decides how often those points
 * actually reach the chart.
 */
const RUN_POLL_MS = 2_000

/**
 * Payments a second the simulator sustains once a run is going, which is what turns the contract's
 * declared daily volume into a per-payment ticket.
 *
 * A constant rather than the TPS control, because the run is not throttled to it: payments go as
 * fast as the backend answers, and TPS only sizes the batch. Held a little under the ~24/s
 * measured so a slower machine sends slightly larger tickets and still delivers the declared rate,
 * which is the same margin the shipped sample targets are chosen with.
 */
const SIMULATOR_PAYMENTS_PER_SEC = 20

/**
 * A steer rate at or above this takes every payment its PSP is allowed to take, so nothing is left
 * over for the commitments ranked below it. Not exactly 1 — the rate is a computed ratio, and a
 * hair under one still leaves no practical share for anyone behind.
 */
const SATURATED_STEER_RATE = 0.995

type Row = {
  name: string
  color: string
  auth: number
  steered: number
  status: PacingStatus
  steerRate: number
  achieved: number
  goal: number
  reason?: string
  /** Why this commitment took nothing, when it was set to steer and took nothing. */
  noSteerReason?: string
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
  tally,
  colorFor,
  isSimulating,
  onLoad,
  onContractGone,
  onCycleEnded,
  onPrepareRun,
}: {
  merchantId: string | null
  /** The run's payments, counted as they landed — see `ContractRunTally`. */
  tally: ContractRunTally
  colorFor: (gateway: string) => string
  isSimulating: boolean
  onLoad: (preset: ContractRunPreset) => void
  /** Called when no contract is available, so the page can drop a pace set by an earlier Load. */
  onContractGone: () => void
  /** Called when the cycle closes mid-run: volume sent past it lands in the next period. */
  onCycleEnded: () => void
  /**
   * Hands the page the work to do before a run's first payment: opening the cycle that run will
   * race, and reporting the terms it should race on. Registered once; the page awaits it from its
   * own start path, which is the only place that can order it before the run rather than during
   * it, and takes the terms from the return rather than from state a render behind.
   */
  onPrepareRun: (prepare: () => Promise<ContractRunPreset | null>) => void
}) {
  // One request for pacing, series and audit. The bucket size depends on the contract-day length,
  // which arrives in the same response, so the first fetch comes back at whole-day resolution and
  // the next poll refines it — `keepPreviousData` keeps the chart from blanking in between.
  const [daySecs, setDaySecs] = useState<number | null | undefined>(undefined)
  // The card follows the clock only while a run is in flight — running or paused, which is what
  // `isSimulating` covers. Off a run there is nothing arriving to plot, and a `test_minutes` cycle
  // repeats the instant it closes, so a card that kept following would answer with a cycle that
  // has delivered nothing: the standings, the eliminations and the wedge the run was watched for,
  // replaced by an empty chart seconds after the verdict landed. The last payload from the run is
  // kept and shown instead, until the next run opens a cycle of its own.
  //
  // The hold is of the whole payload. `run_id` cannot do it: the dashboard's pacing block is the
  // live plan whatever run is asked for.
  const dashboard = useVolumeCommitmentDashboard(merchantId ?? undefined, {
    perDay: bucketsPerDay(daySecs),
    // Between runs the view is frozen; polling it only invites the next cycle in.
    refreshInterval: isSimulating ? RUN_POLL_MS : 0,
  })
  const live = {
    pacing: dashboard.pacing,
    series: dashboard.series,
    runs: dashboard.runs,
    events: dashboard.events,
  }
  // The cycle this run is watching. A payload for a different one means the cycle rolled under the
  // run — its close was never rendered — and the race on screen is already over, so the last
  // payload for it stands rather than being replaced by the successor's empty one.
  const runCycleStart = useRef<string | null>(null)
  const kept = useRef<typeof live | null>(null)
  const liveCycleStart = live.pacing?.cycleStart ?? null
  const rolled = Boolean(
    runCycleStart.current && liveCycleStart && liveCycleStart !== runCycleStart.current,
  )
  // Before the first run there is nothing kept, so the contract's current standing is what shows.
  if ((isSimulating || !kept.current) && !rolled) {
    runCycleStart.current = liveCycleStart
    kept.current = live
  }
  const view = isSimulating && !rolled ? live : kept.current ?? live

  useEffect(() => {
    if (view.pacing?.daySecs !== undefined) setDaySecs(view.pacing.daySecs)
  }, [view.pacing?.daySecs])
  const pacing = { data: view.pacing }
  const audit = { runs: view.runs, events: view.events }
  const active = Boolean(view.pacing?.active)
  const connectors = useMemo(() => view.series?.connectors ?? [], [view.series])
  const currency = view.series?.currency

  // How much of the cycle is left. It lived beside the simulator's controls, which is where you
  // press things, not where you read where the commitments stand — and every verdict on this card
  // is a statement about the time remaining, so the countdown belongs next to them.
  const [, setClockTick] = useState(0)
  const cycleEndMs = view.pacing?.cycleEnd ? Date.parse(view.pacing.cycleEnd) : 0
  const isTest = isTestCycleOf(view.pacing?.daySecs ?? SECS_PER_DAY)
  useEffect(() => {
    // Seconds matter on a test cycle, where a contract day is a minute. On a calendar cycle the
    // countdown reads in days and a minute's resolution is already more than it can show.
    if (!cycleEndMs) return undefined
    const id = window.setInterval(() => setClockTick((t) => t + 1), isTest ? 1_000 : 60_000)
    return () => window.clearInterval(id)
  }, [cycleEndMs, isTest])
  const secondsLeft = cycleEndMs ? Math.max(0, Math.round((cycleEndMs - Date.now()) / 1000)) : 0
  // Null where the document mixes billing cycles: there is then no single end to count down to.
  const cycleDays = view.pacing?.daysTotal ?? null

  // The window control belongs with the title, not stranded above the plot, so this card owns the
  // value and hands it to the chart.
  const seriesDaySecs = view.series?.daySecs ?? daySecs
  const axisDays = useMemo(
    () => pacingAxis(connectors, seriesDaySecs).daysTotal,
    [connectors, seriesDaySecs],
  )
  const [chartWindow, setChartWindow] = useState<number | 'all' | null>(null)
  const [restarting, setRestarting] = useState(false)
  const [restartError, setRestartError] = useState<string | null>(null)

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
    const ruleId = view.pacing?.ruleId
    if (!ruleId || restarting) return
    await restartCycleFor(ruleId)
  }

  /** The restart itself, for a rule the caller has already established. */
  async function restartCycleFor(ruleId: string) {
    if (!merchantId) return
    setRestarting(true)
    setRestartError(null)
    try {
      const body = { created_by: merchantId, routing_algorithm_id: ruleId }
      await apiPost('/routing/deactivate', body)
      await apiPost('/routing/activate', body)
      // Adopt the new cycle from the refetch itself. Clearing what is kept before it arrives
      // would let the payload still in hand — the closed cycle's — be taken for the new one, and
      // every later payload would then read as a roll and freeze the card on a dead race.
      const fresh = await dashboard.mutate()
      runCycleStart.current = fresh?.pacing.cycleStart ?? null
      kept.current = fresh
        ? {
            pacing: fresh.pacing,
            series: fresh.series,
            runs: fresh.audit.runs,
            events: fresh.audit.events,
          }
        : null
    } catch (e) {
      setRestartError(e instanceof Error ? e.message : 'Could not start the next cycle')
    } finally {
      setRestarting(false)
    }
  }
  // `null` until the series arrives: the default depends on how long the cycle is, which is not
  // known at mount.
  const windowValue = chartWindow ?? pacingWindows(axisDays).initial

  const steeredCount = tally.steered

  // Drop times for *this* cycle only; an old cycle's elimination would pin the marker at minute 0.
  const currentRunId = audit.runs.find((r) => r.isCurrent)?.runId
  const eliminatedAtMs = useMemo(
    () => (currentRunId ? firstEliminationByConnector(audit.events, currentRunId) : new Map<string, number>()),
    [audit.events, currentRunId],
  )

  // Once the cycle has closed there is no more traffic to come, so a commitment short of its goal
  // has missed it rather than being behind on it.
  const cycleOver = Boolean(cycleEndMs) && secondsLeft === 0

  const rows = useMemo<Row[]>(() => {
    const psps = pacing.data?.psps ?? []
    const eliminated = pacing.data?.eliminated ?? []
    const names = [...new Set<string>([...connectors.map((c) => c.connector), ...psps.map((p) => p.connector), ...eliminated.map((e) => e.connector)])]
    return names.map((name) => {
      const counted = tally.byConnector[name]
      const auth = counted?.auth ?? 0
      const steered = counted?.steered ?? 0
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
        : cycleOver
          ? 'missed'
          : dropped
            ? 'eliminated'
            : live
              ? live.steering
                ? 'steering'
                : 'on_pace'
              : 'pending'
      // The gate that denied this commitment most often. One reason, not a tally: the row is a
      // line of text, and the dominant gate is the one that explains the zero.
      let noSteerReason: string | undefined
      if (steered === 0) {
        const top = Object.entries(counted?.blocked ?? {})
          .filter((entry): entry is [DenialGate, number] => isDenial(entry[0] as SteerBlock))
          .sort((a, b) => b[1] - a[1])[0]
        if (top) noSteerReason = GATE_REASONS[top[0]]?.(top[1]) ?? GATE_REASONS.UNKNOWN(top[1])
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
        status,
        steerRate: live?.steerRate ?? 0,
        achieved,
        goal,
        reason: dropped?.reason,
        noSteerReason,
        blockedBy,
      }
    })
  }, [pacing.data, connectors, tally, colorFor, cycleOver])

  const statusFor = useCallback(
    (name: string): PacingStatus => rows.find((r) => r.name === name)?.status ?? 'pending',
    [rows],
  )
  const reasons = useMemo(
    () => new Map((pacing.data?.eliminated ?? []).map((e) => [e.connector, e.reason])),
    [pacing.data],
  )


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


  // A run starts its own cycle, and it has to exist before the run's first payment does.
  //
  // Watching `isSimulating` instead put the restart *inside* the run, where it raced everything
  // it touched: for the moment between deactivating and activating, the card still held the
  // closed cycle, so it reported that cycle over and the page aborted the run it had just
  // started. The page asks for the cycle now, and waits for it.
  //
  // Re-registered on every render rather than on mount alone. Mounting is not a reliable moment
  // to hand something up: a card already on screen does not mount again, so a registration made
  // only then is missing for the whole life of that card if anything about the arrangement
  // changed after it appeared — and a page whose reference is null simply skips the step and
  // runs against whatever cycle happened to be open, which is what this exists to prevent.
  const prepareRun = useRef<() => Promise<ContractRunPreset | null>>(async () => null)
  prepareRun.current = async () => {
    // Decided on a fresh reading rather than on what is displayed. Between runs this card holds
    // a frozen payload and stops polling, so what it shows can predate the contract the run is
    // about to race — a document created since — or, moments after a page load, not exist at all.
    // Both used to skip the restart in silence, and a skipped restart is invisible: the run just
    // joins whichever cycle happens to be open, part-spent, and the promises are sized for a
    // whole one.
    const pacing = (await dashboard.mutate())?.pacing
    // A calendar cycle is anchored to a day of the month, which no re-activation moves; the run
    // joins the period the merchant is really in.
    if (pacing?.ruleId && isTestCycleOf(pacing.daySecs ?? SECS_PER_DAY)) {
      await restartCycleFor(pacing.ruleId)
    }
    // `preset` is rebuilt every render from the contract, so it is right even on a first run the
    // page has never been handed one for — which is a run with no pacing and the wrong ticket.
    return preset.current.gateways.length > 0 ? { ...preset.current } : null
  }
  useEffect(() => {
    onPrepareRun(() => prepareRun.current())
    // Unregistered on the way out, so the page is never holding a gone card's promise to open a
    // cycle — the simulator tab can be left, and this card goes with it.
    return () => onPrepareRun(async () => null)
  })

  // The simulator takes its settings from the contract rather than waiting to be pointed at it.
  // Ticket size and the eligible gateways are both contract terms — a run driven by anything else
  // measures nothing about the commitments on this card, and the numbers under each promise only
  // mean what they say because the traffic matches the rate the document declares.
  //
  // Applied once per cycle rather than on every render, and never mid-run: the ticket is read
  // from a ref and the effect keys on the contract and cycle alone, so a run cannot have its
  // amount rewritten underneath it.
  //
  // `expectedDailyTraffic` is a rate — so much value per contract day — and it is the figure every
  // goal on this card is sized against. Divided by the payments a contract day actually carries,
  // it gives the ticket that makes the declaration true. A run sending anything else races
  // promises priced for a different amount of money: at the shipped sample's $200k a contract day
  // the ticket is $1,000, so the page's own $10-$100 range would deliver a small fraction of the
  // cycle the targets assume and every commitment would read as unreachable from day 0.
  const contractDaySecs = view.pacing?.daySecs ?? SECS_PER_DAY
  const declaredDaily = view.pacing?.expectedDailyTraffic ?? 0
  const paymentsPerContractDay = Math.max(1, SIMULATOR_PAYMENTS_PER_SEC * contractDaySecs)
  const ticket =
    declaredDaily > 0
      ? Math.max(
          0.01,
          Math.round((toMajorUnits(declaredDaily, currency) / paymentsPerContractDay) * 100) / 100,
        )
      : null
  const preset = useRef<ContractRunPreset>({ gateways: [], ticket: null })
  preset.current = { gateways: rows.map((r) => r.name), ticket }
  const onLoadRef = useRef(onLoad)
  onLoadRef.current = onLoad
  const appliedCycle = useRef<string | null>(null)
  const liveCycle =
    active && !cycleOver
      ? `${view.pacing?.ruleId ?? ''}|${view.pacing?.cycleStart ?? ''}`
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
  if (view.pacing?.contractConfigured && !view.pacing.featureEnabled) {
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
          <VolumeContractFeatureNotice
              merchantId={merchantId}
              onEnabled={() => void dashboard.mutate()}
            />
        </CardBody>
      </Card>
    )
  }

  // Nothing to say until a contract is live or this run has actually steered something.
  if (!active && steeredCount === 0) return null
  if (rows.length === 0) return null

  const total = tally.total

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
                Test cycle · {secondsLeft}s left of {cycleDays} days
              </Badge>
            ) : (
              <Badge variant="blue">
                {cycleDays}-day cycle
                {cycleEndMs ? ` · ${Math.ceil(secondsLeft / 86_400)} days left` : ''}
              </Badge>
            )}
          </div>
          <div className="ml-auto flex shrink-0 items-center gap-2">
            {isTest && view.pacing?.ruleId && (
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
              {/* The counts are held together, but the line as a whole wraps: the reason for a
                  zero is a clause, and holding that unbroken pushes it over the column beside it
                  rather than onto a second line of its own. */}
              <span className="ml-auto min-w-0 tabular-nums text-slate-500 dark:text-slate-400">
                {r.goal > 0 && (
                  <>
                    <span className="whitespace-nowrap">
                      {formatAchieved(r.achieved, r.goal, currency)}/{formatMoney(r.goal, currency)}
                      {r.status !== 'eliminated' && ` · ${pctOfGoal(r.achieved, r.goal)}%`}
                    </span>
                    {' · '}
                  </>
                )}
                <span className="whitespace-nowrap">
                  {r.auth} by approval · {r.steered} steered in
                </span>
                {/* Reads on from the "0 steered in" it follows, so it needs no label of its
                    own — the clause is the reason for that zero. */}
                {r.noSteerReason && (
                  <span className="text-amber-600 dark:text-amber-500">
                    {' · '}
                    {r.noSteerReason}
                  </span>
                )}
              </span>
            </div>
          ))}
        </div>
        {restartError && <p className="text-xs text-red-500">{restartError}</p>}
      </CardBody>
    </Card>
  )
}
