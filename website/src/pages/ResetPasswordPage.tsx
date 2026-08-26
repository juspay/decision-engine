import { useState } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router-dom'
import {
  ArrowRight,
  Eye,
  EyeOff,
  Loader2,
  LockKeyhole,
  Moon,
  Sun,
  XCircle,
} from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../lib/api'
import { getResolvedThemePreference, persistThemePreference } from '../lib/theme'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface ResetPasswordResponse {
  message: string
}

function getPasswordPolicyError(password: string): string | null {
  if (password.length < 10) return 'Use at least 10 characters.'
  if (!/[A-Z]/.test(password)) return 'Add at least one uppercase letter.'
  if (!/[a-z]/.test(password)) return 'Add at least one lowercase letter.'
  if (!/[0-9]/.test(password)) return 'Add at least one number.'
  if (!/[^A-Za-z0-9]/.test(password)) return 'Add at least one special character.'
  return null
}

export function ResetPasswordPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const token = searchParams.get('token')
  const assetBaseUrl = import.meta.env.BASE_URL

  const [isDark, setIsDark] = useState(() => getResolvedThemePreference() === 'dark')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function handleThemeToggle() {
    const next = isDark ? 'light' : 'dark'
    setIsDark(next === 'dark')
    persistThemePreference(next)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)

    const policyError = getPasswordPolicyError(password)
    if (policyError) {
      setError(policyError)
      return
    }
    if (password !== confirmPassword) {
      setError('Passwords do not match.')
      return
    }

    setLoading(true)
    try {
      await apiFetch<ResetPasswordResponse>('/auth/reset-password', {
        method: 'POST',
        body: JSON.stringify({ token, new_password: password }),
      })
      navigate('/login', {
        replace: true,
        state: { notice: 'Password reset! Sign in with your new password.' },
      })
    } catch (err) {
      setError(apiErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  const themeToggle = (
    <button
      type="button"
      onClick={handleThemeToggle}
      className="flex h-9 w-9 items-center justify-center rounded-xl text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-white/8 dark:hover:text-white"
      aria-label="Toggle theme"
      title="Toggle theme"
    >
      {isDark ? <Sun size={18} /> : <Moon size={18} />}
    </button>
  )

  const shell = (children: React.ReactNode) => (
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
          {themeToggle}
        </header>
        <div className="flex flex-1 items-center justify-center px-6 py-12">{children}</div>
        <footer className="px-6 py-5 text-center text-xs text-slate-500 dark:text-[#78849a]">
          Juspay Decision Engine
        </footer>
      </div>
    </div>
  )

  if (!token) {
    return shell(
      <div className="w-full max-w-[420px] rounded-3xl border border-slate-200 bg-white px-10 py-12 text-center shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none">
        <div className="space-y-5">
          <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-full bg-red-500/10">
            <XCircle size={28} className="text-red-600" />
          </div>
          <div className="space-y-2">
            <p className="text-xl font-semibold tracking-tight text-slate-950 dark:text-white">
              Invalid reset link
            </p>
            <p className="text-sm text-slate-500 dark:text-[#8a94a7]">
              This link is missing a token. Request a new password reset link to continue.
            </p>
          </div>
          <Link
            to="/forgot-password"
            className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-6 text-sm font-semibold text-white transition-all hover:brightness-110"
          >
            Request a new link
          </Link>
        </div>
      </div>,
    )
  }

  return shell(
    <div className="w-full max-w-[440px] rounded-3xl border border-slate-200 bg-white px-8 py-10 shadow-[0_20px_60px_-20px_rgba(15,23,42,0.08)] dark:border-[#1d1d23] dark:bg-[#0b0e14] dark:shadow-none sm:px-10">
      <div className="text-center">
        <h1 className="text-2xl font-semibold tracking-tight text-slate-950 dark:text-white sm:text-[1.75rem]">
          Set a new password
        </h1>
        <p className="mt-3 text-sm leading-6 text-slate-500 dark:text-[#8a94a7]">
          Choose a strong password you don&rsquo;t use anywhere else.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="mt-8 space-y-5">
        <label className="block">
          <span className="mb-2 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
            New password
          </span>
          <div className="relative">
            <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-500 dark:text-[#78849a]">
              <LockKeyhole size={16} />
            </span>
            <input
              type={showPassword ? 'text' : 'password'}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Enter new password"
              required
              autoFocus
              className="h-14 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-12 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-500 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none"
            />
            <button
              type="button"
              onClick={() => setShowPassword((value) => !value)}
              className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 transition-colors hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200"
              aria-label={showPassword ? 'Hide password' : 'Show password'}
            >
              {showPassword ? <Eye size={18} /> : <EyeOff size={18} />}
            </button>
          </div>
        </label>

        <label className="block">
          <span className="mb-2 block text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-[#8a94a7]">
            Confirm password
          </span>
          <div className="relative">
            <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-500 dark:text-[#78849a]">
              <LockKeyhole size={16} />
            </span>
            <input
              type={showPassword ? 'text' : 'password'}
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder="Re-enter new password"
              required
              className="h-14 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-4 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-500 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none"
            />
          </div>
        </label>

        <p className="text-xs leading-5 text-slate-500 dark:text-[#7b8496] max-w-[57ch]">
          Minimum 10 characters, including 1 uppercase letter, 1 lowercase letter, 1 number, and 1
          special character.
        </p>

        <ErrorMessage error={error} />

        <button
          type="submit"
          disabled={loading}
          className="group inline-flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-5 text-sm font-semibold text-white transition-all duration-200 hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {loading ? (
            <>
              <Loader2 size={16} className="animate-spin" />
              Resetting
            </>
          ) : (
            <>
              Reset password
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
          className="text-sm font-medium text-slate-500 transition-colors hover:text-slate-950 dark:text-[#8a94a7] dark:hover:text-white"
        >
          Back to sign in
        </Link>
      </div>
    </div>,
  )
}
