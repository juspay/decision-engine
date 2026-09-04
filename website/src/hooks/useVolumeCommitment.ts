import useSWR from 'swr'
import { fetcher } from '../lib/api'
import {
  CommitmentDashboardResponse,
  CommitmentImpactResponse,
  VolumeCommitmentView,
  VolumeContractSamplesResponse,
} from '../types/api'

/** Default poll for the pacing card and series; the forecast recomputes in the background. */
export const PACING_POLL_MS = 15_000
/** Default poll for the audit trail and the impact view. */
export const ACTIVITY_POLL_MS = 10_000

/** A volume-commitment endpoint for one merchant, or `null` (no request) without one. */
function vcPath(
  merchantId: string | undefined,
  suffix = '',
  params: Record<string, string | undefined> = {},
) {
  if (!merchantId) return null
  const query = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) if (value) query.set(key, value)
  const qs = query.toString()
  return `/merchant-account/${merchantId}/volume-commitment${suffix}${qs ? `?${qs}` : ''}`
}

/**
 * Pacing, series and audit in one request.
 *
 * These used to be three hooks against three endpoints, polled at two cadences — and because the
 * series hook keyed on its bucket size, two components on one page fetched the same data twice.
 * The server composes them now, so a page polls once and every component reads the same snapshot.
 */
export function useVolumeCommitmentDashboard(
  merchantId?: string,
  options: { runId?: string; perDay?: number; refreshInterval?: number } = {},
) {
  const path = vcPath(merchantId, '/dashboard', {
    run_id: options.runId,
    per_day: options.perDay && options.perDay > 1 ? String(options.perDay) : undefined,
  })
  const { data, error, isLoading, mutate } = useSWR<CommitmentDashboardResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: options.refreshInterval ?? PACING_POLL_MS,
    keepPreviousData: true,
  })
  return {
    pacing: data?.pacing,
    series: data?.series,
    runs: data?.audit.runs ?? [],
    events: data?.audit.events ?? [],
    error,
    isLoading,
    mutate,
  }
}

/**
 * What contract routing would do this cycle if it were switched on now — the same view as
 * `useVolumeCommitment`, from a plan the server computes and never stores. Fetched only when the
 * dashboard is about to ask the merchant to enable the feature, so it does not poll.
 */
export function useVolumeCommitmentProjection(merchantId?: string) {
  const { data, error, isLoading, mutate } = useSWR<VolumeCommitmentView>(
    vcPath(merchantId, '/projection'),
    fetcher,
    { revalidateOnFocus: false, shouldRetryOnError: false },
  )
  return { data, error, isLoading, mutate }
}



/**
 * The deployment's demo contract templates. Static per deployment, so it neither polls nor
 * retries: an empty list is the ordinary answer in production, not a failure.
 */
export function useVolumeContractSamples(merchantId?: string) {
  const { data, isLoading } = useSWR<VolumeContractSamplesResponse>(
    vcPath(merchantId, '/samples'),
    fetcher,
    { revalidateOnFocus: false, shouldRetryOnError: false },
  )
  return { samples: data?.samples ?? [], isLoading }
}

/** Previous cycle vs this one per PSP, split unaided / steered / ceded; polls. */
export function useVolumeCommitmentImpact(merchantId?: string, runId?: string) {
  const path = vcPath(merchantId, '/impact', { run_id: runId })
  const { data, error, isLoading, mutate } = useSWR<CommitmentImpactResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: ACTIVITY_POLL_MS,
    // A 404 just means no contract is live; keep the last good story on screen rather than flashing.
    shouldRetryOnError: false,
    keepPreviousData: true,
  })
  return { data, error, isLoading, mutate }
}
