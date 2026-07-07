import { Coins } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useCostCoverage, type CoverageSummary } from '../../hooks/useCostRouting'

/**
 * Headline health card. A one-glance money-weighted coverage number, then a per-verdict breakdown
 * table — the table makes the key insight visible: GOOD covers most *transactions* but far less
 * *volume*, because the big-ticket segments (high avg ticket) fall into THIN / NON_LINEAR. Shares
 * its SWR key with the ingestion flow, so a new report refreshes it automatically.
 */
export function CostCoverageCard({ merchantId }: { merchantId?: string }) {
  const { coverage, error, isLoading } = useCostCoverage(merchantId)
  const hasData = !!coverage && coverage.total_clusters > 0

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <Coins size={16} className="text-brand-500" />
          <div>
            <SurfaceLabel>Cost model coverage</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              How much volume we can cost accurately
            </h2>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-5">
        {merchantId && isLoading ? (
          <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
            <Spinner size={16} />
            Loading coverage...
          </div>
        ) : hasData && coverage ? (
          <>
            <VerdictTable coverage={coverage} />

            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-500 dark:text-[#9ca7ba]">
              <span>
                Model accuracy:{' '}
                <span className="font-semibold text-slate-700 dark:text-[#c7cfdd]">
                  ±{coverage.bps_rmse_p50.toFixed(1)} bps
                </span>{' '}
                median · ±{coverage.bps_rmse_p90.toFixed(1)} bps p90
              </span>
              <span aria-hidden>·</span>
              <span>{coverage.good_clusters.toLocaleString()} active cost models</span>
              {coverage.report_date && (
                <>
                  <span aria-hidden>·</span>
                  <span>as of {coverage.report_date}</span>
                </>
              )}
            </div>
          </>
        ) : (
          <p className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-sm text-slate-500 dark:border-[#232833] dark:bg-[#0b1017]">
            No fitted cost models yet. They appear after the first settlement report is ingested.
          </p>
        )}
        <ErrorMessage
          error={
            error instanceof Error ? error.message : error ? 'Failed to load coverage' : null
          }
        />
      </CardBody>
    </Card>
  )
}

function VerdictTable({ coverage }: { coverage: CoverageSummary }) {
  const rows = [
    {
      verdict: 'GOOD',
      note: 'trustworthy cost model',
      dot: 'bg-emerald-500',
      txns: coverage.good_txns,
      gross: coverage.good_gross,
    },
    {
      verdict: 'THIN',
      note: 'too few txns — safe fallback',
      dot: 'bg-slate-400',
      txns: coverage.thin_txns,
      gross: coverage.thin_gross,
    },
    {
      verdict: 'NON_LINEAR',
      note: 'non-linear cost — needs attention',
      dot: 'bg-amber-500',
      txns: coverage.non_linear_txns,
      gross: coverage.non_linear_gross,
    },
  ]
  const txnPct = (n: number) => (coverage.total_txns > 0 ? (n / coverage.total_txns) * 100 : 0)
  const volPct = (v: number) => (coverage.total_gross > 0 ? (v / coverage.total_gross) * 100 : 0)

  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[520px] text-left text-sm">
        <thead>
          <tr className="border-b border-slate-200 text-xs uppercase tracking-[0.14em] text-slate-400 dark:border-[#232833]">
            <th className="py-2 pr-3 font-semibold">Verdict</th>
            <th className="py-2 pr-3 text-right font-semibold">Txns</th>
            <th className="py-2 pr-3 text-right font-semibold">Txn %</th>
            <th className="py-2 pr-3 text-right font-semibold">Volume</th>
            <th className="py-2 pr-3 text-right font-semibold">Vol %</th>
            <th className="py-2 pr-3 text-right font-semibold">Avg ticket</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr
              key={r.verdict}
              className="border-b border-slate-100 last:border-0 dark:border-[#1c1c23]"
            >
              <td className="py-2 pr-3">
                <span className="flex items-center gap-2">
                  <span className={`h-2 w-2 rounded-full ${r.dot}`} />
                  <span className="font-medium text-slate-700 dark:text-[#c7cfdd]">{r.verdict}</span>
                </span>
                <span className="ml-4 text-xs text-slate-400">{r.note}</span>
              </td>
              <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
                {r.txns.toLocaleString()}
              </td>
              <td className="py-2 pr-3 text-right tabular-nums text-slate-500 dark:text-[#9ca7ba]">
                {txnPct(r.txns).toFixed(1)}%
              </td>
              <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
                {formatCompact(r.gross)}
              </td>
              <td className="py-2 pr-3 text-right tabular-nums text-slate-500 dark:text-[#9ca7ba]">
                {volPct(r.gross).toFixed(1)}%
              </td>
              <td className="py-2 pr-3 text-right tabular-nums text-slate-600 dark:text-[#c7cfdd]">
                {r.txns > 0 ? formatMoney(r.gross / r.txns) : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function formatCompact(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}K`
  return n.toFixed(0)
}

function formatMoney(n: number): string {
  return n.toLocaleString(undefined, { maximumFractionDigits: 0 })
}
