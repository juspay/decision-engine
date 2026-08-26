import { useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { ArrowLeft, ArrowRight, CheckCircle, Loader2, Mail, Moon, Sun } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../lib/api'
import { getResolvedThemePreference, persistThemePreference } from '../lib/theme'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface ForgotPasswordResponse {
  message: string
}

export function ForgotPasswordPage() {
  const assetBaseUrl = import.meta.env.BASE_URL
  const location = useLocation()
  const prefillEmail = (location.state as { email?: string } | null)?.email ?? ''
  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')
  const [email, setEmail] = useState(prefillEmail)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [sentMessage, setSentMessage] = useState<string | null>(null)

  function handleThemeToggle() {
    const next = isDark ? 'light' : 'dark'
    setIsDark(next === 'dark')
    persistThemePreference(next)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const res = await apiFetch<ForgotPasswordResponse>('/auth/forgot-password', {
        method: 'POST',
        body: JSON.stringify({ email }),
      })
      setSentMessage(
        res.message ??
          'If an account exists for that email, a password reset link has been sent.',
      )
    } catch (err) {
      setError(apiErrorMessage(err))
    } finally {
      setLoading(false)
    }
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
          <div className="w-full max-w-[440px] rounded-3xl border border-slate-200 bg-white px-8 py-10 shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none sm:px-10">
            {sentMessage ? (
              <div className="space-y-6 text-center">
                <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-emerald-500/10">
                  <CheckCircle size={28} className="text-emerald-700" />
                </div>
                <div className="space-y-2">
                  <h1 className="text-2xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Check your inbox
                  </h1>
                  <p className="text-sm leading-6 text-slate-500 dark:text-[#8a94a7]">{sentMessage}</p>
                </div>
                <Link
                  to="/login"
                  className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110"
                >
                  Back to sign in
                </Link>
              </div>
            ) : (
              <>
                <div className="text-center">
                  <h1 className="text-2xl font-semibold tracking-tight text-slate-950 dark:text-white sm:text-[1.75rem]">
                    Forgot your password?
                  </h1>
                  <p className="mt-3 text-sm leading-6 text-slate-500 dark:text-[#8a94a7] mx-auto max-w-[57ch]">
                    Enter the email tied to your account and we&rsquo;ll send you a link to reset your
                    password.
                  </p>
                </div>

                <form onSubmit={handleSubmit} className="mt-8 space-y-5">
                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
                      Email
                    </span>
                    <div className="relative">
                      <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-500 dark:text-[#78849a]">
                        <Mail size={16} />
                      </span>
                      <input
                        type="email"
                        value={email}
                        onChange={(e) => setEmail(e.target.value)}
                        placeholder="name@company.com"
                        required
                        autoFocus
                        className="h-14 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-4 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-500 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none"
                      />
                    </div>
                  </label>

                  <ErrorMessage error={error} />

                  <button
                    type="submit"
                    disabled={loading}
                    className="group inline-flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-5 text-sm font-semibold text-white transition-all duration-200 hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {loading ? (
                      <>
                        <Loader2 size={16} className="animate-spin" />
                        Sending link
                      </>
                    ) : (
                      <>
                        Send reset link
                        <ArrowRight
                          size={16}
                          className="transition-transform duration-200 group-hover:translate-x-0.5"
                        />
                      </>
                    )}
                  </button>
                </form>

                <div className="mt-8 border-t border-slate-200 pt-6 text-center dark:border-[#1d1d23]">
                  <Link
                    to="/login"
                    className="inline-flex items-center gap-1.5 text-sm font-medium text-slate-500 transition-colors hover:text-slate-950 dark:text-[#8a94a7] dark:hover:text-white"
                  >
                    <ArrowLeft size={15} />
                    Back to sign in
                  </Link>
                </div>
              </>
            )}
          </div>
        </div>

        <footer className="px-6 py-5 text-center text-xs text-slate-500 dark:text-[#78849a]">
          Juspay Decision Engine
        </footer>
      </div>
    </div>
  )
}
