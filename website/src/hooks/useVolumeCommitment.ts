import useSWR from 'swr'
import { fetcher } from '../lib/api'
import { CommitmentAuditResponse, CommitmentSeriesResponse, VolumeCommitmentView } from '../types/api'

/** Latest pacing decision from the controller. Polls, since it recomputes in the background. */
export function useVolumeCommitment(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/volume-commitment` : null
  const { data, error, isLoading, mutate } = useSWR<VolumeCommitmentView>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: 15_000,
  })

  return {
    data,
    error,
    isLoading,
    /** False before the controller's first tick. */
    isActive: Boolean(data?.active),
    mutate,
  }
}

/** Per-day delivered volume per PSP plus the promise lines, for the pacing chart. */
export function useVolumeCommitmentSeries(merchantId?: string, runId?: string) {
  const base = merchantId ? `/merchant-account/${merchantId}/volume-commitment/series` : null
  const path = base ? (runId ? `${base}?run_id=${encodeURIComponent(runId)}` : base) : null
  const { data, error, isLoading, mutate } = useSWR<CommitmentSeriesResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: 15_000,
  })
  return { data, error, isLoading, mutate }
}

/**
 * The audit trail: every forecast, steer chunk and elimination, newest first — plus the list of
 * contract executions it spans. Pass `runId` to narrow it to one execution.
 */
export function useVolumeCommitmentAudit(merchantId?: string, runId?: string) {
  const base = merchantId ? `/merchant-account/${merchantId}/volume-commitment/audit` : null
  const path = base ? (runId ? `${base}?run_id=${encodeURIComponent(runId)}` : base) : null
  const { data, error, isLoading } = useSWR<CommitmentAuditResponse>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: 10_000,
  })
  return { events: data?.events ?? [], runs: data?.runs ?? [], error, isLoading }
}
