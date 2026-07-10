import useSWR from 'swr'
import { apiDelete, apiPost, apiPut, apiUploadWithProgress, fetcher, type UploadProgress } from '../lib/api'

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
  /** Masked preview of the stored webhook secret (e.g. "••••a3f9"), never the full value. */
  webhook_secret_hint?: string
  /** Masked preview of the report-download auth ("reportuser:••••" or "••••a3f9"). */
  download_auth_hint?: string
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

/**
 * One connector's fee picture: the model-derived blended fee (rolled up from the fitted snapshot),
 * any manual override, and the effective fee actually used at decide time.
 */
export interface ConnectorFee {
  connector: string
  account: string | null
  has_credentials: boolean
  model_pct_bps: number | null
  model_fixed: number | null
  good_gross: number | null
  override_pct_bps: number | null
  override_fixed: number | null
  override_updated_at: string | null
  effective_pct_bps: number | null
  effective_fixed: number | null
  source: 'override' | 'model' | 'none' | string
}

/** Per-connector blended fees (model + override) for the merchant. */
export function useConnectorFees(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/connector-fees` : null
  const { data, error, isLoading, mutate } = useSWR<ConnectorFee[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { fees: data ?? [], error, isLoading, mutate }
}

/** Set (upsert) a connector's manual blended-fee override. All new EV calculations then use it. */
export async function setFeeOverride(
  merchantId: string,
  connector: string,
  body: { pct_bps: number; fixed: number },
) {
  return apiPut(`/merchant-account/${merchantId}/connectors/${connector}/fee-override`, body)
}

/** Clear a connector's override, reverting to the learned model. */
export async function deleteFeeOverride(merchantId: string, connector: string) {
  return apiDelete(`/merchant-account/${merchantId}/connectors/${connector}/fee-override`)
}

/**
 * One fitted cluster (a specific segment like "Visa debit · US · USD"): its learned fee, GMV/txn
 * volume, and any per-cluster override. `key` identifies the cluster for the override endpoints.
 */
export interface ClusterFee {
  key: string
  connector: string
  card_network: string
  /** Fitted card program/tier, e.g. `visastandarddebit` (surfaced as the Program column). */
  variant: string
  funding: string
  issuer_country: string
  currency: string
  ic_category: string
  n: number
  gross_sum: number
  model_pct_bps: number | null
  model_fixed: number | null
  override_pct_bps: number | null
  override_fixed: number | null
  override_updated_at: string | null
  effective_pct_bps: number
  effective_fixed: number
  source: 'override' | 'model' | string
}

export interface ClustersScope {
  /** Scope to one ingested snapshot's segments (all three required together). */
  connector?: string
  account?: string
  reportDate?: string
}

/**
 * The merchant's top clusters by GMV. Merchant-wide by default (the override targets, plus any
 * overridden clusters); pass a snapshot scope to get one ingestion's fitted segments instead.
 */
export function useCostClusters(
  merchantId?: string,
  opts: { limit?: number } & ClustersScope = {},
) {
  const { limit = 10, connector, account, reportDate } = opts
  let path: string | null = null
  if (merchantId) {
    const params = new URLSearchParams({ limit: String(limit) })
    // Each scope dimension is independent: a connector (+optional account) narrows to that
    // connector's latest-snapshot segments; adding report_date pins one exact ingestion. The
    // backend AND-combines whichever are present, so send each one we have.
    if (connector) params.set('connector', connector)
    if (account) params.set('account', account)
    if (reportDate) params.set('report_date', reportDate)
    path = `/merchant-account/${merchantId}/cost-clusters?${params.toString()}`
  }
  const { data, error, isLoading, mutate } = useSWR<ClusterFee[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { clusters: data ?? [], error, isLoading, mutate }
}

/** Set (upsert) a per-cluster fee override. Wins over the connector override and the learned model. */
export async function setClusterOverride(
  merchantId: string,
  clusterKey: string,
  body: { pct_bps: number; fixed: number },
) {
  return apiPut(
    `/merchant-account/${merchantId}/cost-clusters/${encodeURIComponent(clusterKey)}/fee-override`,
    body,
  )
}

/** Clear a per-cluster override, reverting that segment to the learned model. */
export async function deleteClusterOverride(merchantId: string, clusterKey: string) {
  return apiDelete(
    `/merchant-account/${merchantId}/cost-clusters/${encodeURIComponent(clusterKey)}/fee-override`,
  )
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
 * pending or processing, so a queued webhook job's start and an in-progress upload's climbing
 * `staged_rows` show up live; idles otherwise.
 */
export function useIngestionHistory(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/cost-ingestions` : null
  const { data, error, isLoading, mutate } = useSWR<IngestionDto[]>(path, fetcher, {
    revalidateOnFocus: false,
    refreshInterval: (latest) =>
      latest?.some((j) => j.status === 'processing' || j.status === 'pending')
        ? 2000
        : 0,
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

/** Delete a connector's stored credentials and drop it from the configured sources list. */
export async function deleteConnectorCredentials(
  merchantId: string,
  connector: string,
  account: string,
) {
  return apiDelete(
    `/merchant-account/${merchantId}/connectors/${connector}/credentials/${encodeURIComponent(account)}`,
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

// ── Invoice ingestion (the second data source: recovers the fees the PAR structurally can't) ─────

/** How an identified invoice line participates in the cost. */
export type InvoiceLineKind =
  | 'flat_per_txn'
  | 'periodic'
  | 'credit'
  | 'already_modeled'
  | 'volume'
  | string

/** One identified fee type from the uploaded invoice. */
export interface InvoiceLineDto {
  description: string
  kind: InvoiceLineKind
  /** True for a missing PAR fee we now apply; false for a line we ignored (already modeled / volume). */
  added: boolean
  /** Total on the invoice for this fee type (credits are negative). */
  amount: number
  /** Amortized contribution per transaction (0 for ignored lines). */
  per_txn: number
}

/** Result of an invoice upload — the computed add-on plus the identified detail. */
export interface InvoiceUploadResponse {
  merchant_id: string
  connector: string
  account: string
  pct_addon_bps: number
  fixed_addon: number
  /** Total additional fee applied per transaction for this connector account (the headline). */
  total_addon_per_txn: number
  subtotal_ex_tax: number | null
  card_volume: number | null
  txn_count: number | null
  currency: string
  lines: number
  breakdown: InvoiceLineDto[]
}

/** A currently-active invoice add-on for a connector (layered onto every learned cost). */
export interface InvoiceAddon {
  connector: string
  pct_addon_bps: number
  fixed_addon: number
  invoice_ref: string
  subtotal_ex_tax: number | null
  card_volume: number | null
  txn_count: number | null
  currency: string
  period_start: string | null
  period_end: string | null
  updated_at: string
}

/** The invoice add-ons currently in effect for the merchant. */
export function useInvoiceAddons(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/invoice-addons` : null
  const { data, error, isLoading, mutate } = useSWR<InvoiceAddon[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { addons: data ?? [], error, isLoading, mutate }
}

/**
 * Upload a connector invoice. Synchronous: returns the computed add-on and the identified fee
 * breakdown once parsed (invoices are small). The add-on then applies to every routing decision.
 */
export async function uploadInvoice(
  merchantId: string,
  connector: string,
  account: string,
  file: Blob,
  invoiceRef?: string,
  onProgress?: (p: UploadProgress) => void,
) {
  const params = new URLSearchParams({ account })
  if (invoiceRef) params.set('invoice_ref', invoiceRef)
  return apiUploadWithProgress<InvoiceUploadResponse>(
    `/merchant-account/${merchantId}/connectors/${connector}/invoice?${params.toString()}`,
    file,
    onProgress,
  )
}

/** Clear a connector's invoice add-on, reverting its served cost to the learned-only model. */
export async function deleteInvoiceAddon(merchantId: string, connector: string) {
  return apiDelete(`/merchant-account/${merchantId}/connectors/${connector}/invoice-addon`)
}
