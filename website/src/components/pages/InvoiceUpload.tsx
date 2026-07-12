import { useRef, useState } from 'react'
import { FileText, Check, Info, Trash2 } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  uploadInvoice,
  deleteInvoiceAddon,
  useInvoiceAddons,
  type InvoiceUploadResponse,
  type InvoiceLineDto,
} from '../../hooks/useCostRouting'
import { Field, inputClass } from './CostRoutingShared'

/** Connectors whose invoice parser is implemented. Only Adyen for now (Braintree invoice pending). */
const INVOICE_CONNECTORS = [{ value: 'adyen', label: 'Adyen' }] as const

/** Money / rate formatting. Invoice currency may be blank on odd exports → fall back to a plain code. */
function money(v: number, ccy: string, digits = 2): string {
  const opts: Intl.NumberFormatOptions = ccy
    ? { style: 'currency', currency: ccy, maximumFractionDigits: digits }
    : { maximumFractionDigits: digits }
  try {
    return new Intl.NumberFormat(undefined, opts).format(v)
  } catch {
    return `${v.toFixed(digits)}${ccy ? ` ${ccy}` : ''}`
  }
}

const KIND_LABEL: Record<string, string> = {
  flat_per_txn: 'Flat per transaction',
  periodic: 'Periodic / non-transactional',
  credit: 'Credit',
  already_modeled: 'Already in report',
  volume: 'Turnover',
}

/**
 * Invoice ingestion tab. Upload the monthly connector invoice; we identify the fees that never
 * appear on the settlement report (PAR) — flat per-transaction fees, periodic fees, credits — reduce
 * them to a per-transaction add-on, and show the merchant exactly what we found and how much extra
 * will be applied to every routing decision for that connector account.
 */
export function InvoiceUpload({ merchantId }: { merchantId?: string }) {
  const { addons, mutate: mutateAddons } = useInvoiceAddons(merchantId)

  const fileRef = useRef<HTMLInputElement>(null)
  const [connector, setConnector] = useState('adyen')
  const [account, setAccount] = useState('')
  const [invoiceRef, setInvoiceRef] = useState('')
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<InvoiceUploadResponse | null>(null)

  async function handleUpload() {
    if (!merchantId) return setError('Set a merchant ID first')
    if (!account || !file) return setError('Account and an invoice file are both required')
    setUploading(true)
    setError(null)
    setResult(null)
    try {
      const res = await uploadInvoice(merchantId, connector, account, file, invoiceRef || undefined)
      setResult(res)
      setFile(null)
      if (fileRef.current) fileRef.current.value = ''
      await mutateAddons()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to process invoice')
    } finally {
      setUploading(false)
    }
  }

  async function handleRemove(conn: string) {
    if (!merchantId) return
    try {
      await deleteInvoiceAddon(merchantId, conn)
      await mutateAddons()
      if (result?.connector === conn) setResult(null)
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to remove add-on')
    }
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <FileText size={16} className="text-brand-500" />
            <div>
              <SurfaceLabel>Invoice</SurfaceLabel>
              <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
                Upload a connector invoice
              </h2>
            </div>
          </div>
        </CardHeader>
        <CardBody className="space-y-4">
          <p className="text-sm text-slate-500 dark:text-[#9ca7ba]">
            The settlement report only carries a transaction's core fees, so our model was
            structurally missing ~9% of the true cost — flat per-transaction fees, periodic fees and
            credits that only appear on the monthly invoice. Upload that invoice and we'll identify
            those missing fees and start applying them to every routing decision.
          </p>
          <Field label="Connector">
            <select
              className={inputClass}
              value={connector}
              onChange={(e) => setConnector(e.target.value)}
            >
              {INVOICE_CONNECTORS.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Account" hint="Connector-side account the invoice covers">
            <input
              className={inputClass}
              value={account}
              onChange={(e) => setAccount(e.target.value)}
              placeholder="AcmeMerchantEU"
            />
          </Field>
          <Field label="Invoice number" hint="Optional — for your records">
            <input
              className={inputClass}
              value={invoiceRef}
              onChange={(e) => setInvoiceRef(e.target.value)}
              placeholder="NL2025…"
            />
          </Field>
          <Field
            label="Invoice file"
            hint="The monthly invoice — a PDF (as downloaded from Adyen) or a CSV export"
          >
            <input
              ref={fileRef}
              type="file"
              accept=".pdf,application/pdf,.csv,text/csv,text/plain"
              className={inputClass}
              onChange={(e) => setFile(e.target.files?.[0] ?? null)}
            />
          </Field>

          <div className="flex items-center gap-3">
            <Button onClick={handleUpload} disabled={!merchantId || uploading}>
              {uploading ? (
                <>
                  <Spinner size={14} />
                  Reading invoice…
                </>
              ) : (
                'Upload & identify fees'
              )}
            </Button>
          </div>

          <ErrorMessage error={error} />
        </CardBody>
      </Card>

      {result && <ResultCard result={result} />}

      {addons.length > 0 && (
        <ActiveAddons addons={addons} onRemove={handleRemove} />
      )}
    </div>
  )
}

/** The "here's what we found" card shown after a successful upload. */
function ResultCard({ result }: { result: InvoiceUploadResponse }) {
  const ccy = result.currency
  const added = result.breakdown.filter((l) => l.added)
  const ignored = result.breakdown.filter((l) => !l.added)

  return (
    <Card>
      <CardBody className="space-y-5">
        {/* Headline */}
        <div className="rounded-xl border border-brand-500/30 bg-brand-500/5 p-5">
          <div className="flex items-center gap-2 text-brand-600 dark:text-brand-400">
            <Check size={16} />
            <span className="text-sm font-medium">
              We understood the fees missing from your settlement report
            </span>
          </div>
          <p className="mt-3 text-3xl font-semibold text-slate-900 dark:text-white">
            +{money(result.total_addon_per_txn, ccy, 4)}
            <span className="ml-2 text-base font-normal text-slate-500 dark:text-[#9ca7ba]">
              additional cost per transaction
            </span>
          </p>
          <p className="mt-1 text-sm text-slate-600 dark:text-[#c7cfdd]">
            Now applied on top of the learned model for{' '}
            <span className="font-medium">
              {result.connector} / {result.account}
            </span>{' '}
            in every economic-value calculation.
          </p>
          <div className="mt-3 flex flex-wrap gap-2 text-xs">
            <Chip>+{money(result.fixed_addon, ccy, 4)} flat / txn</Chip>
            <Chip>+{result.pct_addon_bps.toFixed(3)} bps on amount</Chip>
            {result.subtotal_ex_tax != null && (
              <Chip muted>Invoice subtotal {money(result.subtotal_ex_tax, ccy)}</Chip>
            )}
            {result.txn_count != null && (
              <Chip muted>{result.txn_count.toLocaleString()} transactions</Chip>
            )}
          </div>
        </div>

        {/* Identified missing fees */}
        <div>
          <h3 className="mb-2 text-sm font-medium text-slate-800 dark:text-white">
            Fees we identified and are now applying
          </h3>
          <LineTable lines={added} ccy={ccy} showPerTxn />
        </div>

        {/* Ignored (already modeled) — the not-double-counted reassurance */}
        {ignored.length > 0 && (
          <details className="group">
            <summary className="flex cursor-pointer items-center gap-2 text-sm text-slate-500 hover:text-slate-700 dark:text-[#9ca7ba] dark:hover:text-white">
              <Info size={14} />
              {ignored.length} line{ignored.length === 1 ? '' : 's'} already captured by your
              settlement report — ignored so nothing is double-counted
            </summary>
            <div className="mt-3">
              <LineTable lines={ignored} ccy={ccy} showPerTxn={false} />
            </div>
          </details>
        )}
      </CardBody>
    </Card>
  )
}

/** Table of identified fee types. */
function LineTable({
  lines,
  ccy,
  showPerTxn,
}: {
  lines: InvoiceLineDto[]
  ccy: string
  showPerTxn: boolean
}) {
  if (lines.length === 0) {
    return (
      <p className="text-sm text-slate-400">No lines in this group.</p>
    )
  }
  return (
    <div className="overflow-x-auto rounded-lg border border-slate-200 dark:border-[#232833]">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-slate-200 text-left text-xs uppercase tracking-wide text-slate-400 dark:border-[#232833]">
            <th className="px-3 py-2 font-medium">Fee</th>
            <th className="px-3 py-2 font-medium">Category</th>
            <th className="px-3 py-2 text-right font-medium">On invoice</th>
            {showPerTxn && <th className="px-3 py-2 text-right font-medium">Per transaction</th>}
          </tr>
        </thead>
        <tbody>
          {lines.map((l, i) => (
            <tr
              key={`${l.kind}-${l.description}-${i}`}
              className="border-b border-slate-100 last:border-0 dark:border-[#1c1c23]"
            >
              <td className="px-3 py-2 capitalize text-slate-800 dark:text-[#e7ecf3]">
                {l.description}
              </td>
              <td className="px-3 py-2">
                <KindTag kind={l.kind} added={l.added} />
              </td>
              <td className="px-3 py-2 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
                {money(l.amount, ccy)}
              </td>
              {showPerTxn && (
                <td className="px-3 py-2 text-right font-medium tabular-nums text-slate-900 dark:text-white">
                  {l.per_txn !== 0 ? `${l.per_txn < 0 ? '−' : '+'}${money(Math.abs(l.per_txn), ccy, 4)}` : '—'}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function KindTag({ kind, added }: { kind: string; added: boolean }) {
  const label = KIND_LABEL[kind] ?? kind
  const cls = added
    ? 'bg-brand-500/10 text-brand-600 dark:text-brand-400'
    : 'bg-slate-100 text-slate-500 dark:bg-[#1c1c23] dark:text-[#9ca7ba]'
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-xs font-medium ${cls}`}>
      {label}
    </span>
  )
}

function Chip({ children, muted }: { children: React.ReactNode; muted?: boolean }) {
  const cls = muted
    ? 'border-slate-200 text-slate-500 dark:border-[#232833] dark:text-[#9ca7ba]'
    : 'border-brand-500/30 text-brand-600 dark:text-brand-400'
  return (
    <span className={`rounded-full border px-2.5 py-1 font-medium ${cls}`}>{children}</span>
  )
}

/** The invoice add-ons currently in effect, each removable. */
function ActiveAddons({
  addons,
  onRemove,
}: {
  addons: import('../../hooks/useCostRouting').InvoiceAddon[]
  onRemove: (connector: string) => void
}) {
  return (
    <Card>
      <CardHeader>
        <SurfaceLabel>Active invoice add-ons</SurfaceLabel>
        <h3 className="mt-2 font-medium text-slate-800 dark:text-white">
          Currently applied to routing
        </h3>
      </CardHeader>
      <CardBody className="space-y-2">
        {addons.map((a) => (
          <div
            key={a.connector}
            className="flex items-center justify-between rounded-lg border border-slate-200 px-3 py-2.5 dark:border-[#232833]"
          >
            <div className="min-w-0">
              <p className="text-sm font-medium text-slate-800 dark:text-white">{a.connector}</p>
              <p className="text-xs text-slate-500 dark:text-[#9ca7ba]">
                +{money(a.fixed_addon, a.currency, 4)} / txn · +{a.pct_addon_bps.toFixed(3)} bps
                {a.invoice_ref ? ` · invoice ${a.invoice_ref}` : ''}
              </p>
            </div>
            <button
              type="button"
              onClick={() => onRemove(a.connector)}
              className="flex shrink-0 items-center gap-1 rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs text-slate-500 hover:border-rose-300 hover:text-rose-600 dark:border-[#232833] dark:hover:border-rose-900/50"
            >
              <Trash2 size={13} />
              Remove
            </button>
          </div>
        ))}
      </CardBody>
    </Card>
  )
}
