import { useState } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router-dom'
import {
  ArrowRight,
  Eye,
  EyeOff,
  Loader2,
  LockKeyhole,
  XCircle,
} from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../lib/api'
import { ErrorMessage } from '../components/ui/ErrorMessage'
import { AUTH_PRIMARY_BUTTON, AuthCardLayout, AuthStatus } from '../components/auth/AuthCardLayout'

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

  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

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

  if (!token) {
    return (
      <AuthCardLayout size="status">
        <AuthStatus
          icon={XCircle}
          tone="error"
          title="Invalid reset link"
          action={<Link to="/forgot-password" className={AUTH_PRIMARY_BUTTON}>Request a new link</Link>}
        >
          This link is missing a token. Request a new password reset link to continue.
        </AuthStatus>
      </AuthCardLayout>
    )
  }

  return (
    <AuthCardLayout>
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
              className="h-14 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-12 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-500 focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none"
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
              className="h-14 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-4 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-500 focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none"
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
    </AuthCardLayout>
  )
}
