import useSWR from 'swr'
import { apiDelete, apiPost, apiUploadWithProgress, fetcher, type UploadProgress } from '../lib/api'

export interface CoverageSummary {
  total_clusters: number
  good_clusters: number
  thin_clusters: number
  non_linear_clusters: number
  total_txns: number
  good_txns: number
  thin_txns: number
  non_linear_txns: number
  good_txn_pct: number
  total_gross: number
  good_gross: number
  thin_gross: number
  non_linear_gross: number
  /** Share of settled volume (money) with a trustworthy cost model — the headline metric. */
  good_gross_pct: number
  bps_rmse_p50: number
  bps_rmse_p90: number
  /** Snapshot date these numbers are from (YYYY-MM-DD). */
  report_date: string
}

export interface ConnectorSource {
  connector: string
  account: string
}

export interface SetCredentialsResponse {
  merchant_id: string
  connector: string
  account: string
  status: string
}

/** 202 response from a manual upload — the created job's id, polled for progress. */
export interface UploadAccepted {
  id: number
  status: string
}

/** One ingestion (history + live progress). */
export interface IngestionDto {
  id: number
  connector: string
  account: string
  source: 'manual' | 'webhook' | string
  status: 'processing' | 'completed' | 'failed' | string
  staged_rows: number
  report_date: string | null
  period_start: string | null
  period_end: string | null
  currency_count: number
  currencies: string[]
  country_count: number
  countries: string[]
  total_gross: number
  total_clusters: number
  good_clusters: number
  last_error: string | null
  created_at: string
}

/** A detected fee-regime change on a cluster (its price moved between the last two fits). */
export interface PriceChange {
  connector: string
  account: string
  card_network: string
  variant: string
  funding: string
  issuer_country: string
  currency: string
  ic_category: string
  old_pct_bps: number
  new_pct_bps: number
  old_fixed: number
  new_fixed: number
  changed_on: string
}

/** Fee-regime changes detected by diffing each cluster's two most recent fits. */
export function usePriceChanges(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/cost-price-changes` : null
  const { data, error, isLoading } = useSWR<PriceChange[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { changes: data ?? [], error, isLoading }
}

/** Latest cost-model coverage for the merchant (backs the health card). */
export function useCostCoverage(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/cost-coverage` : null
  const { data, error, isLoading, mutate } = useSWR<CoverageSummary>(path, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 60_000,
  })
  return { coverage: data, error, isLoading, mutate }
}

/** The (connector, account) pairs a merchant has configured (no secrets). */
export function useConnectorSources(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/connectors` : null
  const { data, error, isLoading, mutate } = useSWR<ConnectorSource[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { sources: data ?? [], error, isLoading, mutate }
}

/**
 * A merchant's ingestion history (and in-flight jobs). Polls every 2s while any job is still
 * processing, so an in-progress upload's `staged_rows` climbs live; idles otherwise.
 */
export function useIngestionHistory(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/cost-ingestions` : null
  const { data, error, isLoading, mutate } = useSWR<IngestionDto[]>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: (latest) =>
      latest?.some((j) => j.status === 'processing') ? 2000 : 0,
  })
  return { ingestions: data ?? [], error, isLoading, mutate }
}

/** Save (encrypt at rest) a connector's settlement-ingestion credentials. */
export async function setConnectorCredentials(
  merchantId: string,
  connector: string,
  body: { account: string; webhook_secret: string; download_auth: string },
) {
  return apiPost<SetCredentialsResponse>(
    `/merchant-account/${merchantId}/connectors/${connector}/credentials`,
    body,
  )
}

/**
 * Upload a settlement report file. Returns as soon as the bytes are received (202 + job id); the
 * server processes it in the background. Poll {@link useIngestionHistory} for progress and outcome.
 */
export async function uploadReport(
  merchantId: string,
  connector: string,
  account: string,
  file: Blob,
  onProgress?: (p: UploadProgress) => void,
) {
  return apiUploadWithProgress<UploadAccepted>(
    `/merchant-account/${merchantId}/connectors/${connector}/report?account=${encodeURIComponent(account)}`,
    file,
    onProgress,
  )
}

/**
 * Delete (undo) an ingestion: removes its fitted snapshot + staged rows and its history row.
 * Coverage/serving revert to the previous snapshot automatically.
 */
export async function deleteIngestion(merchantId: string, id: number) {
  return apiDelete(`/merchant-account/${merchantId}/cost-ingestions/${id}`)
}
