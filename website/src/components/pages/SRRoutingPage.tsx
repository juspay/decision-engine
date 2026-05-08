import useSWR from 'swr'
import { useForm, useFieldArray } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useEffect, useState } from 'react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { PAYMENT_METHOD_TYPES, PAYMENT_METHODS } from '../../lib/constants'
import { Plus, Trash2, Eye, ChevronDown, Info } from 'lucide-react'

// ---- Schema ----
const subLevelSchema = z.object({
  paymentMethodType: z.string().min(1),
  paymentMethod: z.string().min(1),
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
      subLevelInputConfig: {
        paymentMethodType: string
        paymentMethod: string
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
        <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Default Settings</h3>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          <div>
            <span className="text-slate-500">Bucket Size:</span>
            <p className="font-medium">{config.data.defaultBucketSize}</p>
          </div>
          <div>
            <span className="text-slate-500">Success Rate:</span>
            <p className="font-medium">{config.data.defaultSuccessRate ?? 'Not set'}</p>
          </div>
          <div>
            <span className="text-slate-500">Hedging %:</span>
            <p className="font-medium">{config.data.defaultHedgingPercent ?? 'Not set'}</p>
          </div>
          <div>
            <span className="text-slate-500">Feedback Latency Window:</span>
            <p className="font-medium">{config.data.defaultLatencyThreshold ?? 'Not set'} s</p>
          </div>
        </div>
      </div>

      {config.data.subLevelInputConfig && config.data.subLevelInputConfig.length > 0 ? (
        <div>
          <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Sub-Level Configurations</h3>
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
                    <span className="text-slate-500">Bucket Size:</span>
                    <p className="font-medium">{subConfig.bucketSize}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Hedging %:</span>
                    <p className="font-medium">{subConfig.hedgingPercent ?? 'Default'}</p>
                  </div>
                  <div>
                    <span className="text-slate-500">Feedback Latency Window:</span>
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

export function SRRoutingPage() {
  const { merchantId } = useMerchantStore()
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)
  const [showCurrentConfig, setShowCurrentConfig] = useState(false)
  const [showSubLevelOverrides, setShowSubLevelOverrides] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)
  const [lastSavedAt, setLastSavedAt] = useState<string | null>(null)
  const [showGuide, setShowGuide] = useState(false)

  const { data: existing, isLoading, mutate } = useSWR<SRConfigResponse>(
    merchantId ? ['rule-sr', merchantId] : null,
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
      subLevelInputConfig: [],
    },
  })

  // Pre-fill form from fetched config
  useEffect(() => {
    if (existing?.config?.data) {
      const d = existing.config.data
      const subLevelRows = d.subLevelInputConfig ?? []
      reset({
        defaultBucketSize: d.defaultBucketSize ?? 200,
        defaultSuccessRate: d.defaultSuccessRate ?? 0.5,
        defaultLatencyThreshold: d.defaultLatencyThreshold ?? null,
        defaultHedgingPercent: d.defaultHedgingPercent ?? null,
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
    append({ paymentMethodType: 'card', paymentMethod: 'credit', bucketSize: 20, hedgingPercent: null, latencyThreshold: null })
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

  return (
    <div className="space-y-6 max-w-5xl">
      <div>
        <h1 className="text-2xl font-semibold text-slate-900">Auth-Rate Based Routing</h1>
      </div>

      {!merchantId && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 px-4 py-3 text-sm text-yellow-800">
          Set a Merchant ID in the top bar to load and save configuration.
        </div>
      )}

      {/* Status Card */}
      {merchantId && !isLoading && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-4">
            <div className="min-w-0">
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                {existing?.config?.data ? 'Current Active Configuration' : 'Configuration Status'}
              </h2>
              <p className="text-xs text-slate-500 mt-0.5">
                {existing?.config?.data
                  ? (
                    <>
                      Success Rate routing is configured and active
                      {hasLastModified && lastModifiedDate ? (
                        <span className="ml-1">· Last saved {lastModifiedDate.toLocaleString()}</span>
                      ) : null}
                    </>
                  )
                  : 'No Success Rate configuration found'}
              </p>
            </div>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <Badge variant={existing?.config?.data ? 'green' : 'gray'}>
                {existing?.config?.data ? 'Active' : 'Not Configured'}
              </Badge>
              {existing?.config?.data ? (
                <>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => setShowCurrentConfig(!showCurrentConfig)}
                  >
                    <Eye size={14} className="mr-1" />
                    {showCurrentConfig ? 'Hide' : 'View'}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      if (confirm('Are you sure you want to clear the Success Rate configuration? This will disable SR-based routing.')) {
                        handleDelete()
                      }
                    }}
                    disabled={deleting}
                  >
                    <Trash2 size={14} className="mr-1" />
                    {deleting ? 'Clearing...' : 'Clear Configuration'}
                  </Button>
                </>
              ) : null}
            </div>
          </CardHeader>
          {existing?.config?.data && (deleteError || showCurrentConfig) && (
            <CardBody className="border-t border-slate-100 dark:border-[#222226]">
              {deleteError && (
                <p className={`text-xs text-red-500 ${showCurrentConfig ? 'mb-3' : ''}`}>{deleteError}</p>
              )}
              {showCurrentConfig ? (
                <CurrentConfigDetails config={existing.config} />
              ) : null}
            </CardBody>
          )}
        </Card>
      )}

      {isLoading ? (
        <div className="flex justify-center py-12"><Spinner /></div>
      ) : (
        <>
        <form onSubmit={handleSubmit(onSave)} className="space-y-6">

          {/* Configuration Guide */}
          <Card>
            <div
              className="flex flex-row items-center justify-between cursor-pointer select-none min-w-0 border-b border-slate-200 px-6 py-5 dark:border-[#2a303a]"
              onClick={() => setShowGuide(v => !v)}
            >
              <div className="flex items-center gap-2">
                <Info size={14} className="text-slate-400 dark:text-slate-500 shrink-0" />
                <h2 className="text-sm font-semibold text-slate-800 dark:text-white">How to configure these settings</h2>
              </div>
              <ChevronDown
                size={14}
                className={`text-slate-400 transition-transform duration-200 ${showGuide ? 'rotate-180' : ''}`}
              />
            </div>

            {showGuide && (
              <CardBody className="border-t border-slate-100 dark:border-[#222226] space-y-6 text-xs text-slate-600 dark:text-[#b2bdd1]">

                {/* Bucket Size */}
                <div className="space-y-2">
                  <h3 className="font-semibold text-slate-700 dark:text-slate-200">Bucket Size</h3>
                  <p>
                    The algorithm recalculates each gateway's score after every <em>bucket_size</em> transactions on
                    that gateway. Smaller buckets react faster to outages but produce noisier scores.
                    Larger buckets are more stable but slower to shift traffic when a gateway degrades.
                  </p>
                  <div className="rounded-lg bg-slate-50 dark:bg-[#0f0f16] border border-slate-200 dark:border-[#1c1c24] px-3 py-2 font-mono text-[11px]">
                    detection time ≈ bucket_size ÷ (txns_per_hr ÷ num_gateways ÷ 60) &nbsp;minutes
                  </div>
                  <table className="w-full text-[11px] border-collapse">
                    <thead>
                      <tr className="text-left text-slate-500 border-b border-slate-200 dark:border-[#1c1c24]">
                        <th className="py-1 pr-4 font-medium">Volume</th>
                        <th className="py-1 pr-4 font-medium">Recommended bucket</th>
                        <th className="py-1 font-medium">Detection time</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100 dark:divide-[#1c1c24]">
                      <tr><td className="py-1 pr-4">&lt; 500 txns/hr</td><td className="py-1 pr-4 font-semibold">20</td><td className="py-1 text-slate-500">~2–5 min</td></tr>
                      <tr><td className="py-1 pr-4">500–2000 txns/hr</td><td className="py-1 pr-4 font-semibold">50</td><td className="py-1 text-slate-500">~1–2 min</td></tr>
                      <tr><td className="py-1 pr-4">2000–5000 txns/hr</td><td className="py-1 pr-4 font-semibold">100</td><td className="py-1 text-slate-500">~1–2 min</td></tr>
                      <tr><td className="py-1 pr-4">&gt; 5000 txns/hr</td><td className="py-1 pr-4 font-semibold">200</td><td className="py-1 text-slate-500">~1–2 min</td></tr>
                    </tbody>
                  </table>
                  <p className="text-slate-500 dark:text-slate-400">
                    A bucket size of 50 reliably detects a 10%+ drop in success rate. Detecting a 1% difference
                    requires ~5500 transactions per bucket — not practical. Size your bucket to catch real outages,
                    not marginal noise.
                  </p>
                </div>

                {/* Hedging */}
                <div className="space-y-2">
                  <h3 className="font-semibold text-slate-700 dark:text-slate-200">Hedging %</h3>
                  <p>
                    Hedging sends a fixed percentage of traffic to backup gateways even when a primary gateway
                    dominates. This keeps backup gateway scores fresh so the algorithm can shift traffic
                    instantly when the primary fails — without waiting for the backup to accumulate enough
                    transactions to be trusted.
                  </p>
                  <p className="text-amber-600 dark:text-amber-400">
                    If hedging is too low, backup scores go stale. When the primary goes down, the algorithm
                    sees no reliable data for the backup and failover is slow.
                  </p>
                  <p>Set hedging so the backup fills one bucket within your acceptable failover window:</p>
                  <div className="rounded-lg bg-slate-50 dark:bg-[#0f0f16] border border-slate-200 dark:border-[#1c1c24] px-3 py-2 font-mono text-[11px] space-y-1">
                    <div>hedging% = (bucket_size ÷ failover_minutes) ÷ txns_per_minute × 100</div>
                    <div className="text-slate-400">— e.g. bucket=50, 5 min window, 60 txns/min → (50÷5)÷60×100 ≈ 17%</div>
                  </div>
                  <table className="w-full text-[11px] border-collapse">
                    <thead>
                      <tr className="text-left text-slate-500 border-b border-slate-200 dark:border-[#1c1c24]">
                        <th className="py-1 pr-4 font-medium">Volume</th>
                        <th className="py-1 pr-4 font-medium">Bucket</th>
                        <th className="py-1 pr-4 font-medium">Failover window</th>
                        <th className="py-1 font-medium">Recommended hedging</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100 dark:divide-[#1c1c24]">
                      <tr><td className="py-1 pr-4">&lt; 500/hr</td><td className="py-1 pr-4">20</td><td className="py-1 pr-4">10 min</td><td className="py-1 font-semibold">~7%</td></tr>
                      <tr><td className="py-1 pr-4">500–2000/hr</td><td className="py-1 pr-4">50</td><td className="py-1 pr-4">5 min</td><td className="py-1 font-semibold">~15%</td></tr>
                      <tr><td className="py-1 pr-4">2000–5000/hr</td><td className="py-1 pr-4">100</td><td className="py-1 pr-4">5 min</td><td className="py-1 font-semibold">~20%</td></tr>
                      <tr><td className="py-1 pr-4">&gt; 5000/hr</td><td className="py-1 pr-4">200</td><td className="py-1 pr-4">5 min</td><td className="py-1 font-semibold">~20%</td></tr>
                    </tbody>
                  </table>
                </div>

                {/* Default Success Rate */}
                <div className="space-y-2">
                  <h3 className="font-semibold text-slate-700 dark:text-slate-200">Default Success Rate</h3>
                  <p>
                    The prior score assigned to any gateway that has no transaction history yet (e.g. newly added
                    gateways). Set this close to your historical average across all gateways — typically
                    <span className="font-semibold"> 0.80–0.95</span>.
                  </p>
                  <p className="text-slate-500 dark:text-slate-400">
                    Too high → new or poor-quality gateways receive too much traffic before proving themselves.
                    Too low → new gateways are starved of traffic and take longer to build a reliable score.
                  </p>
                </div>

                {/* Feedback Latency Window */}
                <div className="space-y-2">
                  <h3 className="font-semibold text-slate-700 dark:text-slate-200">Feedback Latency Window</h3>
                  <p>
                    When a score update (feedback) arrives, the system measures how long has passed since the
                    transaction was created. If that elapsed time is within this window, the failure is
                    classified as a standard penalty; if it exceeds the window, it is classified as an SR V3
                    penalty. This affects which scoring path is applied, not whether the gateway is filtered
                    during routing. Unit is <span className="font-semibold">seconds</span>. Default is 300 s (5 min).
                  </p>
                </div>

              </CardBody>
            )}
          </Card>

          <Card>
            <CardHeader>
              <div>
                <h2 className="text-sm font-semibold text-slate-800">Default Success Rate Config</h2>
                <p className="text-xs text-slate-500 mt-0.5">
                  Base settings used when there is no payment-method-specific override.
                </p>
              </div>
            </CardHeader>
            <CardBody className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
              <label className="space-y-1">
                <span className="text-xs text-slate-500">Bucket Size</span>
                <input
                  type="number"
                  {...register('defaultBucketSize')}
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                {errors.defaultBucketSize && (
                  <p className="text-xs text-red-500">{errors.defaultBucketSize.message}</p>
                )}
                <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-relaxed">
                  Transactions per score recalculation. Smaller = faster reaction. Aim for a 1–5 min fill time at your volume.
                </p>
              </label>

              <label className="space-y-1">
                <span className="text-xs text-slate-500">Success Rate</span>
                <input
                  type="number"
                  step="0.1"
                  min="0"
                  max="1"
                  {...register('defaultSuccessRate')}
                  placeholder="0.5"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-relaxed">
                  Prior score for gateways with no history yet. Set near your expected average (0.80–0.95).
                </p>
              </label>

              <label className="space-y-1">
                <span className="text-xs text-slate-500">Hedging %</span>
                <input
                  type="number"
                  step="0.1"
                  {...register('defaultHedgingPercent')}
                  placeholder="null"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-relaxed">
                  Traffic % always sent to backup gateways to keep their scores fresh. Too low = slow failover.
                </p>
              </label>

              <label className="space-y-1">
                <span className="text-xs text-slate-500">Feedback Latency Window (s)</span>
                <input
                  type="number"
                  {...register('defaultLatencyThreshold')}
                  placeholder="300"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
                <p className="text-[11px] text-slate-400 dark:text-slate-500 leading-relaxed">
                  Seconds after txn creation within which feedback is a standard penalty; beyond it uses the SR V3 penalty path. Default 300 s.
                </p>
              </label>

            </CardBody>
          </Card>

          <Card>
            <CardHeader className="flex flex-row items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold text-slate-800">Sub-Level Overrides</h2>
                <p className="text-xs text-slate-500 mt-0.5">
                  Optional overrides for specific payment method type and method combinations.
                </p>
              </div>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                onClick={addSubLevelOverride}
              >
                <Plus size={14} /> Add Level
              </Button>
            </CardHeader>
            {subLevelOverridesOpen ? (
              <CardBody className="overflow-x-auto p-0">
              {fields.length ? (
                <table className="w-full text-sm">
                  <thead>
                    <tr className="text-left text-xs text-slate-500 border-b border-slate-200 dark:border-[#1c1c24] bg-slate-50 dark:bg-[#0a0a0f]">
                      <th className="px-4 py-2">Payment Method Type</th>
                      <th className="px-4 py-2">Payment Method</th>
                      <th className="px-4 py-2">Bucket Size</th>
                      <th className="px-4 py-2">Hedging %</th>
                      <th className="px-4 py-2">Feedback Latency Window (s)</th>
                      <th className="px-4 py-2" />
                    </tr>
                  </thead>
                  <tbody>
                    {fields.map((field, idx) => {
                      const methodType = watchedRows?.[idx]?.paymentMethodType || ''
                      const methodOptions = PAYMENT_METHODS[methodType] || []
                      return (
                        <tr key={field.id} className="border-b border-slate-200 dark:border-[#1c1c24] hover:bg-slate-100 dark:bg-[#0f0f16] transition-colors">
                          <td className="px-4 py-2">
                            <select
                              {...register(`subLevelInputConfig.${idx}.paymentMethodType`)}
                              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            >
                              {PAYMENT_METHOD_TYPES.map((t) => (
                                <option key={t} value={t}>{t}</option>
                              ))}
                            </select>
                          </td>
                          <td className="px-4 py-2">
                            <select
                              {...register(`subLevelInputConfig.${idx}.paymentMethod`)}
                              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                            >
                              {(methodOptions.length ? methodOptions : ['credit', 'debit']).map((m) => (
                                <option key={m} value={m}>{m}</option>
                              ))}
                            </select>
                          </td>
                          <td className="px-4 py-2">
                            <input
                              type="number"
                              {...register(`subLevelInputConfig.${idx}.bucketSize`)}
                              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </td>
                          <td className="px-4 py-2">
                            <input
                              type="number"
                              step="0.1"
                              {...register(`subLevelInputConfig.${idx}.hedgingPercent`)}
                              placeholder="null"
                              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </td>
                          <td className="px-4 py-2">
                            <input
                              type="number"
                              {...register(`subLevelInputConfig.${idx}.latencyThreshold`)}
                              placeholder="null"
                              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500"
                            />
                          </td>
                          <td className="px-4 py-2">
                            <button type="button" onClick={() => removeSubLevelOverride(idx)} className="text-slate-400 hover:text-red-500">
                              <Trash2 size={14} />
                            </button>
                          </td>
                        </tr>
                      )
                    })}
                  </tbody>
                </table>
              ) : (
                <div className="px-4 py-8 text-sm text-slate-500">
                  No sub-level overrides configured. The default row above is the only active configuration.
                </div>
              )}
              </CardBody>
            ) : null}
          </Card>

          <ErrorMessage error={saveError} />
          {saveSuccess && (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-4 py-3 text-sm text-emerald-400">
              Configuration saved successfully.
            </div>
          )}

          <Button type="submit" disabled={saving || !merchantId}>
            {saving ? <><Spinner size={14} /> Saving…</> : 'Save Configuration'}
          </Button>
        </form>

        <EliminationConfig merchantId={merchantId} />
        </>
      )}
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
              <h2 className="text-sm font-semibold text-slate-800 dark:text-white">
                Elimination Configuration
              </h2>
              <p className="text-xs text-slate-500 mt-0.5">
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
          <h2 className="text-sm font-semibold text-slate-800 dark:text-white">Elimination Config</h2>
          <p className="text-xs text-slate-500 mt-0.5">
            Gateways whose SR score drops below the threshold are removed from routing entirely.
          </p>
        </CardHeader>
        <CardBody className="grid gap-4 md:grid-cols-2">
          <label className="space-y-1">
            <span className="text-xs text-slate-500">Threshold <span className="text-red-400">*</span></span>
            <input
              type="number" step="0.01" min="0" max="1"
              value={threshold}
              onChange={e => setThreshold(e.target.value)}
              placeholder="e.g. 0.35"
              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
            />
            <p className="text-[11px] text-slate-400 dark:text-slate-500">
              Score (0–1) below which a gateway is eliminated. System default is 0.05.
            </p>
          </label>
          <label className="space-y-1">
            <span className="text-xs text-slate-500">Gateway Latency Threshold (ms)</span>
            <input
              type="number"
              value={gatewayLatency}
              onChange={e => setGatewayLatency(e.target.value)}
              placeholder="e.g. 5000"
              className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
            />
            <p className="text-[11px] text-slate-400 dark:text-slate-500">
              Gateways exceeding this latency are also eliminated. Leave blank to disable.
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
