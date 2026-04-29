import { useEffect, useRef, useState } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import {
  ArrowRight,
  Building2,
  Eye,
  EyeOff,
  Loader2,
  LockKeyhole,
  Mail,
} from 'lucide-react'
import { useAuthStore, MerchantInfo } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'
import { apiFetch } from '../lib/api'
import { SurfaceLabel } from '../components/ui/Card'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface AuthResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
  merchants: MerchantInfo[]
}

interface CreateMerchantResponse {
  token: string
  merchant_id: string
  merchant_name: string
  merchants: MerchantInfo[]
}

type Tab = 'login' | 'signup'

interface AuthLocationState {
  email?: string
  focusPassword?: boolean
  notice?: string
}

function getTabFromPath(pathname: string): Tab {
  return pathname.endsWith('/signup') ? 'signup' : 'login'
}

function getPasswordPolicyError(password: string): string | null {
  if (password.length < 10) {
    return 'Use at least 10 characters.'
  }

  if (!/[A-Z]/.test(password)) {
    return 'Add at least one uppercase letter.'
  }

  if (!/[a-z]/.test(password)) {
    return 'Add at least one lowercase letter.'
  }

  if (!/[0-9]/.test(password)) {
    return 'Add at least one number.'
  }

  if (!/[^A-Za-z0-9]/.test(password)) {
    return 'Add at least one special character.'
  }

  return null
}

function getApiErrorMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : 'Something went wrong'
  const match = msg.match(/API error \d+: (.+)/)

  if (!match) return msg

  try {
    const parsed = JSON.parse(match[1])
    return parsed.message ?? msg
  } catch {
    return match[1]
  }
}

function isDuplicateEmailError(message: string): boolean {
  return /email.*(already registered|already exists)|user.*already exists/i.test(message)
}

export function AuthPage() {
  const navigate = useNavigate()
  const location = useLocation()
  const locationState = location.state as AuthLocationState | null
  const { token, hasHydrated, setAuth, updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const assetBaseUrl = import.meta.env.BASE_URL
  const passwordInputRef = useRef<HTMLInputElement>(null)

  const [tab, setTab] = useState<Tab>(() => getTabFromPath(location.pathname))
  const [email, setEmail] = useState(locationState?.email ?? '')
  const [password, setPassword] = useState('')
  const [merchantName, setMerchantName] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(locationState?.notice ?? null)
  const [focusPasswordOnLogin, setFocusPasswordOnLogin] = useState(
    Boolean(locationState?.focusPassword),
  )

  useEffect(() => {
    if (!hasHydrated || !token || loading) return
    navigate('/', { replace: true })
  }, [hasHydrated, loading, navigate, token])

  useEffect(() => {
    setTab(getTabFromPath(location.pathname))

    if (locationState?.email) {
      setEmail(locationState.email)
    }

    if (locationState?.notice) {
      setNotice(locationState.notice)
    }

    if (locationState?.focusPassword) {
      setFocusPasswordOnLogin(true)
    }
  }, [location.pathname, locationState?.email, locationState?.focusPassword, locationState?.notice])

  useEffect(() => {
    if (tab !== 'login' || !focusPasswordOnLogin) return
    passwordInputRef.current?.focus()
    setFocusPasswordOnLogin(false)
  }, [focusPasswordOnLogin, tab])

  function switchTab(nextTab: Tab) {
    setTab(nextTab)
    setError(null)
    setNotice(null)
    navigate(nextTab === 'login' ? '/login' : '/signup')
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setNotice(null)

    if (tab === 'signup') {
      const passwordPolicyError = getPasswordPolicyError(password)
      if (passwordPolicyError) {
        setError(passwordPolicyError)
        return
      }
    }

    setLoading(true)

    try {
      const path = tab === 'login' ? '/auth/login' : '/auth/signup'
      const normalizedMerchantName = merchantName.trim()
      const res = await apiFetch<AuthResponse>(path, {
        method: 'POST',
        body: JSON.stringify({ email, password }),
      })

      setAuth(
        res.token,
        { userId: res.user_id, email: res.email, merchantId: res.merchant_id, role: res.role },
        res.merchants,
      )
      if (res.merchant_id) setMerchantId(res.merchant_id)

      if (tab === 'signup' && normalizedMerchantName && !res.merchant_id) {
        const merchantRes = await apiFetch<CreateMerchantResponse>('/onboarding/merchant', {
          method: 'POST',
          body: JSON.stringify({ merchant_name: normalizedMerchantName }),
        })

        updateMerchant(merchantRes.token, merchantRes.merchant_id, merchantRes.merchants)
        setMerchantId(merchantRes.merchant_id)
        navigate('/', { replace: true })
        return
      }

      if (!res.merchant_id || res.merchants.length === 0) {
        navigate('/onboarding', { replace: true })
      } else {
        navigate('/', { replace: true })
      }
    } catch (err) {
      const msg = getApiErrorMessage(err)

      if (tab === 'signup' && isDuplicateEmailError(msg)) {
        setTab('login')
        const duplicateEmailNotice = 'Account already exists. Sign in with this email.'
        setNotice(duplicateEmailNotice)
        setFocusPasswordOnLogin(true)
        navigate('/login', {
          replace: true,
          state: {
            email,
            focusPassword: true,
            notice: duplicateEmailNotice,
          } satisfies AuthLocationState,
        })
        return
      }

      setError(msg)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="dark relative min-h-screen overflow-hidden bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(180deg,_rgba(255,255,255,1),_rgba(255,255,255,1))] dark:bg-[linear-gradient(180deg,_rgba(3,5,7,1),_rgba(5,8,12,1))]" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_18%_18%,_rgba(59,130,246,0.06),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(14,165,233,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.03),_transparent_24%)] dark:bg-[radial-gradient(circle_at_18%_18%,_rgba(56,189,248,0.05),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(59,130,246,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.035),_transparent_24%)]" />
      <div className="pointer-events-none absolute inset-0 opacity-[0.05] dark:opacity-[0.08] [background-image:linear-gradient(rgba(148,163,184,0.08)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.08)_1px,transparent_1px)] [background-size:56px_56px]" />

      <div className="relative z-10 grid min-h-screen lg:grid-cols-[1.06fr_0.94fr]">
        <section className="flex min-h-[44vh] flex-col border-b border-slate-200 px-6 py-8 dark:border-white/6 sm:px-10 lg:min-h-screen lg:border-b-0 lg:border-r lg:border-[#1d1d23] lg:px-14 lg:py-12 xl:px-16">
          <div className="pt-2">
            <img
              src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
              alt="Juspay Decision Engine"
              className="h-11 w-auto sm:h-12"
            />
          </div>

          <div className="flex flex-1 items-center py-12 lg:py-0">
            <div className="max-w-[620px] space-y-9">
              <div className="space-y-6">
                <h2 className="max-w-[10ch] text-[clamp(4rem,7vw,6.2rem)] font-semibold leading-[0.92] tracking-[-0.065em] text-slate-950 dark:text-white">
                  Manage routing, analytics, and audits from one dashboard.
                </h2>
                <p className="max-w-[38rem] text-lg leading-9 text-slate-600 dark:text-[#9aa4b6] sm:text-[1.35rem]">
                  Sign in to manage gateway routing, analytics, and payment audits.
                </p>
              </div>

              <div className="flex flex-wrap gap-3">
                <PillStat>Gateway routing</PillStat>
                <PillStat>Merchant analytics</PillStat>
                <PillStat>Payment audit</PillStat>
              </div>
            </div>
          </div>
        </section>

        <section className="flex min-h-[56vh] items-center justify-center px-6 py-10 sm:px-10 lg:min-h-screen lg:px-14 lg:py-12 xl:px-16">
          <div className="w-full max-w-[520px]">
            <div className="text-center">
              <h3 className="text-[clamp(2.25rem,3.6vw,3.15rem)] font-semibold tracking-[-0.05em] text-slate-950 dark:text-white">
                {tab === 'login' ? 'Welcome back' : 'Create account'}
              </h3>
              <p className="mt-3 text-base text-slate-500 dark:text-[#8a94a7]">
                {tab === 'login'
                  ? 'Sign in to access your dashboard'
                  : 'Create access for your Decision Engine dashboard'}
              </p>
            </div>

            <div className="mt-10">
              <div className="inline-flex rounded-full border border-slate-200 bg-white p-1 dark:border-[#27272a] dark:bg-[#121214]">
                <AuthTabButton active={tab === 'login'} onClick={() => switchTab('login')}>
                  Sign in
                </AuthTabButton>
                <AuthTabButton active={tab === 'signup'} onClick={() => switchTab('signup')}>
                  Sign up
                </AuthTabButton>
              </div>

              <div className="mt-10 border-t border-slate-200 pt-10 dark:border-[#1d1d23]">
                <form onSubmit={handleSubmit} className="space-y-5">
                  <Field label="Email">
                    <FieldInput
                      type="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder="name@company.com"
                      required
                      icon={<Mail size={16} />}
                    />
                  </Field>

                  {tab === 'signup' ? (
                    <Field
                      label="Merchant name"
                      footer="This merchant will be available across routing, analytics, and audits."
                    >
                      <FieldInput
                        type="text"
                        value={merchantName}
                        onChange={(e) => setMerchantName(e.target.value)}
                        placeholder="e.g. Acme Corp"
                        required
                        icon={<Building2 size={16} />}
                      />
                    </Field>
                  ) : null}

                  <Field
                    label="Password"
                    footer={
                      tab === 'login'
                        ? 'Password reset is managed by your account admin.'
                        : 'Use at least 10 characters with uppercase, lowercase, number, and special character.'
                    }
                  >
                    <div className="relative">
                      <FieldInput
                        type={showPassword ? 'text' : 'password'}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        placeholder="Enter your password"
                        required
                        icon={<LockKeyhole size={16} />}
                        inputRef={passwordInputRef}
                        className="pr-12"
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword((value) => !value)}
                        className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-500 transition-colors hover:text-slate-200"
                        aria-label={showPassword ? 'Hide password' : 'Show password'}
                      >
                        {showPassword ? <Eye size={18} /> : <EyeOff size={18} />}
                      </button>
                    </div>
                  </Field>

                  {tab === 'signup' ? (
                    <p className="text-xs leading-5 text-slate-500 dark:text-[#7b8496]">
                      Password policy: minimum 10 characters, including 1 uppercase letter, 1
                      lowercase letter, 1 number, and 1 special character.
                    </p>
                  ) : null}

                  <ErrorMessage error={error} />
                  {notice ? (
                    <div className="rounded-lg border border-sky-500/20 bg-sky-500/8 px-4 py-3 text-sm text-sky-300">
                      {notice}
                    </div>
                  ) : null}

                  <button
                    type="submit"
                    disabled={loading}
                    className="group inline-flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-5 text-sm font-semibold text-white transition-all duration-200 hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {loading ? (
                      <>
                        <Loader2 size={16} className="animate-spin" />
                        Authenticating
                      </>
                    ) : (
                      <>
                        {tab === 'login' ? 'Enter dashboard' : 'Create account'}
                        <ArrowRight
                          size={16}
                          className="transition-transform duration-200 group-hover:translate-x-0.5"
                        />
                      </>
                    )}
                  </button>
                </form>

                <div className="mt-10 border-t border-slate-200 pt-6 text-center text-xs text-slate-500 dark:border-[#1d1d23] dark:text-[#667085]">
                  <p>By continuing you agree to access policy and audit logging for account activity.</p>
                  <p className="mt-4 text-slate-400 dark:text-[#525866]">Juspay Decision Engine</p>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}

function AuthTabButton({
  active,
  children,
  onClick,
}: {
  active: boolean
  children: React.ReactNode
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-4 py-2 text-sm font-semibold transition-all duration-200 ${
        active
          ? 'bg-slate-950 text-white shadow-[0_8px_24px_-18px_rgba(15,23,42,0.45)] dark:bg-white dark:text-slate-950'
          : 'text-slate-500 hover:text-slate-950 dark:text-[#8a94a7] dark:hover:text-white'
      }`}
    >
      {children}
    </button>
  )
}

function Field({
  label,
  children,
  footer,
}: {
  label: string
  children: React.ReactNode
  footer?: string
}) {
  return (
    <label className="block">
      <SurfaceLabel className="mb-2 block text-slate-500 dark:text-[#8a94a7]">{label}</SurfaceLabel>
      {children}
      {footer ? <p className="mt-2 text-xs leading-5 text-slate-500 dark:text-[#7b8496]">{footer}</p> : null}
    </label>
  )
}

function FieldInput({
  icon,
  className = '',
  inputRef,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & {
  icon?: React.ReactNode
  inputRef?: React.Ref<HTMLInputElement>
}) {
  return (
    <div className="relative">
      {icon ? (
        <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-400 dark:text-[#667085]">
          {icon}
        </span>
      ) : null}
      <input
        {...props}
        ref={inputRef}
        className={`h-14 w-full rounded-2xl border border-slate-200 bg-white px-4 text-sm text-slate-950 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.12)] outline-none transition placeholder:text-slate-400 focus:border-brand-500 focus:ring-2 focus:ring-brand-500/20 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white dark:shadow-none ${icon ? 'pl-12' : ''} ${className}`}
      />
    </div>
  )
}

function PillStat({ children }: { children: React.ReactNode }) {
  return (
    <div className="inline-flex items-center rounded-full border border-slate-200 bg-white px-4 py-2 text-sm text-slate-700 shadow-[0_12px_30px_-24px_rgba(15,23,42,0.1)] dark:border-[#27272a] dark:bg-[#121214] dark:text-[#c6d0e1] dark:shadow-none">
      {children}
    </div>
  )
}
