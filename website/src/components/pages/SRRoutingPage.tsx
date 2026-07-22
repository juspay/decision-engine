import useSWR from 'swr'
import { useForm, useFieldArray } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useEffect, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { useAuthStore } from '../../store/authStore'
import { apiPost, fetcher } from '../../lib/api'
import { PAYMENT_METHOD_TYPES, PAYMENT_METHODS } from '../../lib/constants'
import {
  Plus,
  Trash2,
  Eye,
  PowerOff,
  Info,
  Layers,
  SlidersHorizontal,
  Ban,
  type LucideIcon,
} from 'lucide-react'
import * as type from '../ui/typography'
import { useMerchantFeatures, type KnownFeature } from '../../hooks/useMerchantFeatures'
import { BucketHedgingTuner } from './BucketHedgingTuner'
import { CostEstimationPanel } from './CostEstimationPanel'

/** One input treatment for the config forms, so fields don't drift apart field by field. */
const configInputClass =
  'w-full rounded-lg border border-slate-200 bg-transparent px-3 py-2 text-sm ' +
  'focus:outline-none focus:ring-1 focus:ring-brand-500 dark:border-[#222226]'

// Ensures a stored value is always selectable in a dropdown, even when it isn't in the known
// option list (e.g. auto-calibration writes the casing live txns use, "CARD"/"CREDIT", while the
// option lists are lowercase). Prepends the value so the <select> renders it instead of going blank.
function optionsWithValue(options: readonly string[], value: string): string[] {
  return value && !options.includes(value) ? [value, ...options] : [...options]
}

// Dimensions a merchant can split SR scoring clusters on (must match backend ELIGIBLE_DIMENSIONS).
const ELIGIBLE_SR_DIMENSIONS: { key: string; label: string; note?: string }[] = [
  { key: 'card_network', label: 'Card network' },
  { key: 'currency', label: 'Currency' },
  { key: 'country', label: 'Country' },
  { key: 'auth_type', label: 'Auth type' },
  { key: 'card_is_in', label: 'Card BIN', note: 'High cardinality — not auto-calibrated' },
]
// Low-cardinality dims Autopilot auto-selects when enabled (BIN excluded to avoid a score-key explosion).
const AUTOPILOT_SR_DIMENSIONS = ['card_network', 'currency', 'country', 'auth_type']

interface SrDimensionResponse {
  paymentInfo?: { fields?: string[] | null; udfs?: number[] | null }
}

// Enable all low-cardinality SR dimensions for a merchant (union with whatever is already set;
// preserves udfs). Used when Autopilot is turned on.
async function enableAutopilotSrDimensions(merchantId: string): Promise<void> {
  let fields: string[] = []
  let udfs: number[] = []
  try {
    const cur = await fetcher<SrDimensionResponse>(`/config-sr-dimension/${merchantId}`)
    fields = cur?.paymentInfo?.fields ?? []
    udfs = cur?.paymentInfo?.udfs ?? []
  } catch {
    // No config yet — start fresh.
  }
  const merged = Array.from(new Set([...fields, ...AUTOPILOT_SR_DIMENSIONS]))
  await apiPost('/config-sr-dimension', {
    merchant_id: merchantId,
    paymentInfo: { udfs, fields: merged },
  })
}

// ---- Schema ----
// Optional cluster dimensions — empty string normalizes to null so the decider treats them as
// "any" (a stored "" would never match a real value).
const optionalDim = z.preprocess(
  (v) => (v === '' || v === null || v === undefined ? null : v),
  z.string().nullable()
)

const subLevelSchema = z.object({
  paymentMethodType: z.string().min(1),
  paymentMethod: z.string().min(1),
  cardNetwork: optionalDim,
  currency: optionalDim,
  country: optionalDim,
  authType: optionalDim,
  // Provenance passthrough: "autopilot" for auto-calibrated rows, null for human-authored.
  source: optionalDim,
  bucketSize: z.coerce.number().int().positive(),
  hedgingPercent: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().nullable()
  ),
  latencyThreshold: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().nullable()
  ),
})

const srFormSchema = z.object({
  defaultBucketSize: z.coerce.number().int().positive(),
  defaultSuccessRate: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().min(0).max(1).nullable()
  ),
  defaultLatencyThreshold: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().nullable()
  ),
  defaultHedgingPercent: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().nullable()
  ),
  margin: z.preprocess(
    (v) => (v === '' || v === null ? null : Number(v)),
    z.number().min(0).max(100).nullable()
  ),
  subLevelInputConfig: z.array(subLevelSchema),
})

type SRForm = z.infer<typeof srFormSchema>

interface SRConfigResponse {
  merchant_id: string
  modified_at?: string
  config: {
    type: string
    data: {
      defaultBucketSize: number
      defaultSuccessRate: number | null
      defaultLatencyThreshold: number | null
      defaultHedgingPercent: number | null
      margin: number | null
      subLevelInputConfig: {
        paymentMethodType: string
        paymentMethod: string
        cardNetwork?: string | null
        currency?: string | null
        country?: string | null
        authType?: string | null
        source?: string | null
        bucketSize: number
        hedgingPercent: number | null
        latencyThreshold: number | null
      }[] | null
    }
  }
}

interface EliminationConfigResponse {
  merchant_id: string
  modified_at?: string
  config: {
    type: string
    data: {
      threshold: number
      txnLatency: { gatewayLatency: number | null } | null
    }
  }
}

function CurrentConfigDetails({ config }: { config: SRConfigResponse['config'] }) {
  return (
    <div className="text-xs text-slate-600 dark:text-[#b2bdd1] space-y-4">
      <div className="border-b border-slate-200 pb-3 dark:border-[#222226]">
        <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Default settings</h3>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <div>
            <span className="text-slate-500">Bucket size</span>
            <p className="font-medium">{config.data.defaultBucketSize}</p>
          </div>
          <div>
            <span className="text-slate-500">Success rate</span>
            <p className="font-medium">{config.data.defaultSuccessRate ?? 'Not set'}</p>
          </div>
          <div>
            <span className="text-slate-500">Hedging %</span>
            <p className="font-medium">{config.data.defaultHedgingPercent ?? 'Not set'}</p>
          </div>
          <div>
            <span className="text-slate-500">Feedback latency window</span>
            <p className="font-medium">{config.data.defaultLatencyThreshold ?? 'Not set'} s</p>
          </div>
          <div>
            <span className="text-slate-500">Margin</span>
            <p className="font-medium">{config.data.margin != null ? `${config.data.margin * 100}%` : 'Not set (100%)'}</p>
          </div>
        </div>
      </div>

      {config.data.subLevelInputConfig && config.data.subLevelInputConfig.length > 0 ? (
        <div>
          <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Sub-level configurations</h3>
          <div className="space-y-2">
            {config.data.subLevelInputConfig.map((subConfig, idx) => (
              <div key={idx} className="bg-slate-50 dark:bg-[#151518] rounded-lg p-3">
                <div className="grid grid-cols-2 gap-2 text-xs md:grid-cols-5">
                  <div>
                    <span className="text-slate-500">Payment Type:</span>
                    <p className="font-medium capitalize">{subConfig.paymentMethodType}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Payment Method:</span>
                    <p className="font-medium">{subConfig.paymentMethod}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Bucket size</span>
                    <p className="font-medium">{subConfig.bucketSize}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Hedging %</span>
                    <p className="font-medium">{subConfig.hedgingPercent ?? 'Default'}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Feedback latency window</span>
                    <p className="font-medium">{subConfig.latencyThreshold ?? 'Default'} s</p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <div className="border-t border-slate-200 pt-3 dark:border-[#222226]">
        <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Raw Configuration (JSON)</h3>
        <pre className="max-h-64 overflow-auto rounded-lg border border-slate-200/80 bg-slate-50/90 p-3 font-mono text-xs leading-6 text-slate-800 shadow-[inset_0_1px_0_rgba(255,255,255,0.75),0_16px_30px_-28px_rgba(15,23,42,0.18)] dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef] dark:shadow-none">
          {JSON.stringify(config, null, 2)}
        </pre>
      </div>
    </div>
  )
}

type SRTab = 'autopilot' | 'manual' | 'flags' | 'cost'
const SR_TABS: readonly SRTab[] = ['autopilot', 'manual', 'flags', 'cost']
/** Tabs laid out as a left rail + content pane, which need the full page width to breathe. */
const WIDE_TABS: readonly SRTab[] = ['manual', 'cost']

type ManualSection = 'scoring' | 'elimination' | 'dimensions'
const MANUAL_SECTIONS: readonly ManualSection[] = ['scoring', 'elimination', 'dimensions']

/** Manual config's three concerns, as a vertical rail — mirrors the Cost tab's section rail. */
const MANUAL_SECTION_DEFS: { id: ManualSection; icon: LucideIcon; title: string; blurb: string }[] =
  [
    {
      id: 'scoring',
      icon: SlidersHorizontal,
      title: 'Scoring defaults',
      blurb: 'Bucket size, hedging & per-payment-type overrides',
    },
    {
      id: 'elimination',
      icon: Ban,
      title: 'Elimination',
      blurb: 'Drop a PSP whose auth rate falls too low',
    },
    {
      id: 'dimensions',
      icon: Layers,
      title: 'SR Dimensions',
      blurb: 'Attributes scoring splits clusters on',
    },
  ]

export function SRRoutingPage() {
  // Same merchant resolution as OverviewPage/RoutingHubPage — this page must
  // never disagree with the Overview setup checklist about what is configured.
  const selectedMerchantId = useMerchantStore((state) => state.merchantId)
  const authMerchantId = useAuthStore((state) => state.user?.merchantId || '')
  const merchantId = selectedMerchantId || authMerchantId
  // Active tab is kept in the URL (?tab=…) so a reload or shared link reopens it directly.
  // Unknown/absent values fall back to Autopilot, and the default is left out of the URL.
  const [searchParams, setSearchParams] = useSearchParams()
  const tabParam = searchParams.get('tab')
  const activeTab: SRTab = SR_TABS.includes(tabParam as SRTab) ? (tabParam as SRTab) : 'autopilot'
  const setActiveTab = (tab: SRTab) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev)
        if (tab === 'autopilot') next.delete('tab')
        else next.set('tab', tab)
        return next
      },
      { replace: true },
    )
  }
  // The Manual sub-section is also kept in the URL (?section=…) so a search
  // result or shared link can jump straight to Elimination / SR Dimensions.
  const sectionParam = searchParams.get('section')
  const manualTab: ManualSection = MANUAL_SECTIONS.includes(sectionParam as ManualSection)
    ? (sectionParam as ManualSection)
    : 'scoring'
  const setManualTab = (section: ManualSection) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev)
        if (section === 'scoring') next.delete('section')
        else next.set('section', section)
        return next
      },
      { replace: true },
    )
  }
  // The ?section= param is shared with the Cost tab, which has a disjoint set of
  // valid values. When the Manual tab is active, canonicalize it: drop unknown
  // values (e.g. a leftover cost section after switching tabs) and the default
  // (scoring) so the URL never advertises a section the Manual UI isn't showing.
  useEffect(() => {
    if (activeTab !== 'manual') return
    const canonical = manualTab === 'scoring' ? null : manualTab
    if (sectionParam !== canonical) setManualTab(manualTab)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeTab, manualTab, sectionParam])
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)
  const [showCurrentConfig, setShowCurrentConfig] = useState(false)
  const [showSubLevelOverrides, setShowSubLevelOverrides] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const [lastSavedAt, setLastSavedAt] = useState<string | null>(null)

  // Shares the SWR key used by OverviewPage/RoutingHubPage so all three surfaces
  // read (and invalidate) the same cached config.
  const { data: existing, isLoading, mutate } = useSWR<SRConfigResponse>(
    merchantId ? ['/rule/get', 'successRate', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false, revalidateOnFocus: false }
  )

  const {
    register,
    control,
    handleSubmit,
    reset,
    watch,
    formState: { errors },
  } = useForm<SRForm>({
    resolver: zodResolver(srFormSchema),
    defaultValues: {
      defaultBucketSize: 200,
      defaultSuccessRate: 0.5,
      defaultLatencyThreshold: null,
      defaultHedgingPercent: null,
      margin: null,
      subLevelInputConfig: [],
    },
  })

  // Pre-fill form from fetched config
  useEffect(() => {
    if (existing?.config?.data) {
      const d = existing.config.data
      // Normalize so the optional dimension fields are always present (controlled inputs).
      const subLevelRows = (d.subLevelInputConfig ?? []).map((r) => ({
        paymentMethodType: r.paymentMethodType,
        paymentMethod: r.paymentMethod,
        cardNetwork: r.cardNetwork ?? null,
        currency: r.currency ?? null,
        country: r.country ?? null,
        authType: r.authType ?? null,
        source: r.source ?? null,
        bucketSize: r.bucketSize,
        hedgingPercent: r.hedgingPercent ?? null,
        latencyThreshold: r.latencyThreshold ?? null,
      }))
      reset({
        defaultBucketSize: d.defaultBucketSize ?? 200,
        defaultSuccessRate: d.defaultSuccessRate ?? 0.5,
        defaultLatencyThreshold: d.defaultLatencyThreshold ?? null,
        defaultHedgingPercent: d.defaultHedgingPercent ?? null,
        // Stored as a fraction in the backend (e.g. 0.2) but shown as a percentage in the UI (20).
        margin: d.margin != null ? d.margin * 100 : null,
        subLevelInputConfig: subLevelRows,
      })
      setShowSubLevelOverrides(subLevelRows.length > 0)
    }
  }, [existing, reset])

  const { fields, append, remove } = useFieldArray({ control, name: 'subLevelInputConfig' })
  const watchedRows = watch('subLevelInputConfig')
  const subLevelOverridesOpen = showSubLevelOverrides || fields.length > 0

  function addSubLevelOverride() {
    setShowSubLevelOverrides(true)
    append({ paymentMethodType: 'card', paymentMethod: 'credit', cardNetwork: null, currency: null, country: null, authType: null, source: null, bucketSize: 20, hedgingPercent: null, latencyThreshold: null })
  }

  function removeSubLevelOverride(index: number) {
    remove(index)
    if (fields.length <= 1) setShowSubLevelOverrides(false)
  }

  async function ensureMerchantExists() {
    try {
      await apiPost(`/merchant-account/create`, {
        merchant_id: merchantId,
        gateway_success_rate_based_decider_input: null,
      })
    } catch {
      // Ignore — merchant may already exist
    }
  }

  async function onSave(data: SRForm) {
    if (!merchantId) { setSaveError('Set a Merchant ID first.'); return }
    setSaving(true); setSaveError(null); setSaveSuccess(false)
    try {
      if (!existing) await ensureMerchantExists()
      const endpoint = existing ? '/rule/update' : '/rule/create'
      await apiPost(endpoint, {
        merchant_id: merchantId,
        config: {
          type: 'successRate',
          data: {
            defaultBucketSize: data.defaultBucketSize,
            defaultSuccessRate: data.defaultSuccessRate,
            defaultLatencyThreshold: data.defaultLatencyThreshold,
            defaultHedgingPercent: data.defaultHedgingPercent,
            // Margin is not a user-facing knob right now, so we omit it and let the
            // backend default (multi_objective::DEFAULT_MARGIN = 1.0 / 100%) apply.
            // This avoids clobbering any stored value and keeps a single source of
            // truth on the backend. Revisit if margin becomes configurable again.
            subLevelInputConfig: data.subLevelInputConfig.length > 0
              ? data.subLevelInputConfig
              : null,
          },
        },
      })
      setLastSavedAt(new Date().toISOString())
      setSaveSuccess(true)
      mutate()
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  async function handleDelete() {
    if (!merchantId) return
    setDeleting(true); setDeleteError(null)
    try {
      await apiPost('/rule/delete', { merchant_id: merchantId, algorithm: 'successRate' })
      setLastSavedAt(null)
      mutate(undefined, { revalidate: false })
    } catch (err: unknown) {
      setDeleteError(err instanceof Error ? err.message : String(err))
    } finally {
      setDeleting(false)
    }
  }

  const lastModifiedAt = existing?.modified_at ?? lastSavedAt
  const lastModifiedDate = lastModifiedAt ? new Date(lastModifiedAt) : null
  const hasLastModified = Boolean(lastModifiedDate && !Number.isNaN(lastModifiedDate.getTime()))

  const tabClass = (tab: SRTab) =>
    `px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
      activeTab === tab
        ? 'border-brand-500 text-brand-600 dark:text-brand-400'
        : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'
    }`

  return (
    // Cost Estimation and Manual are rail + content dashboards, so they take the full page width —
    // constraining them would spend a quarter of an already-narrow column on the rail. The
    // single-column tabs (Autopilot, Flags) still read better constrained.
    <div className={`space-y-6 ${WIDE_TABS.includes(activeTab) ? 'w-full' : 'max-w-4xl'}`}>
      {/* Page header */}
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold text-slate-900 dark:text-white">Multi Objective Routing</h1>
          <p className="mt-0.5 text-[13px] leading-relaxed text-slate-500 dark:text-slate-400">
            Dynamic gateway scoring based on real-time success rates.
          </p>
        </div>
        {merchantId && !isLoading && existing?.config?.data && (
          <div className="flex items-center gap-2">
            <Badge variant="green">Active</Badge>
            {hasLastModified && lastModifiedDate && (
              <span className="text-xs text-slate-400">
                Saved {lastModifiedDate.toLocaleString()}
              </span>
            )}
            <Button type="button" variant="ghost" size="sm" onClick={() => setShowCurrentConfig(v => !v)}>
              <Eye size={14} className="mr-1" />{showCurrentConfig ? 'Hide config' : 'View config'}
            </Button>
            <Button
              type="button" variant="secondary" size="sm" disabled={deleting}
              onClick={() => { if (confirm('Clear the Success Rate configuration? This disables SR-based routing.')) handleDelete() }}
            >
              <Trash2 size={14} className="mr-1" />{deleting ? 'Clearing…' : 'Clear'}
            </Button>
          </div>
        )}
      </div>

      {!merchantId && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 px-4 py-3 text-sm text-yellow-800 dark:border-yellow-800/30 dark:bg-yellow-900/20 dark:text-yellow-400">
          Set a Merchant ID in the top bar to load and save configuration.
        </div>
      )}

      {/* Expanded config view */}
      {showCurrentConfig && existing?.config?.data && (
        <Card>
          <CardBody>
            <CurrentConfigDetails config={existing.config} />
          </CardBody>
        </Card>
      )}
      {deleteError && <p className="text-xs text-red-500">{deleteError}</p>}

      {/* Tab navigation */}
      <div className="border-b border-slate-200 dark:border-[#1c1c23]">
        <nav className="-mb-px flex gap-1">
          <button type="button" className={tabClass('autopilot')} onClick={() => setActiveTab('autopilot')}>Autopilot</button>
          <button type="button" className={tabClass('manual')} onClick={() => setActiveTab('manual')}>Manual</button>
          <button type="button" className={tabClass('flags')} onClick={() => setActiveTab('flags')}>Feature Flags</button>
          <button type="button" className={tabClass('cost')} onClick={() => setActiveTab('cost')}>Cost Estimation</button>
        </nav>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-12"><Spinner /></div>
      ) : (
        <>
          {/* ── Autopilot tab ── */}
          {activeTab === 'autopilot' && <AutopilotConfig merchantId={merchantId} />}

          {/* ── Manual tab ── */}
          {activeTab === 'manual' && (
            <div className="grid gap-6 lg:grid-cols-[220px_1fr] lg:items-start">
              <ManualSectionRail active={manualTab} onSelect={setManualTab} />

              <div className="min-w-0 space-y-6">
              {manualTab === 'scoring' && (
              <div className="space-y-6">
              <ManualCostToggle merchantId={merchantId} />
              <form onSubmit={handleSubmit(onSave)} className="space-y-6">
              <Card>
                <CardHeader>
                  <h2 className={type.heading}>Scoring defaults</h2>
                  <p className={`mt-1 ${type.subheading}`}>Applied to every payment type unless an override below replaces them.</p>
                </CardHeader>
                <CardBody className="grid gap-6 md:grid-cols-3">
                  <label className="space-y-1.5">
                    <span className={type.label}>Bucket size</span>
                    <input
                      type="number"
                      placeholder="50–200"
                      {...register('defaultBucketSize')}
                      className={configInputClass}
                    />
                    {errors.defaultBucketSize && <p className="text-[13px] text-red-500">{errors.defaultBucketSize.message}</p>}
                    <p className={type.hint}>
                      Recent payments per gateway score. Lower adapts faster, higher stays steadier.
                    </p>
                  </label>

                  <label className="space-y-1.5">
                    <span className={type.label}>Hedging %</span>
                    <input
                      type="number" step="0.1"
                      {...register('defaultHedgingPercent')}
                      placeholder="10"
                      className={configInputClass}
                    />
                    <p className={type.hint}>
                      Traffic share sent to non-top gateways to keep their scores fresh. Needs Explore-exploit on.
                    </p>
                  </label>

                  <label className="space-y-1.5">
                    <span className={type.label}>Latency threshold (s)</span>
                    <input
                      type="number"
                      {...register('defaultLatencyThreshold')}
                      placeholder="300"
                      className={configInputClass}
                    />
                    <p className={type.hint}>
                      Timeouts inside this window count as outages, outside it as slow performance.
                    </p>
                  </label>
                </CardBody>
              </Card>

              <Card>
                <CardHeader className="flex flex-row items-center justify-between">
                  <div>
                    <h2 className={type.heading}>Sub-level overrides</h2>
                    <p className={`mt-1 ${type.subheading}`}>Optional per payment-method-type overrides for the settings above.</p>
                  </div>
                  <Button type="button" variant="secondary" size="sm" onClick={addSubLevelOverride}>
                    <Plus size={14} /> Add Override
                  </Button>
                </CardHeader>
                {subLevelOverridesOpen && (
                  <CardBody className="overflow-x-auto p-0">
                    {fields.length ? (
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="text-left text-[12px] font-medium text-slate-500 dark:text-[#8d96aa] border-b border-slate-200 dark:border-[#1c1c24] bg-slate-50 dark:bg-[#0a0a0f]">
                            <th className="px-4 py-2">Source</th>
                            <th className="px-4 py-2">Method Type</th>
                            <th className="px-4 py-2">Method</th>
                            <th className="px-4 py-2">Card Network</th>
                            <th className="px-4 py-2">Currency</th>
                            <th className="px-4 py-2">Country</th>
                            <th className="px-4 py-2">Auth Type</th>
                            <th className="px-4 py-2">Memory Size</th>
                            <th className="px-4 py-2">Hedging %</th>
                            <th className="px-4 py-2">Timeout Grace (s)</th>
                            <th className="px-4 py-2" />
                          </tr>
                        </thead>
                        <tbody>
                          {fields.map((field, idx) => {
                            const methodType = watchedRows?.[idx]?.paymentMethodType || ''
                            const method = watchedRows?.[idx]?.paymentMethod || ''
                            // PAYMENT_METHODS is keyed lowercase; match case-insensitively. Always
                            // include the stored value as an option so auto-calibrated rows (which
                            // use the casing live txns send, e.g. "CARD"/"CREDIT") still display.
                            const baseMethodOptions = PAYMENT_METHODS[methodType.toLowerCase()] || ['credit', 'debit']
                            const typeOptions = optionsWithValue(PAYMENT_METHOD_TYPES, methodType)
                            const methodOptions = optionsWithValue(baseMethodOptions, method)
                            return (
                              <tr key={field.id} className="border-b border-slate-200 dark:border-[#1c1c24] hover:bg-slate-50 dark:bg-[#0f0f16] transition-colors">
                                <td className="px-4 py-2">
                                  {/* Hidden so RHF reliably round-trips provenance on save. */}
                                  <input type="hidden" {...register(`subLevelInputConfig.${idx}.source`)} />
                                  {watchedRows?.[idx]?.source === 'autopilot'
                                    ? <Badge variant="green">Auto</Badge>
                                    : <Badge variant="gray">Manual</Badge>}
                                </td>
                                <td className="px-4 py-2">
                                  <select {...register(`subLevelInputConfig.${idx}.paymentMethodType`)} className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg pl-2 pr-7 py-1 text-sm min-w-[6rem] focus:outline-none focus:ring-1 focus:ring-brand-500">
                                    {typeOptions.map((t) => <option key={t} value={t}>{t}</option>)}
                                  </select>
                                </td>
                                <td className="px-4 py-2">
                                  <select {...register(`subLevelInputConfig.${idx}.paymentMethod`)} className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg pl-2 pr-7 py-1 text-sm min-w-[6rem] focus:outline-none focus:ring-1 focus:ring-brand-500">
                                    {methodOptions.map((m) => <option key={m} value={m}>{m}</option>)}
                                  </select>
                                </td>
                                <td className="px-4 py-2"><input type="text" {...register(`subLevelInputConfig.${idx}.cardNetwork`)} placeholder="Any" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="text" {...register(`subLevelInputConfig.${idx}.currency`)} placeholder="Any" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="text" {...register(`subLevelInputConfig.${idx}.country`)} placeholder="Any" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="text" {...register(`subLevelInputConfig.${idx}.authType`)} placeholder="Any" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="number" {...register(`subLevelInputConfig.${idx}.bucketSize`)} className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="number" step="0.1" {...register(`subLevelInputConfig.${idx}.hedgingPercent`)} placeholder="—" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><input type="number" {...register(`subLevelInputConfig.${idx}.latencyThreshold`)} placeholder="—" className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500" /></td>
                                <td className="px-4 py-2"><button type="button" onClick={() => removeSubLevelOverride(idx)} className="text-slate-400 hover:text-red-500"><Trash2 size={14} /></button></td>
                              </tr>
                            )
                          })}
                        </tbody>
                      </table>
                    ) : (
                      <div className="px-4 py-6 text-sm text-slate-500">No overrides yet — defaults apply to all payment types.</div>
                    )}
                  </CardBody>
                )}
              </Card>

              <ErrorMessage error={saveError} />
              {saveSuccess && (
                <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-4 py-3 text-sm text-emerald-400">
                  Configuration saved.
                </div>
              )}
              <Button type="submit" disabled={saving || !merchantId}>
                {saving ? <><Spinner size={14} /> Saving…</> : 'Save changes'}
              </Button>
              </form>
              </div>
              )}

              {manualTab === 'elimination' && <EliminationConfig merchantId={merchantId} />}

              {manualTab === 'dimensions' && <SrDimensionsConfig merchantId={merchantId} />}
              </div>
            </div>
          )}

          {/* ── Feature Flags tab ── */}
          {activeTab === 'flags' && <SRFeatureFlags merchantId={merchantId} />}

          {/* ── Cost Estimation tab ── */}
          {activeTab === 'cost' && <CostEstimationPanel merchantId={merchantId} />}
        </>
      )}
    </div>
  )
}

/**
 * Left rail for the Manual tab's three sections. Deliberately the same shape as the Cost tab's
 * section rail (220px column, icon + title + blurb) so the two tabs don't teach two different
 * navigation idioms for the same kind of choice.
 */
function ManualSectionRail({
  active,
  onSelect,
}: {
  active: ManualSection
  onSelect: (s: ManualSection) => void
}) {
  return (
    <nav className="flex gap-2 overflow-x-auto lg:flex-col lg:gap-1 lg:overflow-visible">
      {MANUAL_SECTION_DEFS.map(({ id, icon: Icon, title, blurb }) => {
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
            <Icon size={18} className={`mt-0.5 shrink-0 ${on ? 'text-brand-500' : 'text-slate-400'}`} />
            <span className="min-w-0">
              <span className="block text-sm font-medium">{title}</span>
              <span className="mt-0.5 hidden text-xs text-slate-400 lg:block">{blurb}</span>
            </span>
          </button>
        )
      })}
    </nav>
  )
}

// Small on/off switch used by the Autopilot decision rows.
function Switch({ on, onClick, disabled }: { on: boolean; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      disabled={disabled}
      onClick={onClick}
      className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${
        on ? 'bg-brand-500' : 'bg-slate-300 dark:bg-slate-600'
      }`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
          on ? 'translate-x-6' : 'translate-x-1'
        }`}
      />
    </button>
  )
}

// Autopilot reframes routing as a set of outcomes rather than raw flags. The master
// toggle is the real switch: turning it OFF hard-disables every autopilot decision (their
// backend flags are set off) so the engine falls back to the Manual configuration. SR base
// routing ("switch PSP on low auth") is always on and shown as a status pill.
// Cost-savings toggle for the Manual config. Cost (multi-objective routing) is a feature flag that
// was previously only reachable from the Autopilot card (and disabled unless Autopilot was on), so a
// manual-config merchant could never turn cost on. This surfaces the same flag here, ungated, so
// cost-aware routing can run on the manual scoring config independently of Autopilot.
function ManualCostToggle({ merchantId }: { merchantId: string | null }) {
  const features = useMerchantFeatures(merchantId ?? undefined)
  const [toggling, setToggling] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const costOn = features.isEnabled('multi-objective-routing')

  async function toggle() {
    setToggling(true)
    setError(null)
    try {
      await features.setFeatureEnabled('multi-objective-routing', !costOn)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setToggling(false)
    }
  }

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-4">
        <div className="max-w-2xl">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium text-slate-800 dark:text-white">Optimize for economic value (cost awareness), not just approval rate</span>
            <Badge variant="gray">Cost savings</Badge>
            {costOn ? <Badge variant="green">On</Badge> : <Badge variant="gray">Off</Badge>}
          </div>
          <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
            Multi-objective routing: picks the highest expected-value PSP inside it.
          </p>
          {error && <p className="mt-1 text-xs text-red-500">{error}</p>}
        </div>
        <Switch on={costOn} disabled={!merchantId || features.isLoading || toggling} onClick={toggle} />
      </div>
    </Card>
  )
}

function AutopilotConfig({ merchantId }: { merchantId: string | null }) {
  const features = useMerchantFeatures(merchantId ?? undefined)
  const [toggling, setToggling] = useState<KnownFeature | 'master' | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  // The Static-vs-Automatic tuning illustrator is an explainer, hidden until the user
  // clicks the info icon next to the Self-tuning badge.
  const [showTuner, setShowTuner] = useState(false)

  const costOn = features.isEnabled('multi-objective-routing')
  const autoCalibrationOn = features.isEnabled('auto-calibration')
  // Master is its own persisted backend flag (`autopilot`) so the toggle survives reloads.
  const autopilotOn = features.isEnabled('autopilot')

  async function toggleFeature(feature: KnownFeature, enabled: boolean) {
    setToggling(feature); setError(null); setMessage(null)
    try {
      await features.setFeatureEnabled(feature, enabled)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setToggling(null)
    }
  }

  async function toggleMaster(next: boolean) {
    setToggling('master'); setError(null); setMessage(null)
    try {
      await features.setFeatureEnabled('autopilot', next)
      if (next) {
        // Turning Autopilot on enables its decisions by default — cost savings (multi-objective
        // economic routing) and auto-calibration — and activates all low-cardinality SR
        // dimensions so scoring clusters split on them (card scheme / currency / country /
        // auth type) and the calibrator can tune each cluster.
        // Enable unconditionally: the toggle is idempotent, and the captured `costOn` /
        // `autoCalibrationOn` booleans can be stale (the features list is SWR-cached for 5 min),
        // so guarding on them would silently skip the POST and leave the decision off.
        await features.setFeatureEnabled('multi-objective-routing', true)
        await features.setFeatureEnabled('auto-calibration', true)
        if (merchantId) await enableAutopilotSrDimensions(merchantId)
      } else {
        // Hard-disable: turn every autopilot decision off so routing uses manual config.
        // Unconditional for the same reason as the enable path — stale cached booleans must not
        // gate the POST, or a flag that is actually on server-side would be left enabled.
        await features.setFeatureEnabled('elimination', false)
        await features.setFeatureEnabled('multi-objective-routing', false)
        await features.setFeatureEnabled('auto-calibration', false)
      }
      setMessage(next
        ? 'Autopilot on — cost savings and auto-calibration enabled; fine-tune the decisions below.'
        : 'Autopilot off — routing uses your Manual configuration.')
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setToggling(null)
    }
  }

  const busy = !merchantId || features.isLoading
  const rowDisabled = busy || !autopilotOn || toggling !== null

  return (
    <div className="space-y-4">
      {error && (
        <p className="rounded-lg border border-red-500/20 bg-red-500/8 px-3 py-2 text-xs text-red-500">{error}</p>
      )}
      {message && (
        <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-xs text-emerald-500">{message}</p>
      )}

      {/* Master toggle */}
      <Card>
        <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-4">
          <div className="max-w-2xl">
            <div className="flex items-center gap-2">
              <span className={type.heading}>Autopilot mode</span>
              {autopilotOn ? <Badge variant="green">On</Badge> : <Badge variant="gray">Off</Badge>}
            </div>
            <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
              Let the engine adapt routing automatically. Turn off to route purely by your Manual configuration.
            </p>
          </div>
          <Switch on={autopilotOn} disabled={busy || toggling !== null} onClick={() => toggleMaster(!autopilotOn)} />
        </div>
      </Card>

      {/* Autopilot decisions */}
      <Card className={autopilotOn ? '' : 'opacity-60'}>
        {/* (i) SRv3 — always on, shown as status */}
        <div className="flex flex-wrap items-center justify-between gap-4 px-5 py-4">
          <div className="max-w-2xl">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium text-slate-800 dark:text-white">Always route to the best-performing PSP</span>
              <Badge variant="gray">SRv3</Badge>
              <Badge variant="green">Active</Badge>
            </div>
            <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
              Real-time success-rate routing. Always on — it is the base routing path.
            </p>
          </div>
        </div>

        {/* (ii) Elimination — hidden for now. Re-enable this row to expose the toggle;
            the underlying `elimination` flag wiring (seeding + master-off) stays in place.
        <div className="flex flex-wrap items-center justify-between gap-4 border-t border-slate-100 px-5 py-4 dark:border-[#222226]">
          <div className="max-w-2xl">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium text-slate-800 dark:text-white">Disable PSP in case of sub-threshold auth rates</span>
              <Badge variant="gray">Elimination</Badge>
            </div>
            <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
              Temporarily removes a PSP whose auth rate drops below the elimination threshold.
            </p>
          </div>
          <Switch on={eliminationOn} disabled={rowDisabled} onClick={() => toggleFeature('elimination', !eliminationOn)} />
        </div>
        */}

        {/* (iii) Cost savings */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-t border-slate-100 px-5 py-4 dark:border-[#222226]">
          <div className="max-w-2xl">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium text-slate-800 dark:text-white">Optimize for economic value (cost awareness), not just approval rate</span>
              <Badge variant="gray">Cost savings</Badge>
            </div>
            <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
              Multi-objective routing: picks the highest expected-value PSP inside it.
            </p>
          </div>
          <Switch on={costOn} disabled={rowDisabled} onClick={() => toggleFeature('multi-objective-routing', !costOn)} />
        </div>

        {/* (iv) Auto-calibration */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-t border-slate-100 px-5 py-4 dark:border-[#222226]">
          <div className="max-w-2xl">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm font-medium text-slate-800 dark:text-white">Self-tune routing settings to match your traffic patterns</span>
              <Badge variant="gray">Self-tuning</Badge>
              {autopilotOn && autoCalibrationOn && (
                <button
                  type="button"
                  onClick={() => setShowTuner((s) => !s)}
                  aria-expanded={showTuner}
                  aria-label="Show static vs automatic tuning illustration"
                  className={`inline-flex items-center rounded-full p-0.5 transition-colors ${showTuner ? 'text-brand-600 dark:text-brand-400' : 'text-slate-400 hover:text-slate-600 dark:text-slate-500 dark:hover:text-slate-300'}`}
                >
                  <Info className="h-4 w-4" />
                </button>
              )}
            </div>
            <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">
              Auto configures the Learning window and Discovery share based on your traffic volume.
            </p>
          </div>
          <Switch on={autoCalibrationOn} disabled={rowDisabled} onClick={() => toggleFeature('auto-calibration', !autoCalibrationOn)} />
        </div>
      </Card>

      {autopilotOn && autoCalibrationOn && showTuner && <BucketHedgingTuner />}
    </div>
  )
}

const SR_FEATURES: { feature: KnownFeature; title: string; description: string }[] = [
  {
    feature: 'gsm-scoring-filter',
    title: 'GSM scoring filter',
    description:
      'Skip gateway penalization for failures classified by GSM as user or issuer-originated. Keeps gateway scores accurate by excluding faults that are not the gateway\'s responsibility.',
  },
  {
    feature: 'explore-exploit-srv3',
    title: 'Explore-exploit on SRv3 (Card)',
    description:
      'Keeps all gateway ratings fresh by regularly sending a small share of payments to every gateway — not just the top performer. This ensures the system can quickly detect when a backup gateway becomes better than the current top, and reroute accordingly. The Hedging % setting controls how large this share is.',
  },
  {
    feature: 'ab-test-real-payments',
    title: 'A/B test on real payments',
    description:
      'Routes live production traffic through the active A/B test algorithm. When enabled, each payment is deterministically assigned to a control or variant arm based on its payment ID. Disable at any time to fall back to standard SR routing with no impact on in-flight payments.',
  },
  // 'multi-objective-routing' is intentionally not listed here — it is owned by the
  // Autopilot "Maximize economic value" decision (see AutopilotConfig).
]

function SrDimensionsConfig({ merchantId }: { merchantId: string | null }) {
  const { data, mutate } = useSWR<SrDimensionResponse>(
    merchantId ? `/config-sr-dimension/${merchantId}` : null,
    fetcher,
    { revalidateOnFocus: false, shouldRetryOnError: false }
  )
  const [selected, setSelected] = useState<string[] | null>(null)
  const [udfs, setUdfs] = useState<number[]>([])
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)

  useEffect(() => {
    if (!data) return
    setSelected(data.paymentInfo?.fields ?? [])
    setUdfs(data.paymentInfo?.udfs ?? [])
  }, [data])

  const current = selected ?? []

  function toggle(key: string) {
    setSaveSuccess(false)
    setSelected((prev) => {
      const cur = prev ?? []
      return cur.includes(key) ? cur.filter((k) => k !== key) : [...cur, key]
    })
  }

  async function onSave() {
    if (!merchantId || selected == null) return
    setSaving(true); setSaveError(null); setSaveSuccess(false)
    try {
      await apiPost('/config-sr-dimension', {
        merchant_id: merchantId,
        paymentInfo: { udfs, fields: selected },
      })
      setSaveSuccess(true)
      mutate()
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <h2 className={type.heading}>SR scoring dimensions</h2>
          <p className={`mt-1 ${type.subheading}`}>
            Attributes SR scoring splits clusters on. More dimensions = finer, more responsive scores, but more clusters (each needs its own volume to score well). Changing this re-buckets scores. Autopilot enables the low-cardinality dimensions automatically.
          </p>
        </CardHeader>
        <CardBody className="space-y-1">
          {ELIGIBLE_SR_DIMENSIONS.map(({ key, label, note }) => (
            <label key={key} className="flex items-center gap-3 cursor-pointer select-none py-1.5">
              <input
                type="checkbox"
                checked={current.includes(key)}
                onChange={() => toggle(key)}
                disabled={!merchantId || selected == null}
                className="rounded border-slate-300 dark:border-slate-600 disabled:cursor-not-allowed"
              />
              <span className="text-sm text-slate-700 dark:text-slate-200">{label}</span>
              {note && <span className="text-[11px] text-amber-500">{note}</span>}
            </label>
          ))}
        </CardBody>
      </Card>

      <ErrorMessage error={saveError} />
      {saveSuccess && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-4 py-3 text-sm text-emerald-400">
          SR dimensions saved.
        </div>
      )}
      <Button onClick={onSave} disabled={saving || !merchantId || selected == null}>
        {saving ? <><Spinner size={14} /> Saving…</> : 'Save SR Dimensions'}
      </Button>
    </div>
  )
}

function SRFeatureFlags({ merchantId }: { merchantId: string | null }) {
  const features = useMerchantFeatures(merchantId ?? undefined)
  const [toggling, setToggling] = useState<KnownFeature | null>(null)
  const [message, setMessage] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function toggle(feature: KnownFeature, enabled: boolean) {
    setToggling(feature)
    setMessage(null)
    setError(null)
    try {
      await features.setFeatureEnabled(feature, enabled)
      const label = SR_FEATURES.find((f) => f.feature === feature)?.title ?? feature
      setMessage(`${label} ${enabled ? 'enabled' : 'disabled'}.`)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setToggling(null)
    }
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className={type.heading}>Scoring behaviour flags</h2>
        <p className={`mt-1 ${type.subheading}`}>
          Merchant-level toggles that affect how SR scores are computed and how traffic is explored.
        </p>
      </div>

      {error && (
        <p className="rounded-lg border border-red-500/20 bg-red-500/8 px-3 py-2 text-xs text-red-500">{error}</p>
      )}
      {message && (
        <p className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-3 py-2 text-xs text-emerald-500">{message}</p>
      )}

      <Card>
        {SR_FEATURES.map(({ feature, title, description }, idx) => {
          const enabled = features.isEnabled(feature)
          return (
            <div
              key={feature}
              className={`flex flex-wrap items-center justify-between gap-4 px-5 py-4 ${
                idx > 0 ? 'border-t border-slate-100 dark:border-[#222226]' : ''
              }`}
            >
              <div className="max-w-2xl">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium text-slate-800 dark:text-white">{title}</span>
                  {features.isLoading ? (
                    <Badge variant="gray">Checking</Badge>
                  ) : enabled ? (
                    <Badge variant="green">Enabled</Badge>
                  ) : (
                    <Badge variant="gray">Disabled</Badge>
                  )}
                </div>
                <p className="mt-1 text-xs leading-5 text-slate-500 dark:text-[#9aa6bb]">{description}</p>
              </div>
              <div>
                {enabled ? (
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => toggle(feature, false)}
                    disabled={!merchantId || toggling === feature || features.isLoading}
                  >
                    <PowerOff size={13} />
                    {toggling === feature ? 'Disabling' : 'Disable'}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="primary"
                    onClick={() => toggle(feature, true)}
                    disabled={!merchantId || toggling === feature || features.isLoading}
                  >
                    {toggling === feature ? 'Enabling' : 'Enable'}
                  </Button>
                )}
              </div>
            </div>
          )
        })}
      </Card>
    </div>
  )
}

function EliminationConfig({ merchantId }: { merchantId: string | null }) {
  const [threshold, setThreshold] = useState<string>('')
  const [gatewayLatency, setGatewayLatency] = useState<string>('')
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [showCurrentConfig, setShowCurrentConfig] = useState(false)

  const { data: existing, mutate } = useSWR<EliminationConfigResponse>(
    merchantId ? ['rule-elimination', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'elimination' }),
    { shouldRetryOnError: false, revalidateOnFocus: false }
  )

  useEffect(() => {
    if (existing?.config?.data) {
      const d = existing.config.data
      setThreshold(String(d.threshold))
      setGatewayLatency(d.txnLatency?.gatewayLatency != null ? String(d.txnLatency.gatewayLatency) : '')
    }
  }, [existing])

  async function onSave() {
    if (!merchantId) return
    const parsedThreshold = parseFloat(threshold)
    if (isNaN(parsedThreshold) || parsedThreshold < 0 || parsedThreshold > 1) {
      setSaveError('Threshold must be a number between 0 and 1.')
      return
    }
    setSaving(true); setSaveError(null); setSaveSuccess(false)
    try {
      const endpoint = existing ? '/rule/update' : '/rule/create'
      const parsedLatency = gatewayLatency !== '' ? parseFloat(gatewayLatency) : null
      await apiPost(endpoint, {
        merchant_id: merchantId,
        config: {
          type: 'elimination',
          data: {
            threshold: parsedThreshold,
            txnLatency: parsedLatency != null ? { gatewayLatency: parsedLatency } : null,
          },
        },
      })
      setSaveSuccess(true)
      mutate()
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  async function onDelete() {
    if (!merchantId) return
    setDeleting(true)
    try {
      await apiPost('/rule/delete', { merchant_id: merchantId, algorithm: 'elimination' })
      setThreshold('')
      setGatewayLatency('')
      mutate(undefined, { revalidate: false })
    } catch (err: unknown) {
      setSaveError(err instanceof Error ? err.message : String(err))
    } finally {
      setDeleting(false)
    }
  }

  return (
    <div className="space-y-6">
      {merchantId && existing?.config?.data && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-4">
            <div className="min-w-0">
              <h2 className={type.heading}>
                Elimination Configuration
              </h2>
              <p className={`mt-1 ${type.subheading}`}>
                Elimination routing is active · threshold {existing.config.data.threshold}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant="green">Active</Badge>
              <Button type="button" variant="ghost" size="sm" onClick={() => setShowCurrentConfig(v => !v)}>
                <Eye size={14} className="mr-1" />{showCurrentConfig ? 'Hide' : 'View'}
              </Button>
              <Button
                type="button" variant="secondary" size="sm" disabled={deleting}
                onClick={() => { if (confirm('Clear the Elimination configuration?')) onDelete() }}
              >
                <Trash2 size={14} className="mr-1" />{deleting ? 'Clearing…' : 'Clear'}
              </Button>
            </div>
          </CardHeader>
          {showCurrentConfig && (
            <CardBody className="border-t border-slate-100 dark:border-[#222226] text-xs text-slate-600 dark:text-[#b2bdd1]">
              <pre className="max-h-40 overflow-auto rounded-lg border border-slate-200/80 bg-slate-50/90 p-3 font-mono text-xs dark:border-[#2a303a] dark:bg-[#0b1017] dark:text-[#d8e1ef]">
                {JSON.stringify(existing.config, null, 2)}
              </pre>
            </CardBody>
          )}
        </Card>
      )}

      <Card>
        <CardHeader>
          <h2 className={type.heading}>Elimination</h2>
          <p className={`mt-1 ${type.subheading}`}>
            Each gateway carries a health score that decays on consecutive failures and recovers on
            successes. Below the threshold, it drops out of routing until it recovers.
          </p>
        </CardHeader>
        <CardBody className="grid gap-4 md:grid-cols-2">
          <label className="space-y-1.5">
            <span className={type.label}>Threshold <span className="text-red-400">*</span></span>
            <input
              type="number" step="0.01" min="0" max="1"
              value={threshold}
              onChange={e => setThreshold(e.target.value)}
              placeholder="0.05"
              className={configInputClass}
            />
            <p className={type.hint}>
              Health score from 0 to 1, not success rate. Lower tolerates more failures before dropping a gateway.
            </p>
          </label>
          <label className="space-y-1.5">
            <span className={type.label}>Gateway latency threshold (ms)</span>
            <input
              type="number"
              value={gatewayLatency}
              onChange={e => setGatewayLatency(e.target.value)}
              placeholder="5000"
              className={configInputClass}
            />
            <p className={type.hint}>
              Gateways slower than this are eliminated too. Leave blank to disable.
            </p>
          </label>
        </CardBody>
      </Card>

      <ErrorMessage error={saveError} />
      {saveSuccess && (
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-4 py-3 text-sm text-emerald-400">
          Elimination configuration saved successfully.
        </div>
      )}
      <Button onClick={onSave} disabled={saving || !merchantId || threshold === ''}>
        {saving ? <><Spinner size={14} /> Saving…</> : 'Save Elimination Config'}
      </Button>
    </div>
  )
}
