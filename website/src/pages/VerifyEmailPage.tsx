import { useEffect, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { CheckCircle, Loader2, Moon, Sun, XCircle } from 'lucide-react'
import { apiFetch } from '../lib/api'
import { getResolvedThemePreference, persistThemePreference } from '../lib/theme'

type Status = 'verifying' | 'success' | 'error'

export function VerifyEmailPage() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const [status, setStatus] = useState<Status>('verifying')
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')
  const didRun = useRef(false)
  const assetBaseUrl = import.meta.env.BASE_URL

  useEffect(() => {
    if (didRun.current) return
    didRun.current = true

    const token = searchParams.get('token')

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
        const msg = err instanceof Error ? err.message : 'Verification failed'
        const match = msg.match(/API error \d+: (.+)/)
        if (match) {
          try {
            const parsed = JSON.parse(match[1])
            setErrorMessage(parsed.message ?? match[1])
          } catch {
            setErrorMessage(match[1])
          }
        } else {
          setErrorMessage(msg)
        }
        setStatus('error')
      })
  }, [navigate, searchParams])

  function handleThemeToggle() {
    const next = isDark ? 'light' : 'dark'
    setIsDark(next === 'dark')
    persistThemePreference(next)
  }

  return (
    <div className="relative min-h-screen overflow-hidden bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_18%_18%,_rgba(59,130,246,0.06),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(14,165,233,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.03),_transparent_24%)] dark:bg-[radial-gradient(circle_at_18%_18%,_rgba(56,189,248,0.05),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(59,130,246,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.035),_transparent_24%)]" />
      <div className="pointer-events-none absolute inset-0 opacity-[0.05] dark:opacity-[0.08] [background-image:linear-gradient(rgba(148,163,184,0.08)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.08)_1px,transparent_1px)] [background-size:56px_56px]" />

      <div className="relative z-10 flex min-h-screen flex-col">
        <header className="flex items-center justify-between px-6 py-5 sm:px-10">
          <div>
            <img
              src={`${assetBaseUrl}logo/decision-engine-light.svg`}
              alt="Juspay Decision Engine"
              className="h-10 w-auto dark:hidden sm:h-11"
            />
            <img
              src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
              alt="Juspay Decision Engine"
              className="hidden h-10 w-auto dark:block sm:h-11"
            />
          </div>
          <button
            type="button"
            onClick={handleThemeToggle}
            className="flex h-9 w-9 items-center justify-center rounded-xl text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-white/8 dark:hover:text-white"
            aria-label="Toggle theme"
            title="Toggle theme"
          >
            {isDark ? <Sun size={18} /> : <Moon size={18} />}
          </button>
        </header>

        <div className="flex flex-1 items-center justify-center px-6 py-12">
          <div className="w-full max-w-[420px] rounded-3xl border border-slate-200 bg-white px-10 py-12 text-center shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none">

            {status === 'verifying' && (
              <div className="space-y-5">
                <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-brand-500/10">
                  <Loader2 size={28} className="animate-spin text-brand-500" />
                </div>
                <div className="space-y-2">
                  <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Verifying your email
                  </p>
                  <p className="text-sm text-slate-500 dark:text-[#8a94a7]">
                    Hang on while we confirm your address…
                  </p>
                </div>
              </div>
            )}

            {status === 'success' && (
              <div className="space-y-5">
                <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-emerald-500/10">
                  <CheckCircle size={28} className="text-emerald-500" />
                </div>
                <div className="space-y-2">
                  <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Email verified
                  </p>
                  <p className="text-sm text-slate-500 dark:text-[#8a94a7]">
                    Your account is confirmed. Redirecting you to sign in…
                  </p>
                </div>
                <div className="pt-1">
                  <div className="mx-auto h-1 w-24 overflow-hidden rounded-full bg-slate-100 dark:bg-white/8">
                    <div className="h-full animate-[progress_2.5s_linear_forwards] rounded-full bg-emerald-500" />
                  </div>
                </div>
              </div>
            )}

            {status === 'error' && (
              <div className="space-y-5">
                <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-red-500/10">
                  <XCircle size={28} className="text-red-500" />
                </div>
                <div className="space-y-2">
                  <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Verification failed
                  </p>
                  <p className="text-sm text-slate-500 dark:text-[#8a94a7]">
                    {errorMessage ?? 'The link may have expired or already been used.'}
                  </p>
                </div>
                <div className="pt-2">
                  <button
                    onClick={() => navigate('/login', { replace: true })}
                    className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110"
                  >
                    Back to sign in
                  </button>
                </div>
              </div>
            )}

          </div>
        </div>

        <footer className="px-6 py-5 text-center text-xs text-slate-400 dark:text-[#525866]">
          Juspay Decision Engine
        </footer>
      </div>
    </div>
  )
}
