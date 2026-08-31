import { useCallback, useMemo } from 'react'
import { Handshake } from 'lucide-react'
import { Card, CardBody } from '../ui/Card'
import { Badge } from '../ui/Badge'
import {
  useVolumeCommitment,
  useVolumeCommitmentAudit,
  useVolumeCommitmentSeries,
} from '../../hooks/useVolumeCommitment'
import { CommitmentPacingChart, PacingStatus } from './CommitmentPacingChart'
import {
  SolidSwatch,
  bucketsPerDay,
  firstEliminationByConnector,
  formatAchieved,
  formatMoney,
  pctOfGoal,
} from './volumeCommitmentChartBits'

/** The slice of a simulator row this card reads. */
export type SteerableResult = {
  decidedGateway: string
  /** Whether the volume-commitment engine moved this payment, from `volume_steer_info.outcome`. */
  steerOutcome?: 'STEERED' | 'SR_PREVAILED' | null
  /** The PSP approval-rate routing had picked, when the payment was steered elsewhere. */
  steerSrHead?: string | null
}

/** Tighter than the default polls while someone is watching a run; a test cycle is only minutes. */
const RUN_POLL_MS = 5_000

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
      return <Badge variant="orange">Steering · {(row.steerRate * 100).toFixed(0)}% of eligible</Badge>
    case 'on_pace':
      return <Badge variant="green">On pace</Badge>
    case 'missed':
      return <Badge variant="gray">Missed</Badge>
    default:
      return <Badge variant="gray">Pending forecast</Badge>
  }
}

/** Live pacing chart for the run plus one line per PSP: standing, and payments routed vs steered. */
export function VolumeCommitmentRunChart({
  merchantId,
  results,
  colorFor,
}: {
  merchantId: string | null
  results: SteerableResult[]
  colorFor: (gateway: string) => string
}) {
  const pacing = useVolumeCommitment(merchantId ?? undefined, RUN_POLL_MS)
  const daySecs = pacing.data?.daySecs
  const series = useVolumeCommitmentSeries(merchantId ?? undefined, undefined, {
    perDay: bucketsPerDay(daySecs),
    refreshInterval: RUN_POLL_MS,
  })
  const audit = useVolumeCommitmentAudit(merchantId ?? undefined)
  const active = Boolean(pacing.data?.active)
  const connectors = useMemo(() => series.data?.connectors ?? [], [series.data])
  const currency = series.data?.currency

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
          </div>
          <span className="text-xs tabular-nums text-slate-500 dark:text-slate-400">
            {steeredCount.toLocaleString()} of {total.toLocaleString()} payments steered
            {total > 0 ? ` · ${((steeredCount / total) * 100).toFixed(1)}%` : ''}
            {currentRunId && <span className="ml-2 font-mono text-[10px] opacity-60">{currentRunId}</span>}
          </span>
        </div>

        {connectors.length > 0 ? (
          <CommitmentPacingChart
            connectors={connectors}
            currency={currency}
            daySecs={series.data?.daySecs ?? daySecs}
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
              </span>
            </div>
          ))}
        </div>
      </CardBody>
    </Card>
  )
}
