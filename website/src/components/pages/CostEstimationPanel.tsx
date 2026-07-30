import { useEffect } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Coins, Database, LineChart, type LucideIcon } from 'lucide-react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import * as type from '../ui/typography'
import { CoverageBreakdown } from './CostCoverageCard'
import { CostsPanel } from './CostsPanel'
import { ConnectorCredentialsForm } from './ConnectorCredentialsForm'
import { ManualReportUpload } from './ManualReportUpload'
import { InvoiceUpload } from './InvoiceUpload'
import { IngestionHistory } from './IngestionHistory'
import { PriceChanges } from './PriceChanges'
import { useCostCoverage, useIngestionHistory } from '../../hooks/useCostRouting'

/** The concerns of cost estimation, as vertical sections. */
type Section = 'ingestion' | 'data' | 'costs'
const COST_SECTIONS: readonly Section[] = ['ingestion', 'data', 'costs']
type IngestMode = 'automatic' | 'manual' | 'invoice'

interface SectionDef {
  id: Section
  icon: LucideIcon
  title: string
  blurb: string
}

/** Data ingestion's children, rendered as a nested rail under it rather than a tab row in the pane. */
const INGEST_MODES: { id: IngestMode; label: string }[] = [
  { id: 'automatic', label: 'Automatic' },
  { id: 'manual', label: 'Manual' },
  { id: 'invoice', label: 'Invoice' },
]

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
    id: 'costs',
    icon: Coins,
    title: 'Costs',
    blurb: 'Effective fees per connector — overrides, learned rates & contract baseline',
  },
]

/**
 * Cost estimation dashboard, split into the three things a merchant actually does here — get data
 * in, review what was learned, and override a fee — as a vertical section rail. One concern is shown
 * at a time, so the page stops being one long dense stack.
 */
export function CostEstimationPanel({ merchantId }: { merchantId?: string }) {
  // Active section is kept in the URL (?section=…, shared with the SR Manual tab
  // since only one SR tab is visible at a time) so a search result or shared link
  // can open Data Ingestion / Ingested Data directly. Default (costs) is omitted.
  // Legacy values (?section=overrides / =contract, from before those two panels were
  // merged into Costs) fall through to the default here, and the canonicalizer below
  // strips them from the URL — so old bookmarks still land on the merged Costs view.
  const [searchParams, setSearchParams] = useSearchParams()
  const sectionParam = searchParams.get('section')
  const section: Section = COST_SECTIONS.includes(sectionParam as Section)
    ? (sectionParam as Section)
    : 'costs'

  // Which ingestion child is open, from ?source= — so a search result can land on Data Ingestion →
  // Manual directly. Only meaningful under section=ingestion; canonicalized away everywhere else.
  const sourceParam = searchParams.get('source')
  const ingestMode: IngestMode =
    section === 'ingestion' && INGEST_MODES.some((m) => m.id === sourceParam)
      ? (sourceParam as IngestMode)
      : 'automatic'

  /**
   * Write both params at once. Doing this as two separate setters would race — each reads `prev`
   * from the same render, so the second would clobber the first's write.
   */
  const navigate = (nextSection: Section, nextMode: IngestMode = ingestMode) => {
    setSearchParams(
      (prev) => {
        const params = new URLSearchParams(prev)
        if (nextSection === 'costs') params.delete('section')
        else params.set('section', nextSection)
        // `source` is ingestion's alone, and 'automatic' is the default — neither belongs in the URL.
        if (nextSection !== 'ingestion' || nextMode === 'automatic') params.delete('source')
        else params.set('source', nextMode)
        return params
      },
      { replace: true },
    )
  }

  // Canonicalize the shared ?section=/?source= params while this panel is mounted (Cost tab): drop
  // unknown values (e.g. a leftover SR Manual section, or ?source= on a non-ingestion section) and
  // the defaults, so the URL never advertises a state the panel isn't showing.
  useEffect(() => {
    const canonicalSection = section === 'costs' ? null : section
    const canonicalSource =
      section === 'ingestion' && ingestMode !== 'automatic' ? ingestMode : null
    if (sectionParam !== canonicalSection || sourceParam !== canonicalSource) {
      navigate(section, ingestMode)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section, sectionParam, ingestMode, sourceParam])

  return (
    <div className="grid gap-6 lg:grid-cols-[220px_1fr] lg:items-start">
      <SectionRail
        merchantId={merchantId}
        active={section}
        onSelect={(s) => navigate(s)}
        ingestMode={ingestMode}
        onSelectIngestMode={(m) => navigate('ingestion', m)}
      />

      <div className="min-w-0">
        {section === 'ingestion' && <IngestionSection merchantId={merchantId} mode={ingestMode} />}
        {section === 'data' && <IngestedDataSection merchantId={merchantId} />}
        {section === 'costs' && <CostsPanel merchantId={merchantId} />}
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
  ingestMode,
  onSelectIngestMode,
}: {
  merchantId?: string
  active: Section
  onSelect: (s: Section) => void
  ingestMode: IngestMode
  onSelectIngestMode: (m: IngestMode) => void
}) {
  const { coverage } = useCostCoverage(merchantId)
  const { ingestions } = useIngestionHistory(merchantId)

  const hint: Record<Section, string | undefined> = {
    ingestion:
      ingestions.length > 0
        ? `${ingestions.length} report${ingestions.length === 1 ? '' : 's'}`
        : 'Not set up',
    // "Covered" means the router can price it — GOOD **plus** tiered. Using `good_gross_pct` here
    // reported a merchant whose big-ticket segments are capped as roughly half-covered while the
    // router was in fact pricing ~95% of their volume.
    data:
      coverage && coverage.total_clusters > 0
        ? `${coverage.priced_gross_pct.toFixed(1)}% volume covered`
        : 'No data yet',
    costs: undefined,
  }

  return (
    <nav className="flex gap-2 overflow-x-auto lg:flex-col lg:gap-1 lg:overflow-visible">
      {SECTIONS.map(({ id, icon: Icon, title, blurb }) => {
        const on = active === id
        return (
          <div key={id} className="shrink-0 lg:w-full">
            <button
              type="button"
              onClick={() => onSelect(id)}
              aria-current={on ? 'page' : undefined}
              className={`flex w-full items-start gap-3 rounded-xl border px-3 py-2.5 text-left transition-colors ${
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

            {/* Ingestion's three sources hang off it as a nested rail, revealed only while the
                section is open so the rail stays three items tall the rest of the time. The indent
                lines the divider up with the icon's centre (px-3 + half of an 18px icon). */}
            {id === 'ingestion' && on && (
              <div className="ml-[1.3125rem] mt-1 flex flex-col gap-0.5 border-l border-slate-200 pl-2 dark:border-[#2a3344]">
                {INGEST_MODES.map((m) => {
                  const sub = ingestMode === m.id
                  return (
                    <button
                      key={m.id}
                      type="button"
                      onClick={() => onSelectIngestMode(m.id)}
                      aria-current={sub ? 'true' : undefined}
                      className={`rounded-lg px-2.5 py-1.5 text-left text-sm transition-colors ${
                        sub
                          ? 'bg-brand-500/10 font-medium text-brand-600 dark:text-brand-400'
                          : 'text-slate-500 hover:bg-slate-50 hover:text-slate-700 dark:text-[#9ca7ba] dark:hover:bg-[#141923] dark:hover:text-white'
                      }`}
                    >
                      {m.label}
                    </button>
                  )
                })}
              </div>
            )}
          </div>
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

/** 2 — Ingested data: what the reports taught us — coverage, and each report's fitted segments
 * (expand a row in the history to see that report's per-segment fees). */
function IngestedDataSection({ merchantId }: { merchantId?: string }) {
  return (
    <div className="space-y-6">
      <SectionHeading
        title="Ingested data"
        subtitle="What your settlement reports taught us: how much volume we can cost accurately, and — inside each report below — the fee learned for every segment."
      />

      {/* "Clusters whose price moved" is hidden for now. Its query (`detect.rs CHANGES_SQL`) ranks
          each cluster's two most recent fits with a `row_number()` partition that omits
          `card_product`, so two card products under one display key can be ranked rn=1/rn=2 *within
          a single snapshot* — reporting a standing rate difference between two card programs as a
          fresh price change on every refit. Re-enable once that partition covers the full fit key. */}
      {false && <PriceChanges merchantId={merchantId} />}

      <Card>
        <CardHeader>
          <h3 className={type.heading}>Cost model coverage</h3>
          <p className={`mt-1 ${type.subheading}`}>How much volume we can cost accurately.</p>
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
function IngestionSection({ merchantId, mode }: { merchantId?: string; mode: IngestMode }) {
  return (
    <div>
      <SectionHeading
        title="Data ingestion"
        subtitle="Connect a settlement source so reports flow in automatically, or upload a report file manually. Add the monthly invoice to also recover the fees the report can't carry."
      />
      {mode === 'automatic' && <ConnectorCredentialsForm merchantId={merchantId} />}
      {mode === 'manual' && <ManualReportUpload merchantId={merchantId} />}
      {mode === 'invoice' && <InvoiceUpload merchantId={merchantId} />}
    </div>
  )
}
