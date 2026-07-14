import { useEffect, useRef, useState } from 'react'
import { Upload } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  uploadReport,
  useCostCoverage,
  useIngestionHistory,
} from '../../hooks/useCostRouting'
import { Field, UPLOAD_CONNECTORS, inputClass } from './CostRoutingShared'

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
  const [connector, setConnector] = useState('adyen')
  const [account, setAccount] = useState('')
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [uploadPct, setUploadPct] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [activeJobId, setActiveJobId] = useState<number | null>(null)

  const activeJob = activeJobId != null ? ingestions.find((j) => j.id === activeJobId) : undefined

  // When the tracked job finishes, refresh the coverage card.
  useEffect(() => {
    if (activeJob?.status === 'completed') mutateCoverage()
  }, [activeJob?.status, mutateCoverage])

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

  const processing = activeJob?.status === 'processing'
  const done = activeJob?.status === 'completed'
  const failed = activeJob?.status === 'failed'

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <Upload size={16} className="text-brand-500" />
          <div>
            <SurfaceLabel>Manual upload</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              Upload a settlement report
            </h2>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        <p className="text-sm text-slate-500 dark:text-[#9ca7ba]">
          Upload a settlement report file directly — no webhook needed. Processing runs in the
          background (the upload won't hang on large files); watch progress below and in the history.
        </p>
        <Field label="Connector">
          <select
            className={inputClass}
            value={connector}
            onChange={(e) => setConnector(e.target.value)}
          >
            {UPLOAD_CONNECTORS.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
              </option>
            ))}
          </select>
        </Field>
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
            onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          />
        </Field>

        <div className="flex items-center gap-3">
          <Button onClick={handleUpload} disabled={!merchantId || uploading || processing}>
            {uploading || processing ? (
              <>
                <Spinner size={14} />
                {uploading ? 'Uploading…' : 'Processing…'}
              </>
            ) : (
              'Upload & fit'
            )}
          </Button>
          <span className="text-xs text-slate-400">Runs in the background after upload.</span>
        </div>

        {/* Upload transfer bar */}
        {uploading && (
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
