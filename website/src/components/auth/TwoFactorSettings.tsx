import { useCallback, useEffect, useState } from 'react'
import { ShieldCheck } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../../lib/api'
import { useAuthStore } from '../../store/authStore'
import { Button } from '../ui/Button'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { ErrorMessage } from '../ui/ErrorMessage'
import { RecoveryCodes, TwoFactorChallenge } from './TwoFactorFlow'

interface TwoFactorStatus {
  mode: 'off' | 'optional' | 'required'
  enrolled: boolean
  recovery_codes_remaining: number
}

type PendingAction = 'regenerate' | 'disable'

/**
 * The account page's two-factor card. Hidden where the deployment has two-factor authentication
 * off, and for sessions that are not the user's own (handed over, or a super-admin view), which
 * the backend refuses to manage.
 */
export function TwoFactorSettings() {
  const user = useAuthStore((state) => state.user)
  const ownSession = Boolean(user && !user.isRedirectSession && !user.isSuperAdminView)

  const [status, setStatus] = useState<TwoFactorStatus | null>(null)
  const [action, setAction] = useState<PendingAction | null>(null)
  const [code, setCode] = useState('')
  const [newCodes, setNewCodes] = useState<string[] | null>(null)
  const [enrollNotice, setEnrollNotice] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(() => {
    apiFetch<TwoFactorStatus>('/auth/2fa/status')
      .then(setStatus)
      .catch(() => setStatus(null))
  }, [])

  useEffect(() => {
    if (ownSession) load()
  }, [load, ownSession])

  if (!ownSession || !status || status.mode === 'off') return null

  async function enroll() {
    setError(null)
    setLoading(true)
    try {
      const challenge = await apiFetch<TwoFactorChallenge>('/auth/2fa/enroll', { method: 'POST' })
      setEnrollNotice(
        `We sent a link to ${challenge.email}. Open it to continue setting up your authenticator app — it expires in 15 minutes.`,
      )
    } catch (err) {
      setError(apiErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  async function submitCode(e: React.FormEvent) {
    e.preventDefault()
    if (!action) return
    setError(null)
    setLoading(true)
    try {
      if (action === 'regenerate') {
        const res = await apiFetch<{ recovery_codes: string[] }>('/auth/2fa/recovery-codes', {
          method: 'POST',
          body: JSON.stringify({ totp: code }),
        })
        setNewCodes(res.recovery_codes)
      } else {
        await apiFetch('/auth/2fa/disable', { method: 'POST', body: JSON.stringify({ totp: code }) })
        load()
      }
      setAction(null)
      setCode('')
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
              <ShieldCheck
                size={20}
                className={status.enrolled ? 'mt-0.5 text-emerald-600' : 'mt-0.5 text-slate-400'}
              />
              <div className="text-sm text-slate-600 dark:text-[#a1a1aa]">
                {status.enrolled ? (
                  <>
                    <p className="font-medium text-slate-900 dark:text-white">On</p>
                    <p>
                      You are asked for a code from your authenticator app when you sign in.{' '}
                      {status.recovery_codes_remaining} of 8 recovery codes left.
                    </p>
                  </>
                ) : (
                  <>
                    <p className="font-medium text-slate-900 dark:text-white">Off</p>
                    <p>
                      Protect your account with a code from an authenticator app at sign-in. Setup
                      starts with a link sent to your email.
                    </p>
                  </>
                )}
              </div>
            </div>

            {enrollNotice ? (
              <div className="rounded-lg border border-sky-500/30 bg-sky-50 px-4 py-3 text-sm text-sky-700 dark:border-sky-500/20 dark:bg-sky-500/8 dark:text-sky-300">
                {enrollNotice}
              </div>
            ) : null}

            {action ? (
              <form onSubmit={submitCode} className="space-y-3">
                <label className="block">
                  <span className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
                    Code from your authenticator app
                  </span>
                  <input
                    autoFocus
                    required
                    value={code}
                    onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                    inputMode="numeric"
                    autoComplete="one-time-code"
                    placeholder="123456"
                    className="h-12 w-full rounded-xl border border-slate-200 bg-white px-4 font-mono text-sm tracking-[0.3em] text-slate-950 outline-none transition focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white"
                  />
                </label>
                <ErrorMessage error={error} />
                <div className="flex gap-2">
                  <Button
                    type="submit"
                    variant={action === 'disable' ? 'danger' : 'primary'}
                    disabled={loading}
                  >
                    {action === 'disable' ? 'Turn off' : 'Generate new codes'}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    onClick={() => {
                      setAction(null)
                      setCode('')
                      setError(null)
                    }}
                  >
                    Cancel
                  </Button>
                </div>
              </form>
            ) : (
              <>
                <ErrorMessage error={error} />
                <div className="flex flex-wrap gap-2">
                  {status.enrolled ? (
                    <>
                      <Button variant="secondary" onClick={() => setAction('regenerate')}>
                        Generate new recovery codes
                      </Button>
                      {status.mode === 'optional' ? (
                        <Button variant="danger" onClick={() => setAction('disable')}>
                          Turn off
                        </Button>
                      ) : null}
                    </>
                  ) : (
                    <Button onClick={enroll} disabled={loading || Boolean(enrollNotice)}>
                      Set up two-factor authentication
                    </Button>
                  )}
                </div>
              </>
            )}
          </div>
        )}
      </CardBody>
    </Card>
  )
}
