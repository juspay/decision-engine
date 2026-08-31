import { useEffect, useRef, useState } from 'react'
import { Handshake } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { apiPost } from '../../lib/api'
import { useVolumeCommitment, useVolumeCommitmentSeries } from '../../hooks/useVolumeCommitment'
import { SECS_PER_DAY, compactAmount as compact, isTestCycle as isTestCycleOf } from './volumeCommitmentChartBits'

/** Extra payments over the seconds left, so latency cannot end the run before the cycle does. */
const RUN_HEADROOM = 3

/** Loads the simulator from the active contract, sized to the time left in the cycle. */
export function ContractSimulationPanel({
  merchantId,
  isSimulating,
  tps,
  onContractGone,
  onCycleEnded,
  onLoad,
}: {
  merchantId: string | null
  isSimulating: boolean
  /** Payments the run fires per second — the divisor that turns a daily total into a ticket size. */
  tps: number
  /** Called when no contract is available, so the page can drop a pace set by an earlier Load. */
  onContractGone: () => void
  /** Called when the cycle closes mid-run: volume sent past it lands in the next period. */
  onCycleEnded: () => void
  onLoad: (preset: {
    gateways: string[]
    amount: number
    totalPayments: number
    paceMs: number
  }) => void
}) {
  const { data, mutate } = useVolumeCommitment(merchantId ?? undefined)
  const series = useVolumeCommitmentSeries(merchantId ?? undefined)
  const seriesMutate = series.mutate
  const connectors = series.data?.connectors ?? []
  const hasContract = Boolean(data?.active) && connectors.length > 0

  // The page's callbacks change identity every render; the effects below key on facts, not on them.
  const callbacks = useRef({ onContractGone, onCycleEnded })
  callbacks.current = { onContractGone, onCycleEnded }

  // Re-render every second so the countdown ticks and a run can be cut off the moment the cycle
  // closes, without waiting for the next poll.
  const [, setNow] = useState(Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    if (!hasContract) callbacks.current.onContractGone()
  }, [hasContract])

  const cycleEndMs = connectors[0] ? Date.parse(connectors[0].cycleEnd) : 0
  const secondsLeft = cycleEndMs ? Math.max(0, Math.round((cycleEndMs - Date.now()) / 1000)) : 0
  const cycleOver = Boolean(cycleEndMs) && secondsLeft === 0

  // A run outliving its cycle delivers into the next period, against goals that have just reset.
  useEffect(() => {
    if (cycleOver && isSimulating) callbacks.current.onCycleEnded()
  }, [cycleOver, isSimulating])

  // Refetch on cycle close so the countdown does not sit at "Cycle over" until the next poll.
  useEffect(() => {
    if (!cycleOver) return
    void seriesMutate()
    void mutate()
  }, [cycleOver, seriesMutate, mutate])

  if (!hasContract || !data) return null

  const daySecs = data.daySecs ?? SECS_PER_DAY
  const expectedDaily = data.expectedDailyTraffic ?? 0
  const isTestCycle = isTestCycleOf(daySecs)
  const cycleDays = Math.max(...connectors.map((c) => c.daysTotal), 1)

  // Volume rate is a contract term: `expectedDaily` per contract day, however many payments carry
  // it. TPS only decides how finely that is chopped — more payments of proportionally less each.
  const paymentsPerDay = Math.max(1, Math.round(tps * daySecs))
  const perPayment = expectedDaily > 0 ? Math.max(1, Math.round(expectedDaily / paymentsPerDay)) : 1000
  const paceMs = Math.max(1, Math.round((daySecs * 1000) / paymentsPerDay))
  // Over-provisioned so the cycle, never the count, ends the run; `onCycleEnded` discards the rest.
  const totalPayments = Math.max(1, Math.round(tps * secondsLeft * RUN_HEADROOM))

  async function deactivate() {
    if (!data?.ruleId || !merchantId) return
    await apiPost('/routing/deactivate', {
      created_by: merchantId,
      routing_algorithm_id: data.ruleId,
    })
    void mutate()
  }

  return (
    <Card>
      <CardBody className="space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Handshake size={15} className="text-brand-500" />
            <span className="text-sm font-medium text-slate-800 dark:text-white">
              Active volume contract
            </span>
            {cycleOver ? (
              <Badge variant="red">Cycle over</Badge>
            ) : data.psps.length === 0 && data.eliminated.length === 0 ? (
              // The contract is live but no plan covers this cycle yet — right after activation,
              // or in the seconds between one cycle closing and the next forecast landing.
              <Badge variant="gray">Forecast pending</Badge>
            ) : isTestCycle ? (
              <Badge variant="orange">
                Test cycle · {secondsLeft}s left of {cycleDays} min
              </Badge>
            ) : (
              <Badge variant="blue">{cycleDays}-day cycle</Badge>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="ghost" onClick={deactivate} disabled={!data.ruleId}>
              Deactivate contract
            </Button>
            <Button size="sm" variant="secondary" onClick={() => onLoad({
              gateways: connectors.map((c) => c.connector),
              amount: perPayment,
              totalPayments,
              paceMs,
            })} disabled={isSimulating || cycleOver}>
              Load contract into simulator
            </Button>
          </div>
        </div>

        <div className="flex flex-wrap gap-x-5 gap-y-1 text-xs text-slate-600 dark:text-slate-300">
          {connectors.map((c) => {
            const pacing = data.psps.find((p) => p.connector === c.connector)
            // A dropped PSP leaves `psps` for `eliminated`, but its delivered volume is still real.
            const dropped = data.eliminated.find((e) => e.connector === c.connector)
            const achieved = pacing?.achieved ?? dropped?.achieved ?? 0
            const eliminated = c.eliminated
            return (
              <span key={c.connector} className="tabular-nums">
                <strong>{c.connector}</strong> {compact(achieved)}/{compact(c.goal)}
                {eliminated ? (
                  <span className="ml-1 text-red-500">· eliminated</span>
                ) : pacing?.steering ? (
                  <span className="ml-1 text-amber-600 dark:text-amber-500">· steering</span>
                ) : null}
              </span>
            )
          })}
        </div>

        {cycleOver ? (
          <p className="text-xs text-slate-500 dark:text-slate-400">
            This cycle has closed — its goals are settled and a new one has begun, with delivery
            back at zero. Any run still going was stopped, because volume sent now counts toward the
            next period. Reload the contract to drive the fresh cycle, or deactivate it to stop.
          </p>
        ) : (
          <p className="text-xs text-slate-500 dark:text-slate-400">
            Every payment <strong className="tabular-nums">{perPayment.toLocaleString()}</strong> at {tps}/sec
            — {compact(expectedDaily)} per day{isTestCycle ? ' (a minute on this test cycle)' : ''}, the contract&apos;s rate — until
            the cycle closes. More TPS means smaller payments, not more volume; pausing is a real
            traffic drop.
          </p>
        )}
      </CardBody>
    </Card>
  )
}
