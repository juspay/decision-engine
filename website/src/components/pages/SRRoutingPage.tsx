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
import { Plus, Trash2, Eye } from 'lucide-react'

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

function CurrentConfigDetails({ config }: { config: SRConfigResponse['config'] }) {
  return (
    <div className="text-xs text-slate-600 dark:text-[#b2bdd1] space-y-4">
      <div className="border-b border-slate-200 pb-3 dark:border-[#222226]">
        <h3 className="font-medium text-slate-700 mb-2 dark:text-slate-200">Default Settings</h3>
        <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
          <div>
            <span className="text-slate-500">Bucket Size:</span>
            <p className="font-medium">{config.data.defaultBucketSize}</p>
          </div>
          <div>
            <span className="text-slate-500">Hedging %:</span>
            <p className="font-medium">{config.data.defaultHedgingPercent ?? 'Not set'}</p>
          </div>
          <div>
            <span className="text-slate-500">Latency Threshold:</span>
            <p className="font-medium">{config.data.defaultLatencyThreshold ?? 'Not set'} ms</p>
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
                    <span className="text-slate-500">Latency Threshold:</span>
                    <p className="font-medium">{subConfig.latencyThreshold ?? 'Default'} ms</p>
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

  const { data: existing, isLoading, mutate } = useSWR<SRConfigResponse>(
    merchantId ? ['rule-sr', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, algorithm: 'successRate' }),
    { shouldRetryOnError: false }
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
      await ensureMerchantExists()
      const endpoint = existing ? '/rule/update' : '/rule/create'
      await apiPost(endpoint, {
        merchant_id: merchantId,
        config: {
          type: 'successRate',
          data: {
            defaultBucketSize: data.defaultBucketSize,
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
        <form onSubmit={handleSubmit(onSave)} className="space-y-6">
          <Card>
            <CardHeader>
              <div>
                <h2 className="text-sm font-semibold text-slate-800">Default Success Rate Config</h2>
                <p className="text-xs text-slate-500 mt-0.5">
                  Base settings used when there is no payment-method-specific override.
                </p>
              </div>
            </CardHeader>
            <CardBody className="grid gap-4 md:grid-cols-3">
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
              </label>

              <label className="space-y-1">
                <span className="text-xs text-slate-500">Latency Threshold (ms)</span>
                <input
                  type="number"
                  {...register('defaultLatencyThreshold')}
                  placeholder="null"
                  className="border border-slate-200 dark:border-[#222226] bg-transparent rounded-lg px-3 py-2 w-full focus:outline-none focus:ring-1 focus:ring-brand-500"
                />
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
                      <th className="px-4 py-2">Latency Threshold (ms)</th>
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
      )}
    </div>
  )
}
