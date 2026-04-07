import useSWR from 'swr'
import { useForm, useFieldArray } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { useEffect, useState } from 'react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { PAYMENT_METHOD_TYPES, PAYMENT_METHODS } from '../../lib/constants'
import { Plus, Trash2 } from 'lucide-react'

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

export function SRRoutingPage() {
  const { merchantId } = useMerchantStore()
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [saveSuccess, setSaveSuccess] = useState(false)

  const { data: existing, isLoading, mutate } = useSWR<SRConfigResponse>(
    merchantId ? ['rule-sr', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, config: { type: 'successRate' } }),
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
      defaultBucketSize: 20,
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
        defaultBucketSize: d.defaultBucketSize ?? 20,
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
            defaultLatencyThreshold: data.defaultLatencyThreshold,
            defaultHedgingPercent: data.defaultHedgingPercent,
            defaultLowerResetFactor: null,
            defaultUpperResetFactor: null,
            defaultGatewayExtraScore: null,
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
          <Button type="submit" disabled={saving || !merchantId}>
            {saving ? <><Spinner size={14} /> Saving…</> : 'Save Configuration'}
          </Button>
        </form>
      )}
    </div>
  )
}
