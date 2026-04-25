import { useState } from 'react'
import { Network } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { useMerchantStore } from '../../store/merchantStore'
import { useDebitRoutingFlag } from '../../hooks/useDebitRoutingFlag'

export function DebitRoutingPage() {
  const { merchantId } = useMerchantStore()
  const {
    data,
    error: flagError,
    isLoading,
    isEnabled,
    setDebitRoutingEnabled,
  } = useDebitRoutingFlag(merchantId)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  async function handleToggle(nextEnabled: boolean) {
    if (!merchantId) {
      setError('Set a merchant ID first')
      return
    }

    setSaving(true)
    setError(null)
    setSuccess(null)

    try {
      const response = await setDebitRoutingEnabled(nextEnabled)
      setSuccess(
        response.debit_routing_enabled
          ? 'Debit routing enabled for this merchant.'
          : 'Debit routing disabled for this merchant.',
      )
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to update debit routing')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="max-w-3xl space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-slate-900 dark:text-white">Network / Debit Routing</h1>
        <p className="mt-1 text-sm text-slate-500 dark:text-[#b2bdd1]">
          Enable debit network routing for a merchant, then test real network-routing decisions from Decision Explorer.
        </p>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <Network size={16} className="text-brand-500" />
            <div>
              <SurfaceLabel>Merchant feature flag</SurfaceLabel>
              <h2 className="mt-2 font-medium text-slate-800 dark:text-white">Debit Routing Runtime Access</h2>
            </div>
          </div>
        </CardHeader>
        <CardBody className="space-y-5">
          {!merchantId && (
            <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-700">
              Set a merchant ID in the top bar to load debit routing access.
            </p>
          )}

          {merchantId && isLoading ? (
            <div className="flex items-center gap-2 py-4 text-sm text-slate-500">
              <Spinner size={16} />
              Loading debit routing flag...
            </div>
          ) : (
            <div className="rounded-[24px] border border-slate-200 bg-slate-50 p-5 dark:border-[#232833] dark:bg-[#0b1017]">
              <div className="flex flex-wrap items-center justify-between gap-4">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400 dark:text-[#6d768a]">
                    Current state
                  </p>
                  <p className="mt-2 text-2xl font-semibold text-slate-900 dark:text-white">
                    {isEnabled ? 'Enabled' : 'Disabled'}
                  </p>
                  <p className="mt-1 text-sm text-slate-500 dark:text-[#9ca7ba]">
                    {data?.merchant_id || merchantId || 'No merchant selected'}
                  </p>
                </div>

                <Button
                  onClick={() => handleToggle(!isEnabled)}
                  disabled={!merchantId || saving || isLoading}
                  variant={isEnabled ? 'secondary' : 'primary'}
                >
                  {saving ? (
                    <>
                      <Spinner size={14} />
                      Updating...
                    </>
                  ) : isEnabled ? (
                    'Disable Debit Routing'
                  ) : (
                    'Enable Debit Routing'
                  )}
                </Button>
              </div>
            </div>
          )}

          <ErrorMessage
            error={
              error ||
              (flagError instanceof Error ? flagError.message : flagError ? 'Failed to load debit routing flag' : null)
            }
          />
          {success && <p className="text-sm text-emerald-500">{success}</p>}
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <h2 className="font-medium text-slate-800 dark:text-white">What This Controls</h2>
        </CardHeader>
        <CardBody className="space-y-3 text-sm text-slate-600 dark:text-[#aab5c8]">
          <p>
            This toggle controls the backend runtime gate for <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-brand-600 dark:bg-[#111118]">NtwBasedRouting</code> and hybrid debit routing.
          </p>
          <p>
            Detailed debit fee tables and network cost configuration are still backend configuration, not dashboard-editable rule config. This page only enables or disables merchant access to the runtime debit-routing flow.
          </p>
          <p>
            Use Decision Explorer&apos;s Debit Routing tab to send a real <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs text-brand-600 dark:bg-[#111118]">/decide-gateway</code> request and inspect the ranked debit networks.
          </p>
        </CardBody>
      </Card>
    </div>
  )
}
