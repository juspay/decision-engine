import { useState } from 'react'
import { Database, LineChart, SlidersHorizontal, type LucideIcon } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { CoverageBreakdown } from './CostCoverageCard'
import { ConnectorsPanel } from './ConnectorsPanel'
import { ConnectorCredentialsForm } from './ConnectorCredentialsForm'
import { ManualReportUpload } from './ManualReportUpload'
import { IngestionHistory } from './IngestionHistory'
import { PriceChanges } from './PriceChanges'
import { useCostCoverage, useIngestionHistory } from '../../hooks/useCostRouting'

/** The three concerns of cost estimation, as vertical sections. */
type Section = 'ingestion' | 'data' | 'overrides'
type IngestMode = 'automatic' | 'manual'

interface SectionDef {
  id: Section
  icon: LucideIcon
  title: string
  blurb: string
}

const SECTIONS: SectionDef[] = [
  {
    id: 'ingestion',
    icon: Database,
    title: 'Data ingestion',
    blurb: 'Connect a settlement source or upload a report',
  },
  {
    id: 'data',
    icon: LineChart,
    title: 'Ingested data',
    blurb: 'What we learned — coverage & history',
  },
  {
    id: 'overrides',
    icon: SlidersHorizontal,
    title: 'Manual overrides',
    blurb: 'Set your own blended fee per connector',
  },
]

/**
 * Cost estimation dashboard, split into the three things a merchant actually does here — get data
 * in, review what was learned, and override a fee — as a vertical section rail. One concern is shown
 * at a time, so the page stops being one long dense stack.
 */
export function CostEstimationPanel({ merchantId }: { merchantId?: string }) {
  const [section, setSection] = useState<Section>('overrides')

  return (
    <div className="grid gap-6 lg:grid-cols-[220px_1fr] lg:items-start">
      <SectionRail
        merchantId={merchantId}
        active={section}
        onSelect={setSection}
      />

      <div className="min-w-0">
        {section === 'ingestion' && <IngestionSection merchantId={merchantId} />}
        {section === 'data' && <IngestedDataSection merchantId={merchantId} />}
        {section === 'overrides' && <OverridesSection merchantId={merchantId} />}
      </div>
    </div>
  )
}

/** Left rail: the three sections, each with a one-line "what is this". A live status hint sits under
 * the label so a merchant can see state (coverage %, report count) without opening the section. */
function SectionRail({
  merchantId,
  active,
  onSelect,
}: {
  merchantId?: string
  active: Section
  onSelect: (s: Section) => void
}) {
  const { coverage } = useCostCoverage(merchantId)
  const { ingestions } = useIngestionHistory(merchantId)

  const hint: Record<Section, string | undefined> = {
    ingestion:
      ingestions.length > 0
        ? `${ingestions.length} report${ingestions.length === 1 ? '' : 's'}`
        : 'Not set up',
    data:
      coverage && coverage.total_clusters > 0
        ? ``
        : 'No data yet',
    overrides: undefined,
  }

  return (
    <nav className="flex gap-2 overflow-x-auto lg:flex-col lg:gap-1 lg:overflow-visible">
      {SECTIONS.map(({ id, icon: Icon, title, blurb }) => {
        const on = active === id
        return (
          <button
            key={id}
            type="button"
            onClick={() => onSelect(id)}
            aria-current={on ? 'page' : undefined}
            className={`flex shrink-0 items-start gap-3 rounded-xl border px-3 py-2.5 text-left transition-colors lg:w-full ${
              on
                ? 'border-brand-500/40 bg-brand-500/8 text-slate-900 dark:text-white'
                : 'border-transparent text-slate-600 hover:bg-slate-50 dark:text-[#9ca7ba] dark:hover:bg-[#141923]'
            }`}
          >
            <Icon
              size={18}
              className={`mt-0.5 shrink-0 ${on ? 'text-brand-500' : 'text-slate-400'}`}
            />
            <span className="min-w-0">
              <span className="block text-sm font-medium">{title}</span>
              <span className="mt-0.5 hidden text-xs text-slate-400 lg:block">
                {hint[id] ?? blurb}
              </span>
            </span>
          </button>
        )
      })}
    </nav>
  )
}

/** Section header shared across the three panes. */
function SectionHeading({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="mb-5">
      <h2 className="text-lg font-semibold text-slate-900 dark:text-white">{title}</h2>
      <p className="mt-1 text-sm text-slate-500 dark:text-[#9ca7ba]">{subtitle}</p>
    </div>
  )
}

/** 3 — Manual overrides: the primary action. Two levels — a surgical per-segment fee on the
 * highest-traffic clusters, and a blanket per-connector fee. Precedence: segment > connector. */
function OverridesSection({ merchantId }: { merchantId?: string }) {
  return (
    <div className="space-y-6">
      <SectionHeading
        title="Manual overrides"
        subtitle="Set your own fee, and we'll use it instead of the learned model in every economic-value calculation. Set a connector-wide fee, or expand a connector to override its individual segments — a segment fee beats the connector fee beats the model."
      />

      <ConnectorsPanel merchantId={merchantId} />
    </div>
  )
}

/** 2 — Ingested data: what the reports taught us — coverage, and each report's fitted segments
 * (expand a row in the history to see that report's per-segment fees). */
function IngestedDataSection({ merchantId }: { merchantId?: string }) {
  return (
    <div className="space-y-6">
      <SectionHeading
        title="Ingested data"
        subtitle="What your settlement reports taught us: how much volume we can cost accurately, and — inside each report below — the fee learned for every segment."
      />

      <PriceChanges merchantId={merchantId} />

      <Card>
        <CardHeader>
          <SurfaceLabel>Cost model coverage</SurfaceLabel>
          <h3 className="mt-2 font-medium text-slate-800 dark:text-white">
            How much volume we can cost accurately
          </h3>
        </CardHeader>
        <CardBody>
          <CoverageBreakdown merchantId={merchantId} />
        </CardBody>
      </Card>

      <IngestionHistory merchantId={merchantId} />
    </div>
  )
}

/** 1 — Data ingestion: get settlement data in, automatically or by upload. */
function IngestionSection({ merchantId }: { merchantId?: string }) {
  const [mode, setMode] = useState<IngestMode>('automatic')

  const tabClass = (m: IngestMode) =>
    `px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
      mode === m
        ? 'border-brand-500 text-brand-600 dark:text-brand-400'
        : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
    }`

  return (
    <div>
      <SectionHeading
        title="Data ingestion"
        subtitle="Connect a settlement source so reports flow in automatically, or upload a report file manually."
      />
      <div className="mb-5 border-b border-slate-200 dark:border-[#1c1c23]">
        <nav className="-mb-px flex gap-1">
          <button type="button" className={tabClass('automatic')} onClick={() => setMode('automatic')}>
            Automatic
          </button>
          <button type="button" className={tabClass('manual')} onClick={() => setMode('manual')}>
            Manual
          </button>
        </nav>
      </div>
      {mode === 'automatic' ? (
        <ConnectorCredentialsForm merchantId={merchantId} />
      ) : (
        <ManualReportUpload merchantId={merchantId} />
      )}
    </div>
  )
}
