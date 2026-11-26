import { useCallback, useEffect, useState } from 'react'
import { ShieldCheck } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../../lib/api'
import { useAuthStore } from '../../store/authStore'
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { RecoveryCodes } from './TwoFactorFlow'

/**
 * The account page's two-factor card: how many recovery codes are left, and a way to replace
 * them. Shown only to users who have set up two-factor authentication, on their own session
 * (not a handed-over one or a super-admin view, which the backend refuses).
 */
export function TwoFactorSettings() {
  const user = useAuthStore((state) => state.user)
  const ownSession = Boolean(user && !user.isRedirectSession && !user.isSuperAdminView)

  const [remaining, setRemaining] = useState<number | null>(null)
  const [regenerating, setRegenerating] = useState(false)
  const [useRecovery, setUseRecovery] = useState(false)
  const [code, setCode] = useState('')
  const [newCodes, setNewCodes] = useState<string[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(() => {
    apiFetch<{ recovery_codes_remaining?: number }>('/auth/me')
      .then((me) => setRemaining(me.recovery_codes_remaining ?? null))
      .catch(() => setRemaining(null))
  }, [])

  useEffect(() => {
    if (ownSession) load()
  }, [load, ownSession])

  if (!ownSession || remaining === null) return null

  function closeForm() {
    setRegenerating(false)
    setUseRecovery(false)
    setCode('')
    setError(null)
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const res = await apiFetch<{ recovery_codes: string[] }>('/auth/2fa/recovery-codes', {
        method: 'POST',
        body: JSON.stringify(useRecovery ? { recovery_code: code } : { totp: code }),
      })
      setNewCodes(res.recovery_codes)
      closeForm()
    } catch (err) {
      setError(apiErrorMessage(err))
      setCode('')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Card className="mt-6">
      <CardHeader>Two-factor authentication</CardHeader>
      <CardBody>
        {newCodes ? (
          <RecoveryCodes
            codes={newCodes}
            email={user?.email ?? ''}
            doneLabel="Done"
            onDone={() => {
              setNewCodes(null)
              load()
            }}
          />
        ) : (
          <div className="space-y-5">
            <div className="flex items-start gap-3">
              <ShieldCheck size={20} className="mt-0.5 text-emerald-600" />
              <div className="text-sm text-slate-600 dark:text-[#a1a1aa]">
                <p className="font-medium text-slate-900 dark:text-white">On</p>
                <p>
                  You are asked for a code from your authenticator app when you sign in.{' '}
                  {remaining} of 8 recovery codes left.
                </p>
              </div>
            </div>

            {regenerating ? (
              <form onSubmit={submit} className="space-y-3">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
                    {useRecovery ? 'One of your recovery codes' : 'Code from your authenticator app'}
                  </span>
                  <input
                    key={String(useRecovery)}
                    autoFocus
                    required
                    value={code}
                    onChange={(e) =>
                      setCode(
                        useRecovery ? e.target.value : e.target.value.replace(/\D/g, '').slice(0, 6),
                      )
                    }
                    inputMode={useRecovery ? 'text' : 'numeric'}
                    autoComplete="one-time-code"
                    placeholder={useRecovery ? 'xxxx-xxxx' : '123456'}
                    className="h-12 w-full rounded-xl border border-slate-200 bg-white px-4 font-mono text-sm tracking-[0.3em] text-slate-950 outline-none transition focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white"
                  />
                </label>
                <ErrorMessage error={error} />
                <div className="flex flex-wrap items-center gap-2">
                  <Button type="submit" disabled={loading}>
                    Generate new codes
                  </Button>
                  <Button type="button" variant="ghost" onClick={closeForm}>
                    Cancel
                  </Button>
                  <button
                    type="button"
                    onClick={() => {
                      setUseRecovery((value) => !value)
                      setCode('')
                      setError(null)
                    }}
                    className="ml-auto text-sm font-medium text-brand-600 hover:text-brand-700 dark:text-brand-500"
                  >
                    {useRecovery ? 'Use authenticator app' : 'Use a recovery code'}
                  </button>
                </div>
              </form>
            ) : (
              <Button variant="secondary" onClick={() => setRegenerating(true)}>
                Generate new recovery codes
              </Button>
            )}
          </div>
        )}
      </CardBody>
    </Card>
  )
}
