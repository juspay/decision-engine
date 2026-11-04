import { useState } from 'react'
import useSWR from 'swr'
import { Key, Copy, Check, Trash2, Plus, Info } from 'lucide-react'
import { apiFetch, apiPost, fetcher } from '../lib/api'
import { merchantLabel, useMerchantScope } from '../lib/merchantScope'
import { useMerchantStore } from '../store/merchantStore'
import { Card, CardBody, CardHeader } from '../components/ui/Card'
import { Button } from '../components/ui/Button'
import { ErrorMessage } from '../components/ui/ErrorMessage'
import { ConfirmDialog } from '../components/ui/ConfirmDialog'

import { PageHeading } from '../components/ui/PageHeading'
interface ApiKeyListItem {
  key_id: string
  key_prefix: string
  merchant_id: string
  description: string | null
  is_active: boolean
  created_at: string
}

interface CreateApiKeyResponse {
  key_id: string
  api_key: string
  key_prefix: string
  merchant_id: string
  description: string | null
  created_at: string
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  async function copy() {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 1800)
    } catch {
      // clipboard unavailable
    }
  }
  return (
    <button
      onClick={copy}
      className="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-2.5 py-1 text-xs font-medium text-slate-600 transition hover:border-slate-300 hover:text-slate-900 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-slate-300 dark:hover:text-white"
      title={copied ? 'Copied' : 'Copy key'}
    >
      {copied ? <Check size={12} className="text-emerald-700" /> : <Copy size={12} />}
      {copied ? 'Copied' : 'Copy'}
    </button>
  )
}

/** A line of context above the keys: whose keys they are, and what they reach. */
function KeysNote({ children }: { children: React.ReactNode }) {
  return (
    <Card>
      <CardBody>
        <div className="flex items-start gap-3 text-sm text-slate-600 dark:text-[#8a8a93]">
          <Info size={16} className="mt-0.5 shrink-0 text-slate-500" />
          <div className="space-y-2 leading-6">{children}</div>
        </div>
      </CardBody>
    </Card>
  )
}

function Code({ children }: { children: React.ReactNode }) {
  return (
    <code className="rounded bg-slate-100 px-1 py-0.5 text-xs font-mono dark:bg-[#1c1c24]">
      {children}
    </code>
  )
}

export function ApiKeysPage() {
  const { merchantId } = useMerchantStore()
  const { merchant, isSubMerchant, isHyperswitch, isAdmin } = useMerchantScope()
  // On a sub-merchant, every call below is answered with its parent's keys: the only ones there are,
  // and the ones that work for it.
  const merchantName = merchantLabel(merchant, 'this merchant')

  const [description, setDescription] = useState('')
  const [creating, setCreating] = useState(false)
  const [newKey, setNewKey] = useState<CreateApiKeyResponse | null>(null)
  const [createError, setCreateError] = useState<string | null>(null)
  const [revokingId, setRevokingId] = useState<string | null>(null)
  const [revokeError, setRevokeError] = useState<string | null>(null)
  const [revokedIds, setRevokedIds] = useState<Set<string>>(new Set())
  const [confirmRevoke, setConfirmRevoke] = useState<ApiKeyListItem | null>(null)

  const { data: keys, mutate } = useSWR<ApiKeyListItem[]>(
    merchantId && !isHyperswitch ? `/api-key/list/${merchantId}` : null,
    fetcher,
  )

  async function handleCreate() {
    if (!merchantId) return
    setCreating(true)
    setCreateError(null)
    setNewKey(null)
    try {
      const result = await apiPost<CreateApiKeyResponse>('/api-key/create', {
        merchant_id: merchantId,
        description: description.trim() || null,
      })
      setNewKey(result)
      setDescription('')
      await mutate()
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : 'Failed to create API key')
    } finally {
      setCreating(false)
    }
  }

  async function handleRevoke(keyId: string) {
    setRevokingId(keyId)
    setRevokeError(null)
    try {
      await apiFetch(`/api-key/${keyId}`, { method: 'DELETE' })
      setRevokedIds((prev) => new Set([...prev, keyId]))
      if (newKey?.key_id === keyId) setNewKey(null)
      mutate()
    } catch (e) {
      setRevokeError(e instanceof Error ? e.message : 'Failed to revoke key')
    } finally {
      setRevokingId(null)
    }
  }

  const activeKeys = keys?.filter((k) => k.is_active && !revokedIds.has(k.key_id)) ?? []
  const heading = (
    <div>
      <PageHeading title="API Keys" description={<>Create and manage API keys for programmatic access to the routing engine. Pass the key via the{' '} <Code>x-api-key</Code>{' '} header on all API requests.</>} />
    </div>
  )

  if (isHyperswitch) {
    return (
      <div className="space-y-6">
        {heading}
        <KeysNote>
          <p>Hyperswitch accounts do not use API keys. Hyperswitch calls Decision Engine directly.</p>
        </KeysNote>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {heading}

      <KeysNote>
        <p>
          {isSubMerchant ? (
            <>
              These are <span className="font-medium text-slate-800 dark:text-white">{merchantName}</span>'s
              keys. A key works for {merchantName} and every one of its sub-merchants, including this
              one. Send this sub-merchant's <Code>merchant_id</Code>, <Code>{merchantId}</Code>, to
              route with its rules.
            </>
          ) : (
            <>
              A key works for this merchant and every one of its sub-merchants, including ones added
              later. Send the <Code>merchant_id</Code> of whichever one a request is for.
            </>
          )}
          {!isAdmin && ' Only admins can create or revoke keys.'}
        </p>
      </KeysNote>

      {/* Create new key */}
      {isAdmin && (
      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <Plus size={15} />
            <span className="font-medium text-slate-800 dark:text-white">Create API Key</span>
          </div>
        </CardHeader>
        <CardBody className="space-y-4">
          <div className="flex gap-3">
            <input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
              placeholder="Description (optional)"
              className="flex-1 rounded-lg border border-slate-200 bg-transparent px-3 py-1.5 text-sm focus:outline-none focus:border-brand-500 dark:border-[#222226]"
            />
            <Button onClick={handleCreate} disabled={creating || !merchantId}>
              {creating ? 'Creating…' : 'Create API Key'}
            </Button>
          </div>

          <ErrorMessage error={createError} />

          {newKey && (
            <div className="rounded-xl border border-emerald-200 bg-emerald-50 p-4 dark:border-emerald-800/40 dark:bg-emerald-950/20">
              <p className="mb-2 text-xs font-semibold text-emerald-700 dark:text-emerald-400">
                API key created — copy it now. It will not be shown again.
              </p>
              <div className="flex items-center gap-3 rounded-lg border border-emerald-200 bg-white px-3 py-2 font-mono text-sm dark:border-emerald-800/40 dark:bg-[#0d1a12]">
                <span data-testid="api-key-value" className="flex-1 break-all text-slate-800 dark:text-slate-200">
                  {newKey.api_key}
                </span>
                <CopyButton text={newKey.api_key} />
              </div>
              {newKey.description && (
                <p className="mt-2 text-xs text-emerald-700 dark:text-emerald-500">
                  Description: {newKey.description}
                </p>
              )}
            </div>
          )}
        </CardBody>
      </Card>
      )}

      {/* Active keys list */}
      <Card>
        <CardHeader>
          <div className="flex items-center gap-2">
            <Key size={15} />
            <span className="font-medium text-slate-800 dark:text-white">
              Active Keys{activeKeys.length > 0 && ` (${activeKeys.length})`}
            </span>
          </div>
        </CardHeader>
        <CardBody className="p-0">
          <ErrorMessage error={revokeError} />
          {activeKeys.length === 0 ? (
            <p className="px-5 py-6 text-sm text-slate-500 dark:text-[#8a8a93]">
              {isAdmin ? 'No active API keys. Create one above.' : 'No active API keys.'}
            </p>
          ) : (
            <table className="w-full text-sm">
              <thead className="bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:bg-[#0a0a0f] dark:text-[#6d768a]">
                <tr>
                  <th className="px-5 py-3 text-left">Key prefix</th>
                  <th className="px-5 py-3 text-left">Description</th>
                  <th className="px-5 py-3 text-left">Created</th>
                  <th className="px-5 py-3" />
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 dark:divide-[#1c1c24]">
                {activeKeys.map((key) => (
                  <tr key={key.key_id} className="dark:bg-[#0f0f16]">
                    <td className="px-5 py-3 font-mono text-slate-800 dark:text-slate-200">
                      {key.key_prefix}…
                    </td>
                    <td className="px-5 py-3 text-slate-600 dark:text-[#8a8a93]">
                      {key.description || <span className="italic text-slate-500">—</span>}
                    </td>
                    <td className="px-5 py-3 text-slate-500 dark:text-[#78849a]">
                      {new Date(key.created_at).toLocaleDateString()}
                    </td>
                    <td className="px-5 py-3 text-right">
                      {isAdmin && (
                      <Button
                        size="sm"
                        variant="danger"
                        disabled={revokingId === key.key_id}
                        onClick={() => setConfirmRevoke(key)}
                      >
                        <Trash2 size={12} />
                        {revokingId === key.key_id ? 'Revoking…' : 'Revoke'}
                      </Button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardBody>
      </Card>

      <ConfirmDialog
        open={confirmRevoke !== null}
        title="Revoke API key?"
        description={`${confirmRevoke?.key_prefix ?? ''}…${
          confirmRevoke?.description ? ` (${confirmRevoke.description})` : ''
        } stops working immediately for ${merchantName} and all of its sub-merchants. Requests using it will be rejected. This cannot be undone.`}
        confirmLabel="Revoke key"
        variant="danger"
        onCancel={() => setConfirmRevoke(null)}
        onConfirm={() => {
          if (confirmRevoke) handleRevoke(confirmRevoke.key_id)
          setConfirmRevoke(null)
        }}
      />
    </div>
  )
}
