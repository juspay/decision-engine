import { useMemo } from 'react'
import useSWR from 'swr'
import { apiDelete, apiPost, apiPut, apiUploadWithProgress, fetcher, type UploadProgress } from '../lib/api'

export interface CoverageSummary {
  total_clusters: number
  good_clusters: number
  thin_clusters: number
  /** One line can't price these, but their recovered amount tiers can — the router serves them. */
  tiered_clusters: number
  non_linear_clusters: number
  total_txns: number
  good_txns: number
  tiered_txns: number
  thin_txns: number
  non_linear_txns: number
  good_txn_pct: number
  /** Share of transactions the router can price: GOOD **plus** TIERED. */
  priced_txn_pct: number
  total_gross: number
  good_gross: number
  tiered_gross: number
  thin_gross: number
  non_linear_gross: number
  /** Share of settled volume with a trustworthy SINGLE-LINE model. */
  good_gross_pct: number
  /**
   * Share of settled volume the router can actually price: GOOD **plus** TIERED — the real coverage
   * headline. `good_gross_pct` alone under-reports any merchant whose big-ticket segments are capped
   * (UAE/Saudi debit is 100% tiered), because a capped cluster's whole-cluster verdict is
   * NON_LINEAR by construction even though its tiers price it fine.
   */
  priced_gross_pct: number
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
  /** Same UUIDv7 string as `IngestionDto.id`. */
  id: string
  status: string
}

/** One ingestion (history + live progress). */
export interface IngestionDto {
  /** UUIDv7 string (`cost_ingestion.id` is a Varchar), despite reading like a numeric id. */
  id: string
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
 * One recovered piece of a capped/tiered cluster: the fee it charges over an amount range `[lo, hi)`.
 * A cluster whose interchange has an absolute cap (e.g. UAE debit `1.00% cap AED 50`) or a tiered
 * schedule can't be priced by one line, so the fit recovers it into several of these.
 */
export interface ClusterSegment {
  seg_idx: number
  /** Amount range this piece prices: `[lo, hi)`. */
  lo: number
  hi: number
  pct_bps: number | null
  fixed: number | null
  bps_rmse: number | null
  n: number
  gross_sum: number
  /** `GOOD` | `THIN` | `NON_LINEAR`. */
  verdict: string
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
  /** `null` when the cluster has no usable rate (a one-transaction THIN cluster fits to nothing). */
  effective_pct_bps: number | null
  effective_fixed: number | null
  source: 'override' | 'model' | 'segmented' | string
  /**
   * Fit grade of the learned rate. `''` when no fitted row backs the cluster (override-only, or a
   * segmented cluster outside the fetched top-N). Non-GOOD rates are shown so they can be corrected;
   * they are not what the router trusts.
   */
  verdict: 'GOOD' | 'THIN' | 'NON_LINEAR' | ''
  /**
   * Card-program proxy: the issuer BIN's dominant interchange rate as integer bps (`'115'`), or `''`
   * when the report carried no PAN. Within one network+funding+country the rate tier is the program.
   */
  card_product: string
  /**
   * Recovered per-segment rates for a capped/tiered cluster; empty for an ordinary single-line
   * cluster. Display-only for now — the decide path is not yet segment-aware.
   */
  segments?: ClusterSegment[]
}

export interface ClustersScope {
  /** Scope to one ingested snapshot's segments (all three required together). */
  connector?: string
  account?: string
  reportDate?: string
  /**
   * Scope to ONE upload's clusters. Needed because the fit is a rolling-window re-fit that replaces
   * the whole snapshot: two uploads on the same day under the same account share a snapshot, so the
   * three fields above return both reports' clusters.
   */
  ingestionId?: string
}

/**
 * Per-column narrowing of the cluster list. Each column holds a SET of accepted values — values
 * within a column are OR'd, columns are AND'd — and `q` is a free-text substring across all of them.
 * Applied in ClickHouse, not the browser: the list is long-tailed (a real report fits ~1.6k clusters,
 * most of them low-traffic), so the tail has to be reachable without shipping every row.
 */
export interface ClusterFilters {
  card_network?: string[]
  variant?: string[]
  funding?: string[]
  issuer_country?: string[]
  currency?: string[]
  ic_category?: string[]
  verdict?: string[]
  q?: string
}

/** How many values are selected across every column (plus the free-text box). */
export function countActiveFilters(f: ClusterFilters): number {
  let n = f.q?.trim() ? 1 : 0
  for (const [k, v] of Object.entries(f)) {
    if (k !== 'q' && Array.isArray(v)) n += v.length
  }
  return n
}

/** The filterable columns, in table order — drives both the filter bar and the facets lookup. */
export const CLUSTER_FILTER_COLUMNS = [
  'card_network',
  'variant',
  'funding',
  'issuer_country',
  'currency',
  'ic_category',
  'verdict',
] as const

/**
 * The merchant's top clusters by GMV. Merchant-wide by default (the override targets, plus any
 * overridden clusters); pass a snapshot scope to get one ingestion's fitted segments instead.
 */
export function useCostClusters(
  merchantId?: string,
  opts: { limit?: number; order?: 'gross_sum' | 'n'; filters?: ClusterFilters } & ClustersScope = {},
) {
  const { limit = 10, order, connector, account, reportDate, ingestionId, filters } = opts
  let path: string | null = null
  if (merchantId) {
    const params = new URLSearchParams({ limit: String(limit) })
    // Multi-valued columns go out as REPEATED keys (`?card_network=visa&card_network=mc`) — a
    // delimiter would corrupt values that legitimately contain one (several `ic_category` names
    // contain a comma). Blank entries are dropped: they'd ask the backend to match the empty string.
    for (const [k, v] of Object.entries(filters ?? {})) {
      if (Array.isArray(v)) {
        for (const one of v) if (one?.trim()) params.append(k, one.trim())
      } else if (typeof v === 'string' && v.trim()) {
        params.set(k, v.trim())
      }
    }
    // Each scope dimension is independent: a connector (+optional account) narrows to that
    // connector's latest-snapshot segments; adding report_date pins one exact ingestion. The
    // backend AND-combines whichever are present, so send each one we have.
    if (connector) params.set('connector', connector)
    if (account) params.set('account', account)
    if (reportDate) params.set('report_date', reportDate)
    if (ingestionId) params.set('ingestion_id', ingestionId)
    // `order` picks which top-N the backend selects (not just display order): 'n' ranks by txn count,
    // otherwise settled GMV. Only send the non-default so GMV views keep clean URLs.
    if (order === 'n') params.set('order', 'txns')
    path = `/merchant-account/${merchantId}/cost-clusters?${params.toString()}`
  }
  const { data, error, isLoading, mutate } = useSWR<ClusterFee[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { clusters: data ?? [], error, isLoading, mutate }
}

/** One suggestible value for one filterable column, with the traffic behind it. */
export interface ClusterFacet {
  dim: string
  value: string
  txns: number
}

/**
 * Distinct values for every filterable column, highest-traffic first — the autosuggestions behind the
 * filter bar. Scoped like the cluster list, so a snapshot view only suggests values in that snapshot.
 * Returns them grouped by column, ready to feed a `<datalist>`.
 */
export function useClusterFacets(merchantId?: string, scope: ClustersScope = {}) {
  const { connector, account, reportDate, ingestionId } = scope
  let path: string | null = null
  if (merchantId) {
    const params = new URLSearchParams()
    if (connector) params.set('connector', connector)
    if (account) params.set('account', account)
    if (reportDate) params.set('report_date', reportDate)
    if (ingestionId) params.set('ingestion_id', ingestionId)
    const qs = params.toString()
    path = `/merchant-account/${merchantId}/cost-cluster-facets${qs ? `?${qs}` : ''}`
  }
  const { data, error, isLoading } = useSWR<ClusterFacet[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  const byDim = useMemo(() => {
    const m: Record<string, ClusterFacet[]> = {}
    for (const f of data ?? []) (m[f.dim] ??= []).push(f)
    return m
  }, [data])
  return { facets: byDim, error, isLoading }
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

// ── Seed costs (contract rates for the multi-objective simulator) ────────────────────────────────

/**
 * One row of the merchant's seed cost table: a PSP's fee for one card scenario, split into the
 * contract components (interchange / scheme / markup, all bps) plus the flat `fixed` per-txn fee.
 * The matching dimensions (`card_network` … `card_issuing_country`) are wildcards when empty;
 * `is_default` marks the PSP's fallback row (no dimensions).
 */
export interface SeedCostRow {
  psp: string
  card_network?: string | null
  payment_method_type?: string | null
  card_type?: string | null
  transaction_currency?: string | null
  card_issuing_country?: string | null
  interchange_bps: number
  scheme_bps: number
  markup_bps: number
  fixed: number
  label?: string | null
  example_amount?: number | null
  is_default: boolean
  /** Computed Total Effective Rate percentage = interchange + scheme + markup. */
  effective_pct_bps: number
}

/** The merchant's editable seed cost table (their saved edits, else the config default). */
export function useSeedCosts(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/seed-costs` : null
  const { data, error, isLoading, mutate } = useSWR<SeedCostRow[]>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { rows: data ?? [], error, isLoading, mutate }
}

/** Replace the merchant's whole seed table; returns the normalized saved table. */
export async function saveSeedCosts(merchantId: string, rows: SeedCostRow[]) {
  return apiPut<SeedCostRow[]>(`/merchant-account/${merchantId}/seed-costs`, { rows })
}

/** Clear the merchant's edits, reverting to the config default; returns the default table. */
export async function resetSeedCosts(merchantId: string) {
  return apiDelete<SeedCostRow[]>(`/merchant-account/${merchantId}/seed-costs`)
}

/** A scenario to price in the seed-cost simulation preview. */
export interface SeedSimRequest {
  amount: number
  transaction_currency?: string
  card_network?: string
  payment_method_type?: string
  card_type?: string
  card_issuing_country?: string
  /** PSPs to price; omit for every PSP in the table. */
  psps?: string[]
}

/** One PSP's estimated cost for the simulated scenario, from the merchant's configured values. */
export interface SeedSimRow {
  psp: string
  interchange_bps: number
  scheme_bps: number
  markup_bps: number
  fixed: number
  effective_pct_bps: number
  /** All-in effective rate at this amount = pct + fixed/amount·10_000. */
  effective_cost_bps: number
  /** Absolute cost in the transaction currency at this amount. */
  cost_amount: number
}

/**
 * Price the candidate PSPs for one (currency, scenario, amount) using the merchant's configured
 * seed table — the exact resolver the decide path uses, so the preview is faithful.
 */
export async function simulateSeedCost(merchantId: string, body: SeedSimRequest) {
  return apiPost<SeedSimRow[]>(`/merchant-account/${merchantId}/seed-costs/simulate`, body)
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

// ── Column mapping ───────────────────────────────────────────────────────────────────────────────

/** A connector's verdict on a report's header row. */
export interface PreflightReport {
  connector: string
  ok: boolean
  /** Required columns absent from the file. */
  missing: string[]
  /** Required columns the file does carry. */
  matched: string[]
  /** Everything the connector requires. */
  required: string[]
  /** Columns the connector uses if present but tolerates the absence of. */
  optional: string[]
  /**
   * Optional columns this file lacks. These never fail an ingestion — which is why they are worth
   * showing: their absence is silent but not free. Adyen's `Unique Terminal ID` is what separates
   * in-person from online acceptance, and `Booking Date` dates the report period; a renamed one
   * quietly degrades the fitted model. Offer them, don't block on them.
   */
  optional_missing: string[]
  /** The header labels the merchant's file actually has. */
  found: string[]
  /** Connectors that fully accept this file — a strong hint the wrong one was selected. */
  suggested_connectors: { connector: string; matched_required: number }[]
}

/** One row as a candidate mapping would produce it — the derived values, not the raw columns. */
export interface PreviewRow {
  card_network: string
  variant: string
  funding: string
  currency: string
  issuer_country: string
  gross: number
  total_fee: number
  effective_pct: number
  interchange: number
  scheme_fee: number
  markup: number
  commission: number
}

export interface PreviewReport {
  rows: PreviewRow[]
  median_effective_pct: number | null
  /** Set when the derived numbers don't look like card processing. Advisory, not blocking. */
  warning: string | null
}

/** `expected label -> the merchant's label`. */
export type ColumnMapping = Record<string, string>

/**
 * How much of the file to send for header checks. Must not exceed the server's own cap
 * (`preflight::HEADER_SAMPLE_BYTES`), which rejects a larger body outright.
 */
export const HEADER_SAMPLE_BYTES = 64 * 1024

/**
 * Check a report's headers against a connector *before* uploading it. Send only
 * `file.slice(0, HEADER_SAMPLE_BYTES)` — a few KB answered in milliseconds, versus discovering the
 * same problem after a multi-GB upload and a background parse.
 *
 * Passing `account` applies that source's saved mapping, so a previously-mapped source validates
 * clean straight away. A file that can't be parsed is a normal response with `ok: false`, not a
 * rejected request.
 */
export async function validateReportHeaders(
  merchantId: string,
  connector: string,
  headerSample: Blob,
  account?: string,
) {
  const q = account ? `?account=${encodeURIComponent(account)}` : ''
  return apiUploadWithProgress<PreflightReport>(
    `/merchant-account/${merchantId}/connectors/${connector}/report/validate-headers${q}`,
    headerSample,
  )
}

/**
 * Show what a *candidate* mapping actually produces from the merchant's own rows.
 *
 * This is the guardrail that makes mapping safe to offer. A mapping can be perfectly well-formed —
 * every column known, every target present — and still be wrong (an all-in fee column pointed at one
 * fee component, say). A wrong mapping doesn't error: it fits, grades GOOD, and silently misprices
 * routing. The derived `gross` / `total_fee` / effective rate is where that becomes visible, so the
 * UI renders this before it will let a mapping be saved.
 */
export async function previewColumnMapping(
  merchantId: string,
  connector: string,
  columns: ColumnMapping,
  sample: string,
  truncated: boolean,
) {
  return apiPost<PreviewReport>(
    `/merchant-account/${merchantId}/connectors/${connector}/report/preview`,
    { columns, sample, truncated },
  )
}

/** A connector whose settlement report can be ingested. */
export interface IngestConnector {
  id: string
  /** Reports are fetched by polling the connector's API rather than pushed to a webhook. */
  pull: boolean
}

/**
 * Connectors that support report ingestion, read from the backend's connector registry.
 *
 * Deliberately not a constant in this file: the registry is the single source of truth for which
 * connectors exist, and a hardcoded copy here would silently make a newly-registered connector
 * unselectable — and therefore unmappable — until someone remembered to edit the frontend too.
 */
export function useIngestConnectors() {
  const { data, error, isLoading } = useSWR<IngestConnector[]>(
    '/cost-ingestion/connectors',
    fetcher,
    { revalidateOnFocus: false },
  )
  return { connectors: data ?? [], error, isLoading }
}

/** A settlement source's saved column mapping (empty object when none is set). */
export function useColumnMapping(merchantId?: string, connector?: string, account?: string) {
  const path =
    merchantId && connector && account
      ? `/merchant-account/${merchantId}/connectors/${connector}/report/column-mapping?account=${encodeURIComponent(account)}`
      : null
  const { data, error, isLoading, mutate } = useSWR<{ columns: ColumnMapping }>(path, fetcher, {
    revalidateOnFocus: false,
  })
  return { mapping: data?.columns ?? {}, error, isLoading, mutate }
}

/**
 * Save a mapping for a settlement source. Every future ingestion of it — upload, webhook, or poll —
 * applies the mapping automatically, which is most of the value: it turns a recurring monthly chore
 * into a one-time one.
 *
 * `sample` is required (and must be the file the mapping was written against) so the server can
 * reject a mapping that names an unknown column, targets a column the file lacks, or points two
 * expected columns at the same source column.
 */
export async function setColumnMapping(
  merchantId: string,
  connector: string,
  account: string,
  columns: ColumnMapping,
  sample: string,
  truncated: boolean,
) {
  return apiPut(
    `/merchant-account/${merchantId}/connectors/${connector}/report/column-mapping?account=${encodeURIComponent(account)}`,
    { columns, sample, truncated },
  )
}

/** Clear a source's mapping so its reports parse with the connector's own labels again. */
export async function deleteColumnMapping(
  merchantId: string,
  connector: string,
  account: string,
) {
  return apiDelete(
    `/merchant-account/${merchantId}/connectors/${connector}/report/column-mapping?account=${encodeURIComponent(account)}`,
  )
}

/**
 * Run a curated sample report for a connector ("Use a sample file") — for merchants without a
 * report file of their own. The server downloads the configured sample and runs the identical
 * pipeline, returning a job id (202) to poll via {@link useIngestionHistory}. Rejects with a 404 if
 * no sample is configured for the connector.
 */
export async function runSampleReport(merchantId: string, connector: string) {
  return apiPost<UploadAccepted>(
    `/merchant-account/${merchantId}/connectors/${connector}/report/sample`,
  )
}

/**
 * Delete (undo) an ingestion: removes its fitted snapshot + staged rows and its history row.
 * Coverage/serving revert to the previous snapshot automatically.
 */
export async function deleteIngestion(merchantId: string, id: string) {
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
