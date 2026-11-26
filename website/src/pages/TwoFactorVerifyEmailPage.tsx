import { useEffect, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { Loader2, XCircle } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../lib/api'
import { useCompleteSignIn } from '../hooks/useCompleteSignIn'
import { AUTH_PRIMARY_BUTTON, AuthCardLayout, AuthStatus } from '../components/auth/AuthCardLayout'
import { TwoFactorChallenge, TwoFactorFlow } from '../components/auth/TwoFactorFlow'

/**
 * Where the two-factor verification email lands. Redeeming the link confirms the mailbox and opens
 * authenticator setup here, in the browser that opened it; finishing setup signs the user in.
 */
export function TwoFactorVerifyEmailPage() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const completeSignIn = useCompleteSignIn()
  const [challenge, setChallenge] = useState<TwoFactorChallenge | null>(null)
  const [error, setError] = useState<string | null>(null)
  const redeemedToken = useRef<string | null>(null)

  useEffect(() => {
    const token = searchParams.get('token')
    // The link is single-use, so a second run (StrictMode, a re-render) must not spend it again.
    if (token === redeemedToken.current) return
    redeemedToken.current = token

    if (!token) {
      setError('This link is missing its token. Use the link from your email.')
      return
    }

    apiFetch<TwoFactorChallenge>('/auth/2fa/email/confirm', {
      method: 'POST',
      body: JSON.stringify({ token }),
    })
      .then(setChallenge)
      .catch((err: unknown) => setError(apiErrorMessage(err, 'Verification failed')))
  }, [searchParams])

  const backToSignIn = () => navigate('/login', { replace: true })

  if (challenge) {
    return (
      <AuthCardLayout>
        <TwoFactorFlow challenge={challenge} onAuthenticated={(session) => completeSignIn(session)} onCancel={backToSignIn} />
      </AuthCardLayout>
    )
  }

  return (
    <AuthCardLayout size="status">
      {error ? (
        <AuthStatus
          icon={XCircle}
          tone="error"
          title="Verification failed"
          action={
            <div className="pt-2">
              <button onClick={backToSignIn} className={AUTH_PRIMARY_BUTTON}>
                Back to sign in
              </button>
            </div>
          }
        >
          {error} Sign in again to get a new link.
        </AuthStatus>
      ) : (
        <AuthStatus icon={Loader2} tone="progress" spin title="Verifying your email">
          Hang on while we confirm your address…
        </AuthStatus>
      )}
    </AuthCardLayout>
  )
}
