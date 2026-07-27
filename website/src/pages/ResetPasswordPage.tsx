import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Eye, EyeOff, Loader2, LockKeyhole, Moon, Sun, XCircle } from 'lucide-react'
import { apiPost } from '../lib/api'
import { getApiErrorMessage } from '../lib/apiError'
import { getPasswordPolicyError } from '../lib/passwordPolicy'
import { getResolvedThemePreference, persistThemePreference } from '../lib/theme'
import { useAuthStore } from '../store/authStore'
import { ErrorMessage } from '../components/ui/ErrorMessage'

export function ResetPasswordPage() {
  const navigate = useNavigate()
  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')
  // Capture the token before it is stripped from the URL below.
  const [token] = useState(() => new URLSearchParams(window.location.search).get('token'))
  const [newPassword, setNewPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const assetBaseUrl = import.meta.env.BASE_URL

  useEffect(() => {
    // The one-time code shouldn't linger in the address bar, browser history, or leak
    // via the Referer header — strip it immediately and suppress referrers on this page.
    const params = new URLSearchParams(window.location.search)
    if (params.has('token')) {
      params.delete('token')
      const newSearch = params.toString()
      window.history.replaceState(
        null,
        '',
        window.location.pathname + (newSearch ? `?${newSearch}` : ''),
      )
    }

    const meta = document.createElement('meta')
    meta.name = 'referrer'
    meta.content = 'no-referrer'
    document.head.appendChild(meta)
    return () => {
      document.head.removeChild(meta)
    }
  }, [])

  function handleThemeToggle() {
    const next = isDark ? 'light' : 'dark'
    setIsDark(next === 'dark')
    persistThemePreference(next)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)

    const policyError = getPasswordPolicyError(newPassword)
    if (policyError) {
      setError(policyError)
      return
    }
    if (newPassword !== confirmPassword) {
      setError('Passwords do not match.')
      return
    }

    setLoading(true)
    try {
      await apiPost('/auth/reset-password', { token, new_password: newPassword })
      // Any session persisted in this browser now holds a revoked JWT; clear it so the
      // login page shows the success notice instead of bouncing through the dashboard.
      useAuthStore.getState().clearAuth()
      navigate('/login', {
        replace: true,
        state: { notice: 'Password reset successfully. Sign in with your new password.' },
      })
    } catch (err) {
      setError(getApiErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  const isExpiredLinkError = error != null && /invalid or expired/i.test(error)

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
          <div className="w-full max-w-[420px] rounded-3xl border border-slate-200 bg-white px-10 py-12 shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none">
            {!token ? (
              <div className="space-y-5 text-center">
                <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-red-500/10">
                  <XCircle size={28} className="text-red-500" />
                </div>
                <div className="space-y-2">
                  <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Reset link is incomplete
                  </p>
                  <p className="text-sm leading-6 text-slate-500 dark:text-[#8a94a7]">
                    This page needs the link from your password reset email. Open the link
                    directly, or request a new one.
                  </p>
                </div>
                <div className="pt-2">
                  <button
                    onClick={() => navigate('/forgot-password', { replace: true })}
                    className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110"
                  >
                    Request a new link
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-6">
                <div className="space-y-3 text-center">
                  <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-brand-500/10">
                    <LockKeyhole size={28} className="text-brand-500" />
                  </div>
                  <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
                    Choose a new password
                  </p>
                  <p className="text-sm leading-6 text-slate-500 dark:text-[#8a94a7]">
                    Minimum 10 characters with uppercase, lowercase, number, and special
                    character.
                  </p>
                </div>

                <form onSubmit={handleSubmit} className="space-y-4">
                  <PasswordField
                    label="New password"
                    value={newPassword}
                    onChange={setNewPassword}
                    show={showPassword}
                    onToggleShow={() => setShowPassword((v) => !v)}
                    autoFocus
                  />
                  <PasswordField
                    label="Confirm new password"
                    value={confirmPassword}
                    onChange={setConfirmPassword}
                    show={showPassword}
                    onToggleShow={() => setShowPassword((v) => !v)}
                  />

                  <ErrorMessage error={error} />

                  {isExpiredLinkError ? (
                    <button
                      type="button"
                      onClick={() => navigate('/forgot-password')}
                      className="inline-flex h-12 w-full items-center justify-center rounded-2xl border border-slate-200 px-6 text-sm font-semibold text-slate-700 transition-colors hover:bg-slate-50 dark:border-[#2a303a] dark:text-slate-200 dark:hover:bg-white/5"
                    >
                      Request a new link
                    </button>
                  ) : null}

                  <button
                    type="submit"
                    disabled={loading}
                    className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {loading ? (
                      <>
                        <Loader2 size={16} className="animate-spin" />
                        Resetting password
                      </>
                    ) : (
                      'Reset password'
                    )}
                  </button>
                </form>
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

function PasswordField({
  label,
  value,
  onChange,
  show,
  onToggleShow,
  autoFocus,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  show: boolean
  onToggleShow: () => void
  autoFocus?: boolean
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
          onChange={(e) => onChange(e.target.value)}
          required
          autoFocus={autoFocus}
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
    </label>
  )
}
