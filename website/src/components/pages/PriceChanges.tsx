import { TrendingUp } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { usePriceChanges, type PriceChange } from '../../hooks/useCostRouting'

/**
 * Fee-regime changes: clusters whose fitted price stepped between their two most recent snapshots
 * — e.g. a connector moving 2.40% + €0.22 → 2.50% + €0.25. The two-part model lets us show *which*
 * part moved (percentage vs flat fee). Hidden entirely when nothing changed.
 */
export function PriceChanges({ merchantId }: { merchantId?: string }) {
  const { changes } = usePriceChanges(merchantId)
  if (!merchantId || changes.length === 0) return null

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <TrendingUp size={16} className="text-amber-500" />
          <div>
            <SurfaceLabel>Fee changes detected</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              Clusters whose price moved
            </h2>
          </div>
        </div>
      </CardHeader>
      <CardBody>
        <div className="overflow-x-auto">
          <table className="w-full min-w-[560px] text-left text-sm">
            <thead>
              <tr className="border-b border-slate-200 text-xs uppercase tracking-[0.14em] text-slate-400 dark:border-[#232833]">
                <th className="py-2 pr-3 font-semibold">Cluster</th>
                <th className="py-2 pr-3 text-right font-semibold">Percentage</th>
                <th className="py-2 pr-3 text-right font-semibold">Flat fee</th>
                <th className="py-2 pr-3 text-right font-semibold">Detected</th>
              </tr>
            </thead>
            <tbody>
              {changes.map((c, i) => (
                <tr
                  key={`${clusterLabel(c)}-${i}`}
                  className="border-b border-slate-100 last:border-0 dark:border-[#1c1c23]"
                >
                  <td className="py-2 pr-3 text-slate-600 dark:text-[#c7cfdd]">{clusterLabel(c)}</td>
                  <td className="py-2 pr-3 text-right tabular-nums">
                    <Delta from={pct(c.old_pct_bps)} to={pct(c.new_pct_bps)} up={c.new_pct_bps > c.old_pct_bps} same={c.new_pct_bps === c.old_pct_bps} />
                  </td>
                  <td className="py-2 pr-3 text-right tabular-nums">
                    <Delta from={money(c.old_fixed)} to={money(c.new_fixed)} up={c.new_fixed > c.old_fixed} same={c.new_fixed === c.old_fixed} />
                  </td>
                  <td className="py-2 pr-3 text-right text-slate-500 dark:text-[#9ca7ba]">
                    {c.changed_on}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </CardBody>
    </Card>
  )
}

function Delta({ from, to, up, same }: { from: string; to: string; up: boolean; same: boolean }) {
  if (same) return <span className="text-slate-400">{to}</span>
  return (
    <span>
      <span className="text-slate-400">{from}</span>
      <span className="mx-1 text-slate-400">→</span>
      <span className={up ? 'font-medium text-amber-500' : 'font-medium text-emerald-500'}>{to}</span>
    </span>
  )
}

function clusterLabel(c: PriceChange): string {
  return [c.connector, c.card_network, c.funding, c.currency, c.issuer_country, c.ic_category]
    .filter(Boolean)
    .join(' · ')
}

function pct(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`
}

function money(v: number): string {
  return v.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}
