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

export function SRRoutingPage() {
  const { merchantId } = useMerchantStore()
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)
  const [showCurrentConfig, setShowCurrentConfig] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)

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
      reset({
        defaultBucketSize: d.defaultBucketSize ?? 200,
        defaultSuccessRate: d.defaultSuccessRate ?? 0.5,
        defaultLatencyThreshold: d.defaultLatencyThreshold ?? null,
        defaultHedgingPercent: d.defaultHedgingPercent ?? null,
        subLevelInputConfig: d.subLevelInputConfig ?? [],
      })
    }
  }, [existing, reset])

  const { fields, append, remove } = useFieldArray({ control, name: 'subLevelInputConfig' })
  const watchedRows = watch('subLevelInputConfig')

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
            defaultSuccessRate: data.defaultSuccessRate,
            defaultLatencyThreshold: data.defaultLatencyThreshold,
            defaultHedgingPercent: data.defaultHedgingPercent,
            subLevelInputConfig: data.subLevelInputConfig.length > 0
              ? data.subLevelInputConfig
              : null,
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

  async function handleDelete() {
    if (!merchantId) return
    setDeleting(true); setDeleteError(null)
    try {
      await apiPost('/rule/delete', { merchant_id: merchantId, algorithm: 'successRate' })
      mutate(undefined, { revalidate: false })
    } catch (err: unknown) {
      setDeleteError(err instanceof Error ? err.message : String(err))
    } finally {
      setDeleting(false)
    }
  }

  return (
    <div className="space-y-6 max-w-5xl">
      <div>
        <h1 className="text-2xl font-semibold text-gray-900">Auth-Rate Based Routing</h1>
        <p className="text-sm text-gray-500 mt-1">
          Configure success-rate based gateway routing
        </p>
      </div>

      {!merchantId && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 px-4 py-3 text-sm text-yellow-800">
          Set a Merchant ID in the top bar to load and save configuration.
        </div>
      )}

      {/* Status Card */}
      {merchantId && !isLoading && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold text-gray-800">Configuration Status</h2>
              <p className="text-xs text-gray-500 mt-0.5">
                {existing?.config?.data 
                  ? 'Success Rate routing is configured and active' 
                  : 'No Success Rate configuration found'}
              </p>
            </div>
            <Badge variant={existing?.config?.data ? 'green' : 'gray'}>
              {existing?.config?.data ? 'Active' : 'Not Configured'}
            </Badge>
          </CardHeader>
          {existing?.config?.data && (
            <CardBody className="border-t border-gray-100">
              <div className="flex items-center justify-between text-xs text-gray-600">
                <div>
                  <span className="text-gray-500">Last Modified:</span>
                  <span className="ml-1 font-medium">
                    {existing.modified_at 
                      ? new Date(existing.modified_at).toLocaleString() 
                      : 'Unknown'}
                  </span>
                </div>
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
              </div>
              {deleteError && (
                <p className="text-xs text-red-500 mt-2">{deleteError}</p>
              )}
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
              <h2 className="text-sm font-semibold text-gray-800">Success Rate Config</h2>
            </CardHeader>
            <CardBody className="overflow-x-auto p-0">
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-xs text-gray-500 border-b border-[#1c1c24] bg-[#0a0a0f]">
                    <th className="px-4 py-2">Payment Method Type</th>
                    <th className="px-4 py-2">Payment Method</th>
                    <th className="px-4 py-2">Bucket Size</th>
                    <th className="px-4 py-2">Success Rate</th>
                    <th className="px-4 py-2">Hedging %</th>
                    <th className="px-4 py-2">Latency Threshold (ms)</th>
                    <th className="px-4 py-2" />
                  </tr>
                </thead>
                <tbody>
                  {/* Default row */}
                  <tr className="border-b border-[#1c1c24] bg-brand-50/50">
                    <td className="px-4 py-2 text-gray-500 italic">Default</td>
                    <td className="px-4 py-2 text-gray-400">—</td>
                    <td className="px-4 py-2">
                      <input
                        type="number"
                        {...register('defaultBucketSize')}
                        className="border border-gray-300 rounded px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                      {errors.defaultBucketSize && (
                        <p className="text-xs text-red-500 mt-0.5">{errors.defaultBucketSize.message}</p>
                      )}
                    </td>
                    <td className="px-4 py-2">
                      <input
                        type="number"
                        step="0.1"
                        min="0"
                        max="1"
                        {...register('defaultSuccessRate')}
                        placeholder="0.5"
                        className="border border-gray-300 rounded px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                    </td>
                    <td className="px-4 py-2">
                      <input
                        type="number"
                        step="0.1"
                        {...register('defaultHedgingPercent')}
                        placeholder="null"
                        className="border border-gray-300 rounded px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                    </td>
                    <td className="px-4 py-2">
                      <input
                        type="number"
                        {...register('defaultLatencyThreshold')}
                        placeholder="null"
                        className="border border-gray-300 rounded px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500"
                      />
                    </td>
                    <td className="px-4 py-2" />
                  </tr>

                  {fields.map((field, idx) => {
                    const methodType = watchedRows?.[idx]?.paymentMethodType || ''
                    const methodOptions = PAYMENT_METHODS[methodType] || []
                    return (
                      <tr key={field.id} className="border-b border-[#1c1c24] hover:bg-[#0f0f16] transition-colors">
                        <td className="px-4 py-2">
                          <select
                            {...register(`subLevelInputConfig.${idx}.paymentMethodType`)}
                            className="border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                          >
                            {PAYMENT_METHOD_TYPES.map((t) => (
                              <option key={t} value={t}>{t}</option>
                            ))}
                          </select>
                        </td>
                        <td className="px-4 py-2">
                          <select
                            {...register(`subLevelInputConfig.${idx}.paymentMethod`)}
                            className="border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
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
                            className="border border-gray-300 rounded px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                          />
                        </td>
                        <td className="px-4 py-2">
                          <input
                            type="number"
                            step="0.1"
                            {...register(`subLevelInputConfig.${idx}.hedgingPercent`)}
                            placeholder="null"
                            className="border border-gray-300 rounded px-2 py-1 w-20 focus:outline-none focus:ring-1 focus:ring-brand-500"
                          />
                        </td>
                        <td className="px-4 py-2">
                          <input
                            type="number"
                            {...register(`subLevelInputConfig.${idx}.latencyThreshold`)}
                            placeholder="null"
                            className="border border-gray-300 rounded px-2 py-1 w-24 focus:outline-none focus:ring-1 focus:ring-brand-500"
                          />
                        </td>
                        <td className="px-4 py-2">
                          <button type="button" onClick={() => remove(idx)} className="text-gray-400 hover:text-red-500">
                            <Trash2 size={14} />
                          </button>
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
              <div className="px-4 py-3">
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  onClick={() => append({ paymentMethodType: 'card', paymentMethod: 'credit', bucketSize: 20, hedgingPercent: null, latencyThreshold: null })}
                >
                  <Plus size={14} /> Add Level
                </Button>
              </div>
            </CardBody>
          </Card>

          <ErrorMessage error={saveError} />
          {saveSuccess && (
            <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/8 px-4 py-3 text-sm text-emerald-400">
              Configuration saved successfully.
            </div>
          )}
          
          {/* View Current Configuration Section */}
          {existing?.config?.data && (
            <Card>
              <CardHeader className="flex flex-row items-center justify-between">
                <h2 className="text-sm font-semibold text-gray-800">Current Active Configuration</h2>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => setShowCurrentConfig(!showCurrentConfig)}
                >
                  <Eye size={14} className="mr-1" />
                  {showCurrentConfig ? 'Hide' : 'View'}
                </Button>
              </CardHeader>
              {showCurrentConfig && (
                <CardBody>
                  <div className="text-xs text-gray-600 space-y-4">
                    {/* Default Config */}
                    <div className="border-b border-gray-200 pb-3">
                      <h3 className="font-medium text-gray-700 mb-2">Default Settings</h3>
                      <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                        <div>
                          <span className="text-gray-500">Bucket Size:</span>
                          <p className="font-medium">{existing.config.data.defaultBucketSize}</p>
                        </div>
                        <div>
                          <span className="text-gray-500">Success Rate:</span>
                          <p className="font-medium">{existing.config.data.defaultSuccessRate ?? 'Not set'}</p>
                        </div>
                        <div>
                          <span className="text-gray-500">Hedging %:</span>
                          <p className="font-medium">{existing.config.data.defaultHedgingPercent ?? 'Not set'}</p>
                        </div>
                        <div>
                          <span className="text-gray-500">Latency Threshold:</span>
                          <p className="font-medium">{existing.config.data.defaultLatencyThreshold ?? 'Not set'} ms</p>
                        </div>
                      </div>
                    </div>
                    
                    {/* Sub-level Configs */}
                    {existing.config.data.subLevelInputConfig && existing.config.data.subLevelInputConfig.length > 0 && (
                      <div>
                        <h3 className="font-medium text-gray-700 mb-2">Sub-Level Configurations</h3>
                        <div className="space-y-2">
                          {existing.config.data.subLevelInputConfig.map((config, idx) => (
                            <div key={idx} className="bg-gray-50 rounded-lg p-3">
                              <div className="grid grid-cols-2 md:grid-cols-5 gap-2 text-xs">
                                <div>
                                  <span className="text-gray-500">Payment Type:</span>
                                  <p className="font-medium capitalize">{config.paymentMethodType}</p>
                                </div>
                                <div>
                                  <span className="text-gray-500">Payment Method:</span>
                                  <p className="font-medium">{config.paymentMethod}</p>
                                </div>
                                <div>
                                  <span className="text-gray-500">Bucket Size:</span>
                                  <p className="font-medium">{config.bucketSize}</p>
                                </div>
                                <div>
                                  <span className="text-gray-500">Hedging %:</span>
                                  <p className="font-medium">{config.hedgingPercent ?? 'Default'}</p>
                                </div>
                                <div>
                                  <span className="text-gray-500">Latency Threshold:</span>
                                  <p className="font-medium">{config.latencyThreshold ?? 'Default'} ms</p>
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    
                    {/* Raw JSON */}
                    <div className="border-t border-gray-200 pt-3">
                      <h3 className="font-medium text-gray-700 mb-2">Raw Configuration (JSON)</h3>
                      <pre className="bg-gray-900 text-gray-100 rounded-lg p-3 text-xs overflow-auto max-h-64">
                        {JSON.stringify(existing.config, null, 2)}
                      </pre>
                    </div>
                  </div>
                </CardBody>
              )}
            </Card>
          )}
          
          <Button type="submit" disabled={saving || !merchantId}>
            {saving ? <><Spinner size={14} /> Saving…</> : 'Save Configuration'}
          </Button>
        </form>
      )}
    </div>
  )
}
