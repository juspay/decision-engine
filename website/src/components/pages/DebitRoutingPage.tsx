import { useState } from 'react'
import useSWR from 'swr'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { apiPost } from '../../lib/api'
import { DebitRoutingData, CreateRuleRequest } from '../../types/api'
import { Network } from 'lucide-react'

interface RuleConfigResponse {
  merchant_id: string
  config: { type: string; data: DebitRoutingData }
}

export function DebitRoutingPage() {
  const { merchantId } = useMerchantStore()

  const { data: existing, mutate, isLoading } = useSWR<RuleConfigResponse>(
    merchantId ? ['rule-debit', merchantId] : null,
    () => apiPost('/rule/get', { merchant_id: merchantId, config: { type: 'debitRouting' } })
  )

  const [mcc, setMcc] = useState('')
  const [country, setCountry] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  // Pre-fill from fetched config
  const current = existing?.config?.data
  const displayMcc = mcc || current?.merchant_category_code || ''
  const displayCountry = country || current?.acquirer_country || ''

  async function handleSave() {
    if (!merchantId) return setError('Set a merchant ID first')
    const payload: CreateRuleRequest = {
      merchant_id: merchantId,
      config: {
        type: 'debitRouting',
        data: {
          merchant_category_code: displayMcc.trim(),
          acquirer_country: displayCountry.trim(),
        } as DebitRoutingData,
      },
    }
    setSaving(true); setError(null)
    try {
      await apiPost(existing ? '/rule/update' : '/rule/create', payload)
      setSuccess('Debit routing config saved.')
      mutate()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to save')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-6 max-w-2xl">
      <div>
        <h1 className="text-2xl font-bold text-gray-900">Network / Debit Routing</h1>
        <p className="text-gray-500 mt-1 text-sm">
          Configure network-based routing to optimise processing fees for debit card transactions.
          The engine selects the cheapest eligible network (Visa, Mastercard, ACCEL, NYCE, PULSE, STAR).
        </p>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <Network size={16} className="text-brand-500" />
            <h2 className="font-medium text-gray-800">Debit Routing Configuration</h2>
          </div>
        </CardHeader>
        <CardBody className="space-y-4">
          {isLoading ? (
            <div className="flex justify-center py-6"><Spinner /></div>
          ) : (
            <>
              {!merchantId && (
                <p className="text-sm text-amber-600 bg-amber-50 border border-amber-200 rounded px-3 py-2">
                  Set a merchant ID in the top bar to load configuration.
                </p>
              )}

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Merchant Category Code (MCC)
                  </label>
                  <input
                    value={displayMcc}
                    onChange={e => setMcc(e.target.value)}
                    placeholder="e.g. 5411"
                    className="w-full border border-gray-300 rounded px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  />
                  <p className="text-xs text-gray-400 mt-1">4-digit ISO MCC for your business type</p>
                </div>

                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    Acquirer Country
                  </label>
                  <input
                    value={displayCountry}
                    onChange={e => setCountry(e.target.value)}
                    placeholder="e.g. US"
                    className="w-full border border-gray-300 rounded px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-brand-500"
                  />
                  <p className="text-xs text-gray-400 mt-1">ISO 3166-1 alpha-2 country code</p>
                </div>
              </div>

              <ErrorMessage error={error} />
              {success && <p className="text-sm text-emerald-400">{success}</p>}

              <Button onClick={handleSave} disabled={saving || !merchantId}>
                {saving ? <><Spinner size={14} /> Saving…</> : (existing ? 'Update Config' : 'Save Config')}
              </Button>
            </>
          )}
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <h2 className="font-medium text-gray-800">How Network Routing Works</h2>
        </CardHeader>
        <CardBody className="text-sm text-gray-600 space-y-2">
          <p>For co-badged debit cards (e.g. Visa/NYCE, Mastercard/PULSE), the engine evaluates all eligible networks and routes to the one with the lowest processing fee.</p>
          <p>Supported networks: {['VISA','MASTERCARD','ACCEL','NYCE','PULSE','STAR'].map(n => <span key={n} className="font-mono text-xs bg-[#111118] border border-[#1c1c24] px-1.5 py-0.5 rounded-md mr-1 text-gray-700">{n}</span>)}</p>
          <p>Use the <strong className="text-gray-800">Decision Explorer</strong> to test network routing decisions with <code className="text-xs bg-[#111118] border border-[#1c1c24] px-1.5 py-0.5 rounded-md text-brand-500">NtwBasedRouting</code> algorithm.</p>
        </CardBody>
      </Card>
    </div>
  )
}
