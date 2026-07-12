import { useState } from 'react'
import { Pencil, ShieldCheck, Trash2 } from 'lucide-react'
import { Card, CardBody, CardHeader, SurfaceLabel } from '../ui/Card'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Spinner } from '../ui/Spinner'
import {
  deleteConnectorCredentials,
  setConnectorCredentials,
  useConnectorSources,
  type ConnectorSource,
} from '../../hooks/useCostRouting'
import { Field, inputClass } from './CostRoutingShared'

/**
 * Automatic-ingestion tab: store a connector's report-access credentials (encrypted at rest) so the
 * ingest worker can pull settlement reports. Two credential shapes:
 *  - webhook connectors (Adyen, Checkout): a webhook signing secret + report-download auth.
 *  - Chase (J.P. Morgan): OAuth2 client-assertion credentials — no webhook — packed as a JSON blob
 *    into `download_auth` (the pull poller reads it).
 * Lists the (connector, account) pairs already configured.
 */
export function ConnectorCredentialsForm({ merchantId }: { merchantId?: string }) {
  const { sources, mutate: mutateSources } = useConnectorSources(merchantId)

  const [connector, setConnector] = useState('adyen')
  const [account, setAccount] = useState('')
  const [webhookSecret, setWebhookSecret] = useState('')
  const [downloadAuth, setDownloadAuth] = useState('')
  // When set, the form is editing this existing source: the account is its identity (locked), a
  // save upserts its secrets, and its masked hints show what's currently stored.
  const [editing, setEditing] = useState<ConnectorSource | null>(null)
  // Chase OAuth fields (used only when connector === 'chase').
  const [clientId, setClientId] = useState('')
  const [resource, setResource] = useState('')
  const [privateKeyPem, setPrivateKeyPem] = useState('')
  const [tokenUrl, setTokenUrl] = useState('')
  const [reportsUrl, setReportsUrl] = useState('')
  // Local/mock testing: a pre-supplied bearer token that skips OAuth (the mock/local stub ignores
  // it). When set, the OAuth fields aren't required.
  const [accessToken, setAccessToken] = useState('')
  const [saving, setSaving] = useState(false)
  const [deleting, setDeleting] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState<string | null>(null)

  function resetSecrets() {
    setWebhookSecret('')
    setDownloadAuth('')
    setPrivateKeyPem('')
    setAccessToken('')
  }

  function resetForm() {
    setEditing(null)
    setAccount('')
    resetSecrets()
  }

  const isChase = connector === 'chase'
  const isCheckout = connector === 'checkout'

  async function handleSave() {
    if (!merchantId) {
      setError('Set a merchant ID first')
      return
    }

    // Build the (webhook_secret, download_auth) pair for this connector's credential shape.
    let payload: { account: string; webhook_secret: string; download_auth: string }
    if (isChase) {
      // Two credential modes: real OAuth (client id + resource + private key) or a pre-supplied
      // access token for local/mock testing. Exactly one must be complete.
      const hasOAuth = clientId && resource && privateKeyPem
      if (!account || (!accessToken && !hasOAuth)) {
        setError(
          'Chase needs an Account (EntityId) plus either an Access token (testing) or all of Client ID, Resource and Private key',
        )
        return
      }
      // Chase has no webhook; the OAuth credentials ride in download_auth as JSON. Only include the
      // optional fields when set, so the backend defaults apply otherwise.
      const creds: Record<string, string> = {}
      if (clientId) creds.client_id = clientId
      if (resource) creds.resource = resource
      if (privateKeyPem) creds.private_key_pem = privateKeyPem
      if (accessToken) creds.access_token = accessToken
      if (tokenUrl) creds.token_url = tokenUrl
      if (reportsUrl) creds.reports_url = reportsUrl
      payload = { account, webhook_secret: '', download_auth: JSON.stringify(creds) }
    } else {
      if (!account || !webhookSecret || !downloadAuth) {
        setError('Account, webhook secret and download auth are all required')
        return
      }
      payload = { account, webhook_secret: webhookSecret, download_auth: downloadAuth }
    }

    setSaving(true)
    setError(null)
    setSuccess(null)
    try {
      await setConnectorCredentials(merchantId, connector, payload)
      setSuccess(`Saved credentials for ${connector} / ${account}.`)
      resetForm()
      await mutateSources()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to save credentials')
    } finally {
      setSaving(false)
    }
  }

  // Secrets are write-only (never returned), so editing loads the identity and clears the secret
  // fields — the user re-enters them, and the save overwrites the stored blob.
  function handleEdit(s: ConnectorSource) {
    setEditing(s)
    setConnector(s.connector)
    setAccount(s.account)
    setWebhookSecret('')
    setDownloadAuth('')
    setError(null)
    setSuccess(null)
  }

  async function handleDelete(s: ConnectorSource) {
    if (!merchantId) return
    if (!window.confirm(`Delete credentials for ${s.connector} / ${s.account}?`)) return
    const key = `${s.connector}:${s.account}`
    setDeleting(key)
    setError(null)
    setSuccess(null)
    try {
      await deleteConnectorCredentials(merchantId, s.connector, s.account)
      if (editing && account === s.account && connector === s.connector) resetForm()
      await mutateSources()
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to delete credentials')
    } finally {
      setDeleting(null)
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
            onChange={(e) => {
              setConnector(e.target.value)
              setError(null)
            }}
          >
            <option value="adyen">Adyen</option>
            <option value="chase">Chase (J.P. Morgan)</option>
            <option value="checkout">Checkout</option>
          </select>
        </Field>
        <Field
          label="Account"
          hint={
            editing
              ? 'Editing an existing source — account is locked'
              : isChase
                ? 'J.P. Morgan EntityId'
                : isCheckout
                  ? 'Checkout entity id the report relates to (ent_…)'
                  : 'Connector-side account (e.g. Adyen merchantAccountCode)'
          }
        >
          <input
            className={inputClass}
            value={account}
            onChange={(e) => setAccount(e.target.value)}
            placeholder={
              isChase ? '418553' : isCheckout ? 'ent_r7nge7vl53crsa3ozjxzoiykj4' : 'AcmeMerchantEU'
            }
            disabled={editing !== null}
          />
        </Field>

        {isChase ? (
          <>
            <Field label="Client ID" hint="OAuth client id issued by J.P. Morgan">
              <input
                className={inputClass}
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
                placeholder="CC-100000-A0000000"
              />
            </Field>
            <Field label="Resource" hint="OAuth resource issued by J.P. Morgan">
              <input
                className={inputClass}
                value={resource}
                onChange={(e) => setResource(e.target.value)}
                placeholder="JPMC:URI:RS-000000-00-..."
              />
            </Field>
            <Field label="Private key (PEM)" hint="RSA private key that signs the JWT client assertion">
              <textarea
                className={`${inputClass} h-28 font-mono text-xs`}
                value={privateKeyPem}
                onChange={(e) => setPrivateKeyPem(e.target.value)}
                placeholder={'-----BEGIN PRIVATE KEY-----\n...'}
              />
            </Field>
            <Field label="Token URL (optional)" hint="Defaults to the J.P. Morgan production token endpoint">
              <input
                className={inputClass}
                value={tokenUrl}
                onChange={(e) => setTokenUrl(e.target.value)}
                placeholder="https://idag2.jpmorganchase.com/adfs/oauth2/token"
              />
            </Field>
            <Field label="Reports URL (optional)" hint="Defaults to production; use the mock or local stub host to test">
              <input
                className={inputClass}
                value={reportsUrl}
                onChange={(e) => setReportsUrl(e.target.value)}
                placeholder="https://api.reports.jpmorgan.com/api/v1/reports"
              />
            </Field>
            <Field
              label="Access token (testing only)"
              hint="Skips OAuth — for the mock or local stub, which ignore token validity. Leave blank for real J.P. Morgan auth."
            >
              <input
                className={inputClass}
                value={accessToken}
                onChange={(e) => setAccessToken(e.target.value)}
                placeholder="test"
              />
            </Field>
          </>
        ) : (
          <>
            <Field
              label="Webhook secret"
              hint={
                editing
                  ? 'Enter a new value to replace the stored secret'
                  : isCheckout
                    ? 'Checkout webhook signature key (HMAC-SHA256 over the raw body, sent as Cko-Signature)'
                    : 'Used to verify inbound webhook signatures (HMAC key)'
              }
            >
              <input
                className={inputClass}
                type="password"
                value={webhookSecret}
                onChange={(e) => setWebhookSecret(e.target.value)}
                placeholder={editing?.webhook_secret_hint || '••••••••'}
              />
            </Field>
            <Field
              label="Report download auth"
              hint={
                editing
                  ? 'Enter a new value to replace the stored auth'
                  : isCheckout
                    ? 'Checkout secret key (sk_…). Or JSON {"secret_key":"sk_…","api_base_url":"…"} for a sandbox/regional host.'
                    : 'Report-user Basic auth as user:password, or a Report Service API key on its own'
              }
            >
              <input
                className={inputClass}
                type="password"
                value={downloadAuth}
                onChange={(e) => setDownloadAuth(e.target.value)}
                placeholder={editing?.download_auth_hint || '••••••••'}
              />
            </Field>
          </>
        )}

        <div className="flex items-center gap-3">
          <Button onClick={handleSave} disabled={!merchantId || saving}>
            {saving ? (
              <>
                <Spinner size={14} />
                Saving...
              </>
            ) : editing ? (
              'Update credentials'
            ) : (
              'Save credentials'
            )}
          </Button>
          {editing && (
            <Button variant="ghost" size="sm" onClick={resetForm} disabled={saving}>
              Cancel
            </Button>
          )}
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
              {sources.map((s) => {
                const key = `${s.connector}:${s.account}`
                return (
                  <li key={key} className="flex items-center justify-between gap-2 py-0.5">
                    <span>
                      {s.connector} · {s.account}
                    </span>
                    <span className="flex items-center gap-1">
                      <button
                        type="button"
                        title="Edit credentials"
                        onClick={() => handleEdit(s)}
                        className="rounded p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-[#121214] dark:hover:text-white"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        type="button"
                        title="Delete credentials"
                        disabled={deleting === key}
                        onClick={() => handleDelete(s)}
                        className="rounded p-1 text-slate-400 hover:bg-red-50 hover:text-red-500 disabled:opacity-50 dark:hover:bg-red-950/30"
                      >
                        {deleting === key ? <Spinner size={14} /> : <Trash2 size={14} />}
                      </button>
                    </span>
                  </li>
                )
              })}
            </ul>
          </div>
        )}
      </CardBody>
    </Card>
  )
}
