import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useNavigate } from 'react-router-dom'
import { Loader2 } from 'lucide-react'
import { useAuthStore, type MerchantInfo } from '../../store/authStore'
import { useMerchantStore } from '../../store/merchantStore'
import { apiErrorMessage, apiFetch } from '../../lib/api'
import { merchantLabel } from '../../lib/merchantScope'
import { Button } from '../ui/Button'
import { ErrorMessage } from '../ui/ErrorMessage'

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
  const parentName = merchantLabel(parent)

  useEffect(() => {
    inputRef.current?.focus()
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [onClose])

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

  // Rendered at the document root, so the backdrop also covers the sidebar.
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/40 backdrop-blur-[2px]" onClick={onClose} />
      <form
        onSubmit={handleSubmit}
        className="relative w-full max-w-sm rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-[#2a303a] dark:bg-[#0d1118]"
      >
        <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
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
          <Button type="button" variant="secondary" size="sm" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" size="sm" disabled={!name.trim() || creating}>
            {creating && <Loader2 size={13} className="animate-spin" />}
            {creating ? 'Creating…' : 'Create sub-merchant'}
          </Button>
        </div>
      </form>
    </div>,
    document.body,
  )
}
