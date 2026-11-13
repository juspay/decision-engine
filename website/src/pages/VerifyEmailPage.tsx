import { useEffect, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { CheckCircle, Loader2, XCircle } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../lib/api'
import { AUTH_PRIMARY_BUTTON, AuthCardLayout, AuthStatus } from '../components/auth/AuthCardLayout'

type Status = 'verifying' | 'success' | 'error'

export function VerifyEmailPage() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const [status, setStatus] = useState<Status>('verifying')
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const lastVerifiedToken = useRef<string | null>(null)

  useEffect(() => {
    const token = searchParams.get('token')
    if (token === lastVerifiedToken.current) return
    lastVerifiedToken.current = token

    if (!token) {
      setStatus('error')
      setErrorMessage('Verification link is missing a token. Please use the link from your email.')
      return
    }

    apiFetch(`/auth/verify-email?token=${encodeURIComponent(token)}`, { method: 'GET' })
      .then(() => {
        setStatus('success')
        setTimeout(() => {
          navigate('/login', {
            replace: true,
            state: { notice: 'Email verified! You can now sign in.' },
          })
        }, 2500)
      })
      .catch((err: unknown) => {
        setErrorMessage(apiErrorMessage(err, 'Verification failed'))
        setStatus('error')
      })
  }, [navigate, searchParams])

  return (
    <AuthCardLayout size="status">
      {status === 'verifying' && (
        <AuthStatus icon={Loader2} tone="progress" spin title="Verifying your email">
          Hang on while we confirm your address…
        </AuthStatus>
      )}

      {status === 'success' && (
        <AuthStatus
          icon={CheckCircle}
          tone="success"
          title="Email verified"
          action={
            <div className="pt-1">
              <div className="mx-auto h-1 w-24 overflow-hidden rounded-full bg-slate-100 dark:bg-white/8">
                <div className="h-full animate-[progress_2.5s_linear_forwards] rounded-full bg-emerald-500" />
              </div>
            </div>
          }
        >
          Your account is confirmed. Redirecting you to sign in…
        </AuthStatus>
      )}

      {status === 'error' && (
        <AuthStatus
          icon={XCircle}
          tone="error"
          title="Verification failed"
          action={
            <div className="pt-2">
              <button onClick={() => navigate('/login', { replace: true })} className={AUTH_PRIMARY_BUTTON}>
                Back to sign in
              </button>
            </div>
          }
        >
          {errorMessage ?? 'The link may have expired or already been used.'}
        </AuthStatus>
      )}
    </AuthCardLayout>
  )
}
