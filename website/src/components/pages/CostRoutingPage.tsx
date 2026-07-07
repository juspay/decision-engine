import { useState } from 'react'
import { useMerchantStore } from '../../store/merchantStore'
import { CostCoverageCard } from './CostCoverageCard'
import { ConnectorCredentialsForm } from './ConnectorCredentialsForm'
import { ManualReportUpload } from './ManualReportUpload'
import { IngestionHistory } from './IngestionHistory'
import { PriceChanges } from './PriceChanges'

type IngestTab = 'automatic' | 'manual'

/**
 * Cost estimation dashboard. Layout: a full-width coverage hero (the headline metric), then a
 * two-column body — the wide ingestion-history table on the left, and a narrow action rail on the
 * right holding the two ingestion tabs (automatic webhook credentials / manual file upload). The
 * rail collapses below the history on narrow screens.
 */
export function CostRoutingPage() {
  const { merchantId } = useMerchantStore()
  const [activeTab, setActiveTab] = useState<IngestTab>('automatic')

  const tabClass = (tab: IngestTab) =>
    `px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
      activeTab === tab
        ? 'border-brand-500 text-brand-600 dark:text-brand-400'
        : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
    }`

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-900 dark:text-white">Cost Estimation</h1>
        <p className="mt-1 text-sm text-slate-500 dark:text-[#9ca7ba]">
          In-house cost estimation from settlement reports. Configure a connector, then track how
          much of your volume gets a trustworthy cost model.
        </p>
      </div>

      {!merchantId && (
        <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
          Set a merchant ID in the top bar to manage cost estimation.
        </p>
      )}

      {/* Headline coverage metric, full width. */}
      <CostCoverageCard merchantId={merchantId} />

      {/* Fee-regime changes (only renders when something moved). */}
      <PriceChanges merchantId={merchantId} />

      {/* Body: wide history table + narrow ingestion action rail. */}
      <div className="grid gap-6 lg:grid-cols-2 lg:items-start">
        <IngestionHistory merchantId={merchantId} />

        <div className="space-y-4 lg:sticky lg:top-6">
          {/* Ingestion mode tabs */}
          <div className="border-b border-slate-200 dark:border-[#1c1c23]">
            <nav className="-mb-px flex gap-1">
              <button
                type="button"
                className={tabClass('automatic')}
                onClick={() => setActiveTab('automatic')}
              >
                Automatic
              </button>
              <button
                type="button"
                className={tabClass('manual')}
                onClick={() => setActiveTab('manual')}
              >
                Manual
              </button>
            </nav>
          </div>

          {activeTab === 'automatic' && <ConnectorCredentialsForm merchantId={merchantId} />}
          {activeTab === 'manual' && <ManualReportUpload merchantId={merchantId} />}
        </div>
      </div>
    </div>
  )
}
