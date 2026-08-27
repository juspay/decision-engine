import useSWR from 'swr'
import { fetcher } from '../lib/api'
import {
  CommitmentAuditResponse,
  CommitmentImpactResponse,
  CommitmentSeriesResponse,
  VolumeCommitmentView,
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

/** Latest pacing decision from the controller. Polls, since it recomputes in the background. */
export function useVolumeCommitment(merchantId?: string, refreshInterval = PACING_POLL_MS) {
  const { data, error, isLoading, mutate } = useSWR<VolumeCommitmentView>(vcPath(merchantId), fetcher, {
    revalidateOnFocus: false,
    refreshInterval,
  })

  return {
    data,
    error,
    isLoading,
    /** True while a contract document is live, even before its first forecast. */
    isActive: Boolean(data?.active),
    mutate,
  }
}

/** Series for the pacing chart; `perDay` = buckets per contract day, `refreshInterval` tightens the poll. */
export function useVolumeCommitmentSeries(
  merchantId?: string,
  runId?: string,
  options: { perDay?: number; refreshInterval?: number } = {},
) {
  const path = vcPath(merchantId, '/series', {
    run_id: runId,
    per_day: options.perDay && options.perDay > 1 ? String(options.perDay) : undefined,
  })
  const { data, error, isLoading, mutate } = useSWR<CommitmentSeriesResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: options.refreshInterval ?? PACING_POLL_MS,
    keepPreviousData: true,
  })
  return { data, error, isLoading, mutate }
}

/** Audit events (newest first) and the runs they span; `runId` narrows to one run. */
export function useVolumeCommitmentAudit(merchantId?: string, runId?: string) {
  const path = vcPath(merchantId, '/audit', { run_id: runId })
  const { data, error, isLoading } = useSWR<CommitmentAuditResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: ACTIVITY_POLL_MS,
  })
  return { events: data?.events ?? [], runs: data?.runs ?? [], error, isLoading }
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
