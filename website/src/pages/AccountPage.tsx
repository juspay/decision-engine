import { useState } from 'react'
import { Eye, EyeOff, KeyRound } from 'lucide-react'
import { apiFetch } from '../lib/api'
import { getApiErrorMessage } from '../lib/apiError'
import { getPasswordPolicyError } from '../lib/passwordPolicy'
import { useAuthStore } from '../store/authStore'
import { Card, CardBody, CardHeader } from '../components/ui/Card'
import { Button } from '../components/ui/Button'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface ChangePasswordResponse {
  message: string
  token: string
}

export function AccountPage() {
  const setToken = useAuthStore((s) => s.setToken)
  const [currentPassword, setCurrentPassword] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showCurrent, setShowCurrent] = useState(false)
  const [showNew, setShowNew] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [success, setSuccess] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setSuccess(false)

    const policyError = getPasswordPolicyError(newPassword)
    if (policyError) { setError(policyError); return }
    if (newPassword !== confirmPassword) { setError('New passwords do not match.'); return }
    if (newPassword === currentPassword) { setError('New password must differ from the current one.'); return }

    setLoading(true)
    try {
      const res = await apiFetch<ChangePasswordResponse>('/auth/change-password', {
        method: 'POST',
        body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
      })
      // Changing the password revokes every previously issued session token, including
      // the one this request was made with — swap in the fresh one from the response.
      if (res.token) setToken(res.token)
      setSuccess(true)
      setCurrentPassword('')
      setNewPassword('')
      setConfirmPassword('')
    } catch (err) {
      setError(getApiErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="mx-auto max-w-lg px-4 py-10">
      <div className="mb-8 flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-brand-500/10">
          <KeyRound size={20} className="text-brand-500" />
        </div>
        <div>
          <h1 className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
            Account
          </h1>
          <p className="text-sm text-slate-500 dark:text-[#8a94a7]">Manage your account security</p>
        </div>
      </div>

      <Card>
        <CardHeader>Change password</CardHeader>
        <CardBody>
          <form onSubmit={handleSubmit} className="space-y-5">
            <PasswordField
              label="Current password"
              value={currentPassword}
              onChange={setCurrentPassword}
              show={showCurrent}
              onToggleShow={() => setShowCurrent(v => !v)}
            />
            <PasswordField
              label="New password"
              value={newPassword}
              onChange={setNewPassword}
              show={showNew}
              onToggleShow={() => setShowNew(v => !v)}
              footer="Minimum 10 characters with uppercase, lowercase, number, and special character."
            />
            <PasswordField
              label="Confirm new password"
              value={confirmPassword}
              onChange={setConfirmPassword}
              show={showNew}
              onToggleShow={() => setShowNew(v => !v)}
            />

            <ErrorMessage error={error} />

            {success && (
              <div className="rounded-lg border border-emerald-500/30 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 dark:border-emerald-500/20 dark:bg-emerald-500/8 dark:text-emerald-300">
                Password updated successfully.
              </div>
            )}

            <Button type="submit" disabled={loading} className="w-full">
              {loading ? 'Updating…' : 'Update password'}
            </Button>
          </form>
        </CardBody>
      </Card>
    </div>
  )
}

function PasswordField({
  label,
  value,
  onChange,
  show,
  onToggleShow,
  footer,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  show: boolean
  onToggleShow: () => void
  footer?: string
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
        {label}
      </span>
      <div className="relative">
        <input
          type={show ? 'text' : 'password'}
          value={value}
          onChange={e => onChange(e.target.value)}
          required
          className="h-12 w-full rounded-xl border border-slate-200 bg-white px-4 pr-11 text-sm text-slate-950 outline-none transition placeholder:text-slate-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white"
        />
        <button
          type="button"
          onClick={onToggleShow}
          className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 transition-colors hover:text-slate-700 dark:hover:text-slate-200"
          aria-label={show ? 'Hide password' : 'Show password'}
        >
          {show ? <Eye size={17} /> : <EyeOff size={17} />}
        </button>
      </div>
      {footer && <p className="mt-1.5 text-xs text-slate-500 dark:text-[#7b8496]">{footer}</p>}
    </label>
  )
}
