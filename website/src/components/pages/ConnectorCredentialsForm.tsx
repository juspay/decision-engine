import { useState } from 'react'
import { ShieldCheck } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import { setConnectorCredentials, useConnectorSources } from '../../hooks/useCostRouting'
import { Field, inputClass } from './CostRoutingShared'

/**
 * Automatic-ingestion tab: store a connector's webhook secret + report-download auth (encrypted at
 * rest) so the webhook worker can pull settlement reports. Lists the (connector, account) pairs
 * already configured.
 */
export function ConnectorCredentialsForm({ merchantId }: { merchantId?: string }) {
  const { sources, mutate: mutateSources } = useConnectorSources(merchantId)

  const [connector, setConnector] = useState('adyen')
  const [account, setAccount] = useState('')
  const [webhookSecret, setWebhookSecret] = useState('')
  const [downloadAuth, setDownloadAuth] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  async function handleSave() {
    if (!merchantId) {
      setError('Set a merchant ID first')
      return
    }
    if (!account || !webhookSecret || !downloadAuth) {
      setError('Account, webhook secret and download auth are all required')
      return
    }
    setSaving(true)
    setError(null)
    setSuccess(null)
    try {
      await setConnectorCredentials(merchantId, connector, {
        account,
        webhook_secret: webhookSecret,
        download_auth: downloadAuth,
      })
      setSuccess(`Saved credentials for ${connector} / ${account}.`)
      setWebhookSecret('')
      setDownloadAuth('')
      await mutateSources()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to save credentials')
    } finally {
      setSaving(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center gap-2">
          <ShieldCheck size={16} className="text-brand-500" />
          <div>
            <SurfaceLabel>Connector credentials</SurfaceLabel>
            <h2 className="mt-2 font-medium text-slate-800 dark:text-white">
              Settlement report access
            </h2>
          </div>
        </div>
      </CardHeader>
      <CardBody className="space-y-4">
        <Field label="Connector">
          <select
            className={inputClass}
            value={connector}
            onChange={(e) => setConnector(e.target.value)}
          >
            <option value="adyen">Adyen</option>
          </select>
        </Field>
        <Field label="Account" hint="Connector-side account (e.g. Adyen merchantAccountCode)">
          <input
            className={inputClass}
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            placeholder="AcmeMerchantEU"
          />
        </Field>
        <Field label="Webhook secret" hint="Used to verify inbound webhook signatures (HMAC key)">
          <input
            className={inputClass}
            type="password"
            value={webhookSecret}
            onChange={(e) => setWebhookSecret(e.target.value)}
            placeholder="••••••••"
          />
        </Field>
        <Field label="Report download auth" hint="e.g. reportuser:password">
          <input
            className={inputClass}
            type="password"
            value={downloadAuth}
            onChange={(e) => setDownloadAuth(e.target.value)}
            placeholder="••••••••"
          />
        </Field>

        <div className="flex items-center gap-3">
          <Button onClick={handleSave} disabled={!merchantId || saving}>
            {saving ? (
              <>
                <Spinner size={14} />
                Saving...
              </>
            ) : (
              'Save credentials'
            )}
          </Button>
          <span className="text-xs text-slate-400">Secrets are encrypted at rest.</span>
        </div>

        <ErrorMessage error={error} />
        {success && <p className="text-sm text-emerald-500">{success}</p>}

        {sources.length > 0 && (
          <div className="mt-2 border-t border-slate-100 pt-4 dark:border-[#232833]">
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
              Configured
            </p>
            <ul className="mt-2 space-y-1 text-sm text-slate-600 dark:text-[#9ca7ba]">
              {sources.map((s) => (
                <li key={`${s.connector}:${s.account}`}>
                  {s.connector} · {s.account}
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardBody>
    </Card>
  )
}
