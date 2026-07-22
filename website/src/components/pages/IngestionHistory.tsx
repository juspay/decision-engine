import { useState } from 'react'
import { ChevronDown, ChevronRight, History, Trash2 } from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import * as type from '../ui/typography'
import { Spinner } from '../ui/Spinner'
import { ClustersPanel } from './ClustersPanel'
import {
  deleteIngestion,
  useCostCoverage,
  useIngestionHistory,
  type IngestionDto,
} from '../../hooks/useCostRouting'

/**
 * Ingestion history: every settlement report ingested (manual upload or webhook), newest first —
 * period, row count, currencies/countries, and fit outcome. In-flight jobs appear here too with a
 * live row count (the hook polls while any job is processing). Rows expand to show the full
 * currency/country lists and volume.
 */
export function IngestionHistory({ merchantId }: { merchantId?: string }) {
  const { ingestions, isLoading, mutate } = useIngestionHistory(merchantId)
  const { mutate: mutateCoverage } = useCostCoverage(merchantId)
  const [expanded, setExpanded] = useState<Set<number>>(new Set())
  const [deletingId, setDeletingId] = useState<number | null>(null)

  function toggle(id: number) {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  async function handleDelete(job: IngestionDto) {
    const label =
      job.period_start && job.period_end
        ? `${job.connector} / ${job.account} (${job.period_start} → ${job.period_end})`
        : `${job.connector} / ${job.account}`
    if (
      !merchantId ||
      !window.confirm(
        `Delete this ingestion?\n\n${label}\n\nThis removes its fitted cost models and staged rows. ` +
          `Coverage reverts to the previous snapshot. This cannot be undone.`,
      )
    ) {
      return
    }
    setDeletingId(job.id)
    try {
      await deleteIngestion(merchantId, job.id)
      await Promise.all([mutate(), mutateCoverage()])
    } catch (e) {
      window.alert(e instanceof Error ? e.message : 'Failed to delete ingestion')
    } finally {
      setDeletingId(null)
    }
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <History size={16} className="text-brand-500" />
          <div>
            <h2 className={type.heading}>Reports ingested</h2>
          </div>
        </div>
      </CardHeader>
      <CardBody>
        {merchantId && isLoading ? (
          <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
            <Spinner size={16} />
            Loading history...
          </div>
        ) : ingestions.length === 0 ? (
          <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
            No ingestions yet. Upload a report or configure a webhook connector.
          </p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[720px] text-left text-sm">
              <thead>
                <tr className="border-b border-slate-200 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] dark:border-[#232833]">
                  <th className="py-2 pr-2 font-semibold" />
                  <th className="py-2 pr-3 font-semibold">Ingested</th>
                  <th className="py-2 pr-3 font-semibold">Source</th>
                  <th className="py-2 pr-3 font-semibold">Connector · Account</th>
                  <th className="py-2 pr-3 font-semibold">Period</th>
                  <th className="py-2 pr-3 font-semibold">Status</th>
                  <th className="py-2 pr-3 text-right font-semibold">Rows</th>
                  <th className="py-2 pr-3 text-right font-semibold">Ccy</th>
                  <th className="py-2 pr-3 text-right font-semibold">Ctry</th>
                  <th className="py-2 pr-3 text-right font-semibold">GOOD</th>
                  <th className="py-2 pr-2 font-semibold" />
                </tr>
              </thead>
              <tbody>
                {ingestions.map((job) => (
                  <Row
                    key={job.id}
                    merchantId={merchantId}
                    job={job}
                    open={expanded.has(job.id)}
                    onToggle={() => toggle(job.id)}
                    onDelete={() => handleDelete(job)}
                    deleting={deletingId === job.id}
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </CardBody>
    </Card>
  )
}

function Row({
  merchantId,
  job,
  open,
  onToggle,
  onDelete,
  deleting,
}: {
  merchantId?: string
  job: IngestionDto
  open: boolean
  onToggle: () => void
  onDelete: () => void
  deleting: boolean
}) {
  const period =
    job.period_start && job.period_end ? `${job.period_start} → ${job.period_end}` : '—'
  return (
    <>
      <tr
        className="cursor-pointer border-b border-slate-100 hover:bg-slate-50 dark:border-[#1c1c23] dark:hover:bg-[#0b1017]"
        onClick={onToggle}
      >
        <td className="py-2 pr-2 text-slate-400">
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </td>
        <td className="py-2 pr-3 text-slate-600 dark:text-[#c7cfdd]">{formatTs(job.created_at)}</td>
        <td className="py-2 pr-3">
          <span className="rounded-full bg-slate-100 px-2 py-0.5 text-xs text-slate-600 dark:bg-[#1c1c23] dark:text-[#9ca7ba]">
            {job.source}
          </span>
        </td>
        <td className="py-2 pr-3 text-slate-600 dark:text-[#c7cfdd]">
          {job.connector} · {job.account}
        </td>
        <td className="py-2 pr-3 text-slate-500 dark:text-[#9ca7ba]">{period}</td>
        <td className="py-2 pr-3">
          <StatusBadge status={job.status} />
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {job.staged_rows.toLocaleString()}
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {job.currency_count}
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {job.country_count}
        </td>
        <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
          {job.status === 'completed' ? `${job.good_clusters}/${job.total_clusters}` : '—'}
        </td>
        <td className="py-2 pr-2 text-right">
          {job.status !== 'processing' && (
            <button
              type="button"
              title="Delete ingestion"
              disabled={deleting}
              onClick={(e) => {
                e.stopPropagation()
                onDelete()
              }}
              className="rounded p-1 text-slate-400 hover:bg-red-50 hover:text-red-500 disabled:opacity-50 dark:hover:bg-red-950/30"
            >
              {deleting ? <Spinner size={14} /> : <Trash2 size={14} />}
            </button>
          )}
        </td>
      </tr>
      {open && (
        <tr className="border-b border-slate-100 bg-slate-50/60 dark:border-[#1c1c23] dark:bg-[#0b1017]">
          <td />
          <td colSpan={10} className="py-3 pr-3">
            <dl className="grid grid-cols-1 gap-x-8 gap-y-2 sm:grid-cols-2">
              <Detail label="Settled volume" value={formatCompact(job.total_gross)} />
              <Detail
                label="Report date"
                value={job.report_date ?? '—'}
              />
              <Detail
                label={`Currencies (${job.currency_count})`}
                value={job.currencies.length ? job.currencies.join(', ') : '—'}
              />
              <Detail
                label={`Countries (${job.country_count})`}
                value={job.countries.length ? job.countries.join(', ') : '—'}
              />
              {job.last_error && (
                <Detail label="Error" value={job.last_error} error />
              )}
            </dl>

            {/* This report's fitted segments and the fee we learned for each. */}
            {job.status === 'completed' && job.report_date && (
              <div className="mt-4 border-t border-slate-200 pt-3 dark:border-[#232833]">
                <p className="mb-2 text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">
                  Fitted segments (top by txns)
                </p>
                <ClustersPanel
                  merchantId={merchantId}
                  editable={false}
                  limit={20}
                  defaultSort="n"
                  scope={{
                    connector: job.connector,
                    account: job.account,
                    reportDate: job.report_date,
                  }}
                />
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  )
}

function Detail({ label, value, error }: { label: string; value: string; error?: boolean }) {
  return (
    <div>
      <dt className="text-[12px] font-medium text-slate-500 dark:text-[#8d96aa]">{label}</dt>
      <dd
        className={`mt-0.5 break-words text-sm ${
          error ? 'text-red-500' : 'text-slate-600 dark:text-[#c7cfdd]'
        }`}
      >
        {value}
      </dd>
    </div>
  )
}

function StatusBadge({ status }: { status: string }) {
  const styles: Record<string, string> = {
    completed: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-400',
    processing: 'bg-amber-100 text-amber-700 dark:bg-amber-950/40 dark:text-amber-400 animate-pulse',
    failed: 'bg-red-100 text-red-700 dark:bg-red-950/40 dark:text-red-400',
    pending: 'bg-slate-100 text-slate-600 dark:bg-[#1c1c23] dark:text-[#9ca7ba]',
  }
  const cls = styles[status] ?? styles.pending
  return <span className={`rounded-full px-2 py-0.5 text-xs ${cls}`}>{status}</span>
}

function formatTs(iso: string): string {
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString()
}

function formatCompact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`
  return n.toFixed(0)
}
