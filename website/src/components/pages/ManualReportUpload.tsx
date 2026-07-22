import { useCallback, useEffect, useRef, useState } from 'react'
import { Upload } from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import * as type from '../ui/typography'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  HEADER_SAMPLE_BYTES,
  runSampleReport,
  uploadReport,
  useCostCoverage,
  useIngestConnectors,
  useIngestionHistory,
  validateReportHeaders,
  type PreflightReport,
} from '../../hooks/useCostRouting'
import { Field, UPLOAD_CONNECTORS, inputClass } from './CostRoutingShared'
import { ColumnMappingPanel } from './ColumnMappingPanel'

type IngestMode = 'upload' | 'sample'

/**
 * Manual-ingestion tab: upload a settlement report file directly. The upload returns immediately
 * (202 + job id) and the server processes it in the background, so a multi-GB report never hangs
 * the request. Progress is two-phase: a determinate upload bar (bytes transferred), then a live
 * "processing" state driven by polling the job's `staged_rows`.
 */
export function ManualReportUpload({ merchantId }: { merchantId?: string }) {
  // Shares SWR keys with the coverage card and history table; mutating refreshes them.
  const { mutate: mutateCoverage } = useCostCoverage(merchantId)
  const { ingestions, mutate: mutateHistory } = useIngestionHistory(merchantId)

  const fileRef = useRef<HTMLInputElement>(null)
  // Sample-first: a merchant landing here usually has no report file to hand, so the default is the
  // path that works with nothing — run a curated sample end to end, then switch to a real upload.
  const [mode, setMode] = useState<IngestMode>('sample')
  const [connector, setConnector] = useState('adyen')
  const [account, setAccount] = useState('')
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [uploadPct, setUploadPct] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [activeJobId, setActiveJobId] = useState<number | null>(null)

  // Header preflight, run the moment a file is chosen. `preflight` is null until it has run;
  // `sampleText` is the same few KB it ran on, kept so the mapping panel can show example values and
  // send the sample back with a candidate mapping.
  const [preflight, setPreflight] = useState<PreflightReport | null>(null)
  const [sampleText, setSampleText] = useState('')
  /** Whether `sampleText` is only the head of the chosen file. */
  const [truncated, setTruncated] = useState(false)
  /**
   * Whether the merchant has dismissed the mapping panel. Deliberately separate from `preflight`:
   * clearing the verdict to hide the panel would also clear what the Upload button is gated on,
   * so "close this panel" would silently become "let me upload a file we already know cannot be
   * parsed" — the exact outcome the preflight exists to prevent.
   */
  const [mappingDismissed, setMappingDismissed] = useState(false)
  const [checking, setChecking] = useState(false)

  // Which connectors exist comes from the backend registry, so registering one there is enough to
  // make it selectable (and mappable) here. UPLOAD_CONNECTORS is now only a display-name table:
  // anything it doesn't know falls back to a capitalised id rather than disappearing from the list.
  const { connectors: ingestConnectors } = useIngestConnectors()
  const connectorOptions = (
    ingestConnectors.length > 0
      ? ingestConnectors.map((c) => c.id)
      : UPLOAD_CONNECTORS.map((c) => c.value)
  ).map((id) => ({
    value: id,
    label:
      UPLOAD_CONNECTORS.find((c) => c.value === id)?.label ??
      id.charAt(0).toUpperCase() + id.slice(1),
  }))

  const connectorLabel = connectorOptions.find((c) => c.value === connector)?.label ?? connector

  const activeJob = activeJobId != null ? ingestions.find((j) => j.id === activeJobId) : undefined

  // When the tracked job finishes, refresh the coverage card.
  useEffect(() => {
    if (activeJob?.status === 'completed') mutateCoverage()
  }, [activeJob?.status, mutateCoverage])

  // A preflight verdict is only meaningful for the connector and account it was run against —
  // both decide which columns are required and which saved mapping applies. Re-check on either
  // change rather than leaving a stale verdict on screen.
  useEffect(() => {
    if (file) void runPreflight(file)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `file` is handled by handleFileChange
  }, [connector, account])

  /**
   * Check a newly-chosen file's headers before anything is uploaded. Only the first
   * {@link HEADER_SAMPLE_BYTES} are sent, so this costs a few KB and answers in milliseconds —
   * versus the alternative of discovering the same problem after a multi-GB transfer and a
   * background parse, one missing column at a time.
   */
  const runPreflight = useCallback(
    async (chosen: File) => {
      if (!merchantId) return
      setChecking(true)
      setPreflight(null)
      setError(null)
      try {
        const slice = chosen.slice(0, HEADER_SAMPLE_BYTES)
        // Read and send the same bytes, so the mapping panel's example values line up exactly with
        // the headers the server reports.
        const text = await slice.text()
        setSampleText(text)
        setTruncated(chosen.size > HEADER_SAMPLE_BYTES)
        setMappingDismissed(false)
        setPreflight(await validateReportHeaders(merchantId, connector, slice, account || undefined))
      } catch (e: unknown) {
        // A preflight failure must not block the upload — it is an early warning, not a gate. The
        // server re-validates on ingest regardless.
        setPreflight(null)
        setError(e instanceof Error ? e.message : 'Could not check this file’s columns')
      } finally {
        setChecking(false)
      }
    },
    [merchantId, connector, account],
  )

  function handleFileChange(chosen: File | null) {
    setFile(chosen)
    setPreflight(null)
    setSampleText('')
    setMappingDismissed(false)
    setError(null)
    if (chosen) void runPreflight(chosen)
  }

  async function handleUpload() {
    if (!merchantId) {
      setError('Set a merchant ID first')
      return
    }
    if (!account || !file) {
      setError('Account and a report file are both required')
      return
    }
    setUploading(true)
    setUploadPct(0)
    setError(null)
    setActiveJobId(null)
    try {
      const res = await uploadReport(merchantId, connector, account, file, (p) => {
        if (p.phase === 'uploading') {
          setUploadPct(p.total ? Math.round((p.loaded / p.total) * 100) : 0)
        } else {
          setUploadPct(100)
        }
      })
      setActiveJobId(res.id)
      setFile(null)
      if (fileRef.current) fileRef.current.value = ''
      await mutateHistory() // start polling immediately
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to upload report')
    } finally {
      setUploading(false)
    }
  }

  async function handleRunSample() {
    if (!merchantId) {
      setError('Set a merchant ID first')
      return
    }
    setUploading(true)
    setUploadPct(100)
    setError(null)
    setActiveJobId(null)
    try {
      const res = await runSampleReport(merchantId, connector)
      setActiveJobId(res.id)
      await mutateHistory() // start polling immediately
    } catch (e: unknown) {
      // A 404 means no sample is configured for this connector — surface it plainly.
      const msg = e instanceof Error ? e.message : ''
      setError(
        /404|not found|no sample/i.test(msg)
          ? `No sample report available for ${connectorLabel}.`
          : msg || 'Failed to run sample report',
      )
    } finally {
      setUploading(false)
    }
  }

  const processing = activeJob?.status === 'processing'
  const done = activeJob?.status === 'completed'
  const failed = activeJob?.status === 'failed'

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <Upload size={16} className="text-brand-500" />
          <div>
            <h2 className={type.heading}>Upload a settlement report</h2>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        {/* Choose between a real file upload and a curated sample, for merchants without a report. */}
        <div className="inline-flex rounded-lg border border-slate-200 p-0.5 dark:border-[#2a3344]">
          {(
            [
              ['sample', 'Use a sample file'],
              ['upload', 'Upload a file'],
            ] as [IngestMode, string][]
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              onClick={() => {
                setMode(value)
                setError(null)
              }}
              className={
                'rounded-md px-3 py-1.5 text-sm font-medium transition-colors ' +
                (mode === value
                  ? 'bg-brand-500 text-white'
                  : 'text-slate-500 hover:text-slate-700 dark:text-[#9ca7ba] dark:hover:text-white')
              }
            >
              {label}
            </button>
          ))}
        </div>
        <p className="text-sm text-slate-500 dark:text-[#9ca7ba]">
          {mode === 'upload'
            ? "Upload a settlement report file directly — no webhook needed. Processing runs in the background (the upload won't hang on large files); watch progress below and in the history."
            : `No report file? Run a curated ${connectorLabel} sample report through the exact same fit, so you can see the ingest → cost-coverage flow end to end. Progress shows below and in the history.`}
        </p>
        <Field label="Connector">
          <select
            className={inputClass}
            value={connector}
            onChange={(e) => setConnector(e.target.value)}
          >
            {connectorOptions.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </Field>
        {mode === 'upload' && (
          <>
            <Field label="Account" hint="Connector-side account the report belongs to">
              <input
                className={inputClass}
                value={account}
                onChange={(e) => setAccount(e.target.value)}
                placeholder="AcmeMerchantEU"
              />
            </Field>
            <Field label="Report file" hint="The connector's settlement / PAR report (CSV)">
              <input
                ref={fileRef}
                type="file"
                accept=".csv,text/csv,text/plain"
                className={inputClass}
                onChange={(e) => handleFileChange(e.target.files?.[0] ?? null)}
              />
            </Field>

            {/* Header check — resolved before a single byte of the report is uploaded. */}
            {checking && (
              <p className="flex items-center gap-2 text-sm text-slate-500 dark:text-[#9ca7ba]">
                <Spinner size={14} />
                Checking this file's columns…
              </p>
            )}

            {preflight?.ok && (
              <p className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-sm text-emerald-700 dark:border-emerald-900/40 dark:bg-emerald-950/30 dark:text-emerald-400">
                All {preflight.required.length} required columns found — this file is ready to
                upload.
              </p>
            )}

            {preflight && !preflight.ok && !mappingDismissed && merchantId && account && (
              <ColumnMappingPanel
                merchantId={merchantId}
                connector={connector}
                account={account}
                preflight={preflight}
                sampleText={sampleText}
                truncated={truncated}
                onCancel={() => setMappingDismissed(true)}
                onSaved={() => {
                  // Re-run preflight with the mapping now saved: it should come back clean, which
                  // both confirms the mapping took effect and swaps the panel for the ready state.
                  if (file) void runPreflight(file)
                }}
              />
            )}

            {/* Dismissed the panel but the file still cannot be parsed: keep the reason visible
                rather than leaving a disabled button with no explanation. */}
            {preflight && !preflight.ok && mappingDismissed && account && (
              <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700 dark:border-amber-900/40 dark:bg-amber-950/20 dark:text-amber-500">
                This file is still missing {preflight.missing.length} required{' '}
                {preflight.missing.length === 1 ? 'column' : 'columns'}, so it can't be uploaded.{' '}
                <button
                  type="button"
                  onClick={() => setMappingDismissed(false)}
                  className="font-medium underline underline-offset-2"
                >
                  Map columns
                </button>{' '}
                or choose a different file.
              </p>
            )}

            {/* Mapping is stored per (connector, account), so there is nothing to save it under
                until the account is filled in. */}
            {preflight && !preflight.ok && !account && (
              <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700 dark:border-amber-900/40 dark:bg-amber-950/20 dark:text-amber-500">
                This file is missing {preflight.missing.length} required{' '}
                {preflight.missing.length === 1 ? 'column' : 'columns'}. Enter the account above to
                map your columns to {connector}'s.
              </p>
            )}
          </>
        )}

        <div className="flex items-center gap-3">
          {/* Upload is blocked while we *know* the file won't parse — the whole point of the
              preflight is to not spend a multi-GB upload on a file that is going to fail. A
              preflight that errored leaves `preflight` null and does not block: it is an early
              warning, and the server validates on ingest regardless. */}
          {mode === 'upload' ? (
            <Button
              onClick={handleUpload}
              disabled={
                !merchantId || uploading || processing || checking || preflight?.ok === false
              }
            >
              {uploading || processing ? (
                <>
                  <Spinner size={14} />
                  {uploading ? 'Uploading…' : 'Processing…'}
                </>
              ) : (
                'Upload & fit'
              )}
            </Button>
          ) : (
            <Button onClick={handleRunSample} disabled={!merchantId || uploading || processing}>
              {uploading || processing ? (
                <>
                  <Spinner size={14} />
                  {uploading ? 'Starting…' : 'Processing…'}
                </>
              ) : (
                'Run sample'
              )}
            </Button>
          )}
          <span className="text-xs text-slate-400">
            {mode === 'upload'
              ? 'Runs in the background after upload.'
              : 'Fetches the sample and runs in the background.'}
          </span>
        </div>

        {/* Upload transfer bar (file upload only) */}
        {mode === 'upload' && uploading && (
          <ProgressBar
            label={`Uploading report… ${uploadPct}%`}
            pct={uploadPct}
            showPct={false}
          />
        )}

        {/* Server-side processing (rows staged live via polling) */}
        {processing && activeJob && (
          <ProgressBar
            label={`Processing — ${activeJob.staged_rows.toLocaleString()} rows staged…`}
            pct={100}
            pulse
          />
        )}

        <ErrorMessage error={error} />

        {done && activeJob && (
          <div className="rounded-lg border border-emerald-200 bg-emerald-50 p-4 dark:border-emerald-900/40 dark:bg-emerald-950/30">
            <p className="text-sm font-medium text-emerald-700 dark:text-emerald-400">
              Ingested {activeJob.staged_rows.toLocaleString()} rows for {activeJob.connector} /{' '}
              {activeJob.account}
              {activeJob.period_start && activeJob.period_end
                ? ` (${activeJob.period_start} → ${activeJob.period_end})`
                : ''}
              .
            </p>
            <p className="mt-1 text-sm text-emerald-700 dark:text-emerald-400">
              {activeJob.good_clusters} of {activeJob.total_clusters} clusters graded GOOD ·{' '}
              {activeJob.currency_count} currencies · {activeJob.country_count} countries.
              {activeJob.good_clusters === 0 &&
                ' No trustworthy cost model yet — this volume still falls back to SR routing.'}
            </p>
          </div>
        )}

        {failed && activeJob && (
          <ErrorMessage error={activeJob.last_error || 'Ingestion failed'} />
        )}
      </CardBody>
    </Card>
  )
}

function ProgressBar({
  label,
  pct,
  pulse,
  showPct = false,
}: {
  label: string
  pct: number
  pulse?: boolean
  showPct?: boolean
}) {
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs text-slate-500 dark:text-[#9ca7ba]">
        <span>{label}</span>
        {showPct && <span>{pct}%</span>}
      </div>
      <div className="h-2 w-full overflow-hidden rounded-full bg-slate-200 dark:bg-[#232833]">
        <div
          className={`h-full rounded-full bg-brand-500 transition-all duration-200 ${
            pulse ? 'animate-pulse' : ''
          }`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}
