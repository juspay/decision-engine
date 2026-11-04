import { useId, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import { useAuthStore, type MerchantInfo } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiErrorMessage, apiFetch } from '../../lib/api'
import { merchantLabel } from '../../lib/merchantScope'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'
import { Modal } from '../ui/Modal'

interface CreateMerchantResponse {
  token: string
  merchant_id: string
  merchants: MerchantInfo[]
}

/**
 * Names and creates a sub-merchant of `parent`, then moves the session onto it and opens its
 * overview — its routing is what the user will configure next. A dialog over the current page
 * rather than a page of its own: it is one field, asked in the middle of other work.
 */
export function AddSubMerchantDialog({
  parent,
  onClose,
}: {
  parent: MerchantInfo
  onClose: () => void
}) {
  const navigate = useNavigate()
  const updateMerchant = useAuthStore((s) => s.updateMerchant)
  const setMerchantId = useMerchantStore((s) => s.setMerchantId)
  const [name, setName] = useState('')
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const titleId = useId()
  const parentName = merchantLabel(parent)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!name.trim() || creating) return
    setCreating(true)
    setError(null)
    try {
      const res = await apiFetch<CreateMerchantResponse>('/onboarding/merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_name: name.trim(), parent_merchant_id: parent.merchant_id }),
      })
      updateMerchant(res.token, res.merchant_id, res.merchants)
      setMerchantId(res.merchant_id)
      onClose()
      navigate('/')
    } catch (err) {
      setError(apiErrorMessage(err))
      setCreating(false)
    }
  }

  // Held open while the merchant is being created: the session moves onto it when that finishes,
  // which must not happen behind a dialog the user already dismissed.
  return (
    <Modal labelledBy={titleId} onClose={onClose} dismissible={!creating} initialFocus={inputRef}>
      <form onSubmit={handleSubmit}>
        <h3 id={titleId} className="text-sm font-semibold text-slate-900 dark:text-white">
          Add a sub-merchant to {parentName}
        </h3>
        <p className="mt-2 text-sm leading-relaxed text-slate-500 dark:text-[#8a93a6]">
          It gets its own routing rules. {parentName}'s members and API keys work for it as soon as
          it is created.
        </p>
        <input
          ref={inputRef}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={`e.g. ${parentName} US`}
          aria-label="Sub-merchant name"
          className="mt-4 h-10 w-full rounded-lg border border-slate-200 bg-white px-3 text-sm text-slate-900 outline-none transition placeholder:text-slate-500 focus:border-brand-500 dark:border-[#2a2d35] dark:bg-[#1a1d25] dark:text-white"
        />
        <div className="mt-3">
          <ErrorMessage error={error} />
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <Button type="button" variant="secondary" size="sm" onClick={onClose} disabled={creating}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={!name.trim() || creating}>
            {creating && <Loader2 size={13} className="animate-spin" />}
            {creating ? 'Creating…' : 'Create sub-merchant'}
          </Button>
        </div>
      </form>
    </Modal>
  )
}
