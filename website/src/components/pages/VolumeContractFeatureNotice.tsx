import { useState } from 'react'
import { AlertTriangle, Handshake } from 'lucide-react'
import { Button } from '../ui/Button'
import { useMerchantFeatures } from '../../hooks/useMerchantFeatures'
import { useVolumeCommitmentProjection } from '../../hooks/useVolumeCommitment'
import { useCanEditRouting } from '../../store/authStore'
import { VolumeCommitmentView } from '../../types/api'
import { formatMoney, isTestCycle, SECS_PER_DAY } from './volumeCommitmentChartBits'

/** Where a commitment stands, read off which list of the plan it landed in. */
type Verdict = 'already_met' | 'on_track' | 'needs_steering' | 'unreachable'

type Line = {
  connector: string
  verdict: Verdict
  goal: number
  achieved: number
  neededDaily: number
}

const VERDICT_LABEL: Record<Verdict, string> = {
  already_met: 'Already met',
  on_track: 'On track',
  needs_steering: 'Will steer',
  unreachable: 'Unreachable',
}

const VERDICT_CLASS: Record<Verdict, string> = {
  already_met: 'text-emerald-600 dark:text-emerald-400',
  on_track: 'text-brand-600 dark:text-brand-400',
  needs_steering: 'text-amber-600 dark:text-amber-500',
  unreachable: 'text-red-600 dark:text-red-400',
}

/**
 * The plan's own verdicts, not a second opinion: a commitment the engine dropped is `unreachable`,
 * and one it kept is read from whether normal routing already covers its floor. Recomputing any of
 * that here would eventually disagree with the engine at exactly the cases this exists to explain.
 */
function linesOf(view: VolumeCommitmentView): Line[] {
  const kept: Line[] = view.psps.map((p) => ({
    connector: p.connector,
    verdict: p.gap <= 0 ? 'already_met' : p.steering ? 'needs_steering' : 'on_track',
    goal: p.goal,
    achieved: p.achieved,
    neededDaily: p.floorPerDay,
  }))
  const dropped: Line[] = view.eliminated.map((e) => ({
    connector: e.connector,
    verdict: 'unreachable',
    goal: e.achieved + e.gap,
    achieved: e.achieved,
    neededDaily: 0,
  }))
  return [...kept, ...dropped]
}

/** "day 31 of 31" — which contract day the cycle is on right now. */
function cyclePosition(view: VolumeCommitmentView): string | null {
  if (!view.cycleStart || !view.daysTotal) return null
  const startedAt = Date.parse(view.cycleStart)
  if (Number.isNaN(startedAt)) return null
  const daySecs = view.daySecs ?? SECS_PER_DAY
  const elapsed = (Date.now() - startedAt) / (daySecs * 1000)
  const unit = isTestCycle(daySecs) ? 'minute' : 'day'
  // A cycle is on its first day from the moment it opens, and on its last until it closes: the
  // clamp keeps a not-yet-open cycle off "day 0" and a stale one off a day past its own length.
  const day = Math.min(Math.max(Math.floor(elapsed) + 1, 1), view.daysTotal)
  return `${unit} ${day} of ${view.daysTotal}`
}

function formatCycleEnd(cycleEnd?: string | null): string | null {
  if (!cycleEnd) return null
  const at = new Date(cycleEnd)
  if (Number.isNaN(at.getTime())) return null
  return at.toLocaleDateString(undefined, { day: 'numeric', month: 'short' })
}


/** Text colours for the surface the verdict list is sitting on. */
type Tone = { rule: string; label: string; body: string }

const AMBER: Tone = {
  rule: 'border-amber-500/20',
  label: 'text-amber-700/70 dark:text-amber-400/70',
  body: 'text-amber-700/90 dark:text-amber-400/90',
}

const NEUTRAL: Tone = {
  rule: 'border-slate-200 dark:border-[#222226]',
  label: 'text-slate-500 dark:text-[#78849a]',
  body: 'text-slate-600 dark:text-[#9ca7ba]',
}

/**
 * Where each commitment stands, one line each. Shared by the two surfaces that report a projection
 * so neither can word the same plan differently, and so the measurement-failure case is handled
 * once: zeroed measurements would otherwise render as "every commitment is unreachable" — a
 * confident, specific and wrong claim, made exactly when a merchant is deciding whether to trust
 * the feature.
 */
function ProjectionLines({ view, lead, tone }: { view: VolumeCommitmentView; lead: string; tone: Tone }) {
  if (!view.measurementAvailable) {
    return (
      <p className={`mt-2.5 border-t pt-2 text-xs ${tone.rule} ${tone.body}`}>
        Volume history can&apos;t be read right now, so there is no reliable picture of where the
        commitments stand. Pacing resumes at the next forecast that can read it.
      </p>
    )
  }

  const lines = linesOf(view)
  if (lines.length === 0) return null
  const nextCycle = formatCycleEnd(view.cycleEnd)
  const allUnreachable = lines.every((l) => l.verdict === 'unreachable')

  return (
    <div className={`mt-2.5 flex flex-col gap-1 border-t pt-2 ${tone.rule}`}>
      <p className={`text-[11px] font-medium uppercase tracking-wide ${tone.label}`}>{lead}</p>
      {lines.map((line) => (
        <p key={line.connector} className={`text-xs ${tone.body}`}>
          <strong className="font-semibold">{line.connector}</strong>{' '}
          <span className={`font-medium ${VERDICT_CLASS[line.verdict]}`}>
            {VERDICT_LABEL[line.verdict]}
          </span>{' '}
          <span className="tabular-nums">
            {formatMoney(line.achieved, view.currency)} of {formatMoney(line.goal, view.currency)}
          </span>
          {line.verdict === 'needs_steering' && (
            <span className="tabular-nums">
              {' '}
              · needs {formatMoney(line.neededDaily, view.currency)}/day
            </span>
          )}
        </p>
      ))}
      {allUnreachable && (
        <p className={`mt-0.5 text-xs ${tone.body}`}>
          No commitment can be met before the cycle closes, so nothing is steered and your approval
          rate is untouched.
          {nextCycle ? ` Pacing starts on ${nextCycle}, with the full cycle available.` : ''}
        </p>
      )}
    </div>
  )
}

/**
 * Shown when a contract document is activated but `volume-contracts` is off for the merchant.
 * Activation and the feature flag are deliberately independent — the flag is the kill switch that
 * stops steering without tearing the contract down — so this state is legal, silent, and easy to
 * arrive at by accident.
 *
 * Rather than only saying the feature is off, it runs the projection first and reports what
 * enabling would actually do to this cycle. Enabling late is often still the right call; enabling
 * on the last day of a cycle usually changes nothing until the next one, and a merchant should
 * learn that here rather than from a chart that never moves.
 *
 * Renders nothing unless there is a contract and the feature is off, so callers can mount it
 * without repeating that check.
 */
export function VolumeContractFeatureNotice({
  merchantId,
  onEnabled,
  className = '',
}: {
  merchantId: string | null
  /** Called after the flag is switched on, so the caller can refetch what the flag gates. */
  onEnabled?: () => void
  className?: string
}) {
  const canEditRouting = useCanEditRouting()
  const { setFeatureEnabled } = useMerchantFeatures(merchantId ?? undefined)
  const { data, mutate } = useVolumeCommitmentProjection(merchantId ?? undefined)
  const [enabling, setEnabling] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function enable() {
    setEnabling(true)
    setError(null)
    try {
      await setFeatureEnabled('volume-contracts', true)
      // This component's own gate reads `featureEnabled` from the projection, so it has to be
      // refetched here or the notice would linger after the flag it describes has flipped.
      await mutate()
      onEnabled?.()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to enable volume-contract routing')
    } finally {
      setEnabling(false)
    }
  }

  if (!data || !data.contractConfigured || data.featureEnabled) return null

  const position = cyclePosition(data)

  return (
    <div className={`rounded-lg border border-amber-500/25 bg-amber-500/8 px-3 py-2.5 ${className}`}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex items-start gap-2">
          <AlertTriangle size={15} className="mt-0.5 shrink-0 text-amber-600 dark:text-amber-500" />
          <div className="max-w-[70ch]">
            <p className="text-sm font-medium text-amber-700 dark:text-amber-400">
              Contract active, but volume-contract routing is off
            </p>
            <p className="mt-0.5 text-xs text-amber-700/80 dark:text-amber-400/80">
              The document is live, but the merchant feature flag is disabled — no payment is paced
              or steered against it, and no forecast runs.
              {position ? ` You are on ${position} of this cycle.` : ''}
            </p>
          </div>
        </div>
        <Button
          size="sm"
          variant="secondary"
          onClick={enable}
          disabled={!merchantId || !canEditRouting || enabling}
        >
          {enabling ? 'Enabling' : 'Enable'}
        </Button>
      </div>

      <ProjectionLines view={data} lead="If you enable now, this cycle" tone={AMBER} />

      {error && <p className="mt-2 text-xs text-red-500">{error}</p>}
    </div>
  )
}

/**
 * What contract routing will do for the cycle a document was just activated into.
 *
 * Activation lands mid-cycle more often than not, and until the next scheduled forecast the live
 * pacing view has no plan to show — so a merchant who activates sees nothing at all, which is the
 * same blank screen whether the contract is winnable or already lost. This reports the verdicts
 * straight away from a plan computed on demand.
 *
 * Only for merchants whose feature is already on; where it is off, `VolumeContractFeatureNotice`
 * says so and carries the same lines.
 */
export function VolumeContractActivationSummary({
  merchantId,
  className = '',
}: {
  merchantId: string | null
  className?: string
}) {
  const { data } = useVolumeCommitmentProjection(merchantId ?? undefined)

  if (!data || !data.contractConfigured || !data.featureEnabled) return null

  const lines = linesOf(data)
  const position = cyclePosition(data)
  const allUnreachable = lines.length > 0 && lines.every((l) => l.verdict === 'unreachable')

  return (
    <div className={`rounded-lg border border-slate-200 bg-slate-50 px-3 py-2.5 dark:border-[#222226] dark:bg-[#0d0d13] ${className}`}>
      <div className="flex items-start gap-2">
        <Handshake size={15} className="mt-0.5 shrink-0 text-brand-500" />
        <div className="max-w-[70ch]">
          <p className="text-sm font-medium text-slate-800 dark:text-white">
            {allUnreachable
              ? 'Contract active — but nothing can be met this cycle'
              : 'Contract active — routing is pacing against it'}
          </p>
          <p className="mt-0.5 text-xs text-slate-600 dark:text-[#9ca7ba]">
            {position ? `You are on ${position} of this cycle. ` : ''}
            Verdicts update at each forecast.
          </p>
        </div>
      </div>
      <ProjectionLines view={data} lead="This cycle" tone={NEUTRAL} />
    </div>
  )
}
