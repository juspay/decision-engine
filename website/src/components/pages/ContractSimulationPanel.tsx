import { useEffect, useState } from 'react'
import { Handshake } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { apiPost } from '../../lib/api'
import { useVolumeCommitment, useVolumeCommitmentSeries } from '../../hooks/useVolumeCommitment'

function compact(value: number) {
  if (Math.abs(value) >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (Math.abs(value) >= 1_000) return `${(value / 1_000).toFixed(0)}k`
  return value.toFixed(0)
}

/**
 * Drives the batch simulator from the merchant's active volume contract.
 *
 * The contract already says who the connectors are, how much volume a contract day carries, and
 * when the cycle closes — everything the simulator would otherwise ask you to type. "Load" copies
 * those across, sized to the time *left* in the cycle rather than a whole one, because the seconds
 * spent activating and switching pages are seconds of that cycle already gone.
 */
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

  // Re-render every second so the countdown ticks and a run can be cut off the moment the cycle
  // closes, without waiting for the next poll.
  const [, setNow] = useState(Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    if (!hasContract) onContractGone()
    // Keyed on the fact, not the callback identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hasContract])

  const cycleEndMs = connectors[0] ? Date.parse(connectors[0].cycleEnd) : 0
  const secondsLeft = cycleEndMs ? Math.max(0, Math.round((cycleEndMs - Date.now()) / 1000)) : 0
  const cycleOver = Boolean(cycleEndMs) && secondsLeft === 0

  // A run outliving its cycle delivers into the next period, against goals that have just reset.
  useEffect(() => {
    if (cycleOver && isSimulating) onCycleEnded()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cycleOver, isSimulating])

  // The moment the cycle closes, pull the next one rather than waiting out the poll interval —
  // otherwise the countdown sits at "Cycle over" until something else happens to refetch, which
  // looked like the panel had frozen.
  useEffect(() => {
    if (!cycleOver) return
    void seriesMutate()
    void mutate()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cycleOver])

  if (!hasContract || !data) return null

  const daySecs = data.daySecs ?? 86_400
  const expectedDaily = data.expectedDailyTraffic ?? 0
  const isTestCycle = daySecs < 86_400
  const cycleDays = Math.max(...connectors.map((c) => c.daysTotal), 1)

  // Volume rate is a contract term: `expectedDaily` per contract day, however many payments carry
  // it. TPS only decides how finely that is chopped — more payments of proportionally less each.
  const paymentsPerDay = Math.max(1, Math.round(tps * daySecs))
  const perPayment = expectedDaily > 0 ? Math.max(1, Math.round(expectedDaily / paymentsPerDay)) : 1000
  const paceMs = Math.max(1, Math.round((daySecs * 1000) / paymentsPerDay))
  // The cycle should be what ends the run, never the payment count. Each payment costs a round
  // trip on top of its pace slot, so a count sized exactly to the seconds remaining runs out
  // before the cycle closes and leaves the contract half-fed. The headroom absorbs that latency;
  // whatever is unused is discarded when `onCycleEnded` stops the run at the boundary.
  const RUN_HEADROOM = 3
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
            const eliminated = c.eliminated
            return (
              <span key={c.connector} className="tabular-nums">
                <strong>{c.connector}</strong> {compact(pacing?.achieved ?? 0)}/{compact(c.goal)}
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
            Fixes every payment at{' '}
            <strong className="tabular-nums">{perPayment.toLocaleString()}</strong> and fires{' '}
            {tps}/sec — {compact(expectedDaily)} per {isTestCycle ? 'minute' : 'day'}, the
            contract&apos;s stated rate. It runs for the{' '}
            <strong className="tabular-nums">{secondsLeft}s</strong> left in this cycle and stops
            when the cycle closes — not when a payment count runs out. Raise TPS for a livelier run
            — the volume rate is fixed by the contract, so more payments simply means smaller ones.
            Pausing is a real traffic drop the next forecast will see, and you can switch to
            Analytics → Volume commitments while it keeps running.
          </p>
        )}
      </CardBody>
    </Card>
  )
}
