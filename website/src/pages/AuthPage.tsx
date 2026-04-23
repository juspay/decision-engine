import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Activity,
  ArrowRight,
  BadgeCheck,
  Eye,
  EyeOff,
  Loader2,
  LockKeyhole,
  Mail,
  ShieldCheck,
} from 'lucide-react'
import { useAuthStore } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'
import { apiFetch } from '../lib/api'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface AuthResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
}

type Tab = 'login' | 'signup'

const authHighlights = [
  {
    title: 'Live routing control',
    description: 'Operate auth-rate, rule-based, and volume strategies from one workspace.',
    icon: Activity,
  },
  {
    title: 'Merchant-scoped analytics',
    description: 'Track decisions, payment audit trails, and gateway score movement with session-based access.',
    icon: BadgeCheck,
  },
  {
    title: 'Protected operator access',
    description: 'JWT-backed sessions keep dashboard and analytics actions aligned to your merchant account.',
    icon: ShieldCheck,
  },
]

export function AuthPage() {
  const navigate = useNavigate()
  const { setAuth } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const assetBaseUrl = import.meta.env.BASE_URL

  const [tab, setTab] = useState<Tab>('login')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [merchantId, setMerchantIdInput] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function switchTab(nextTab: Tab) {
    setTab(nextTab)
    setError(null)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const path = tab === 'login' ? '/auth/login' : '/auth/signup'
      const body =
        tab === 'login'
          ? { email, password }
          : { email, password, merchant_id: merchantId }

      const res = await apiFetch<AuthResponse>(path, {
        method: 'POST',
        body: JSON.stringify(body),
      })

      setAuth(res.token, {
        userId: res.user_id,
        email: res.email,
        merchantId: res.merchant_id,
        role: res.role,
      })
      setMerchantId(res.merchant_id)
      navigate('/', { replace: true })
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Something went wrong'
      const match = msg.match(/API error \d+: (.+)/)
      if (match) {
        try {
          const parsed = JSON.parse(match[1])
          setError(parsed.message ?? msg)
        } catch {
          setError(match[1])
        }
      } else {
        setError(msg)
      }
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="relative min-h-screen overflow-hidden bg-[#050913] text-white">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,_rgba(59,130,246,0.24),_transparent_26%),radial-gradient(circle_at_75%_18%,_rgba(14,165,233,0.14),_transparent_24%),radial-gradient(circle_at_bottom_right,_rgba(99,102,241,0.16),_transparent_34%),linear-gradient(180deg,_#07101d_0%,_#050913_52%,_#04070d_100%)]" />
      <div className="absolute inset-0 opacity-30 [background-image:linear-gradient(rgba(148,163,184,0.08)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.08)_1px,transparent_1px)] [background-size:64px_64px]" />
      <div className="absolute left-10 top-10 h-40 w-40 rounded-full bg-brand-500/20 blur-3xl" />
      <div className="absolute bottom-0 right-0 h-72 w-72 rounded-full bg-sky-400/10 blur-3xl" />

      <div className="relative z-10 mx-auto flex min-h-screen w-full max-w-[1380px] flex-col px-4 py-6 md:px-8 lg:px-10">
        <header className="flex items-center justify-between py-4">
          <div className="flex items-center gap-4">
            <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-white/10 bg-white/5 backdrop-blur-sm">
              <img
                src={`${assetBaseUrl}logo/decision-engine-light.svg`}
                alt="Decision Engine"
                className="h-8 w-auto"
              />
            </div>
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.28em] text-sky-200/75">
                Juspay Internal
              </p>
              <h1 className="text-lg font-semibold tracking-tight text-white">Decision Engine Console</h1>
            </div>
          </div>

          <div className="hidden items-center gap-2 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs text-slate-300 backdrop-blur-md md:flex">
            <span className="inline-flex h-2 w-2 rounded-full bg-emerald-400 shadow-[0_0_18px_rgba(74,222,128,0.7)]" />
            Dashboard, analytics, and payment audit
          </div>
        </header>

        <div className="flex flex-1 items-center py-6">
          <div className="grid w-full gap-8 lg:grid-cols-[1.05fr_0.95fr] lg:gap-12">
            <section className="flex flex-col justify-between rounded-[32px] border border-white/10 bg-white/[0.035] p-6 backdrop-blur-xl md:p-8 lg:min-h-[700px] lg:p-10">
              <div className="space-y-8">
                <div className="inline-flex items-center gap-2 rounded-full border border-sky-300/20 bg-sky-300/10 px-3 py-1 text-[11px] font-semibold uppercase tracking-[0.24em] text-sky-100">
                  Operator Access
                </div>

                <div className="max-w-xl space-y-5">
                  <h2 className="text-4xl font-semibold leading-[1.02] tracking-[-0.04em] text-white md:text-5xl lg:text-[64px]">
                    Route, inspect, and iterate from one control surface.
                  </h2>
                  <p className="max-w-lg text-base leading-7 text-slate-300 md:text-lg">
                    Sign in to the same internal workspace used for gateway routing setup, merchant-scoped analytics,
                    and payment audit review.
                  </p>
                </div>

                <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-1">
                  {authHighlights.map(({ title, description, icon: Icon }) => (
                    <div
                      key={title}
                      className="rounded-[24px] border border-white/8 bg-black/20 p-5 transition-transform duration-300 hover:-translate-y-0.5"
                    >
                      <div className="mb-4 flex h-11 w-11 items-center justify-center rounded-2xl border border-sky-300/15 bg-sky-300/10 text-sky-100">
                        <Icon size={18} />
                      </div>
                      <h3 className="text-sm font-semibold text-white">{title}</h3>
                      <p className="mt-2 text-sm leading-6 text-slate-400">{description}</p>
                    </div>
                  ))}
                </div>
              </div>

              <div className="mt-10 rounded-[28px] border border-white/10 bg-black/25 p-5">
                <div className="mb-5 flex items-center justify-between">
                  <div>
                    <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-500">
                      Internal Environment
                    </p>
                    <p className="mt-2 text-sm text-slate-300">Access is scoped to the merchant tied to your session.</p>
                  </div>
                  <div className="rounded-full border border-emerald-400/20 bg-emerald-400/10 px-3 py-1 text-xs font-medium text-emerald-200">
                    Auth-derived scope
                  </div>
                </div>

                <div className="grid gap-3 text-sm text-slate-300 md:grid-cols-3">
                  <div className="rounded-2xl border border-white/8 bg-white/[0.04] px-4 py-3">
                    <p className="text-[11px] uppercase tracking-[0.2em] text-slate-500">Gateway routing</p>
                    <p className="mt-2 font-medium text-white">SR, rules, debit, and volume split controls</p>
                  </div>
                  <div className="rounded-2xl border border-white/8 bg-white/[0.04] px-4 py-3">
                    <p className="text-[11px] uppercase tracking-[0.2em] text-slate-500">Analytics</p>
                    <p className="mt-2 font-medium text-white">Overview, scores, decisions, and filterable traces</p>
                  </div>
                  <div className="rounded-2xl border border-white/8 bg-white/[0.04] px-4 py-3">
                    <p className="text-[11px] uppercase tracking-[0.2em] text-slate-500">Payment audit</p>
                    <p className="mt-2 font-medium text-white">Timeline detail backed by ClickHouse event streams</p>
                  </div>
                </div>
              </div>
            </section>

            <section className="flex items-center justify-center">
              <div className="w-full max-w-[520px] rounded-[34px] border border-white/10 bg-[#f8fbff] p-4 shadow-[0_40px_140px_-48px_rgba(8,15,28,0.72)] md:p-5">
                <div className="overflow-hidden rounded-[28px] border border-slate-200 bg-white shadow-[0_20px_80px_-54px_rgba(15,23,42,0.35)]">
                  <div className="border-b border-slate-200 bg-[linear-gradient(180deg,#ffffff_0%,#f8fbff_100%)] px-6 py-6 md:px-8">
                    <div className="flex items-center gap-3">
                      <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-slate-950 text-white shadow-[0_18px_40px_-28px_rgba(15,23,42,0.55)]">
                        <img
                          src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
                          alt="Decision Engine"
                          className="h-7 w-auto"
                        />
                      </div>
                      <div>
                        <p className="text-[11px] font-semibold uppercase tracking-[0.26em] text-slate-400">
                          Access Portal
                        </p>
                        <p className="mt-1 text-sm text-slate-500">
                          {tab === 'login'
                            ? 'Continue into the internal routing workspace.'
                            : 'Create an operator account for an existing merchant.'}
                        </p>
                      </div>
                    </div>

                    <div className="mt-6 inline-flex rounded-full border border-slate-200 bg-slate-50 p-1">
                      <AuthTabButton active={tab === 'login'} onClick={() => switchTab('login')}>
                        Sign in
                      </AuthTabButton>
                      <AuthTabButton active={tab === 'signup'} onClick={() => switchTab('signup')}>
                        Sign up
                      </AuthTabButton>
                    </div>
                  </div>

                  <div className="px-6 py-7 md:px-8 md:py-8">
                    <div className="mb-7">
                      <h3 className="text-[30px] font-semibold tracking-[-0.035em] text-slate-950">
                        {tab === 'login' ? 'Welcome back.' : 'Create operator access.'}
                      </h3>
                      <p className="mt-2 text-sm leading-6 text-slate-500">
                        {tab === 'login'
                          ? 'Use the email and password tied to your Decision Engine merchant.'
                          : 'Sign-up is for an existing merchant account. You will land directly in the dashboard after provisioning.'}
                      </p>
                    </div>

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

                      <Field label="Password" footer={tab === 'login' ? 'Password reset is managed by your internal operator admin.' : undefined}>
                        <div className="relative">
                          <FieldInput
                            type={showPassword ? 'text' : 'password'}
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            placeholder="Enter your password"
                            required
                            icon={<LockKeyhole size={16} />}
                            className="pr-12"
                          />
                          <button
                            type="button"
                            onClick={() => setShowPassword((value) => !value)}
                            className="absolute right-4 top-1/2 -translate-y-1/2 text-slate-400 transition-colors hover:text-slate-700"
                            aria-label={showPassword ? 'Hide password' : 'Show password'}
                          >
                            {showPassword ? <Eye size={18} /> : <EyeOff size={18} />}
                          </button>
                        </div>
                      </Field>

                      {tab === 'signup' && (
                        <Field
                          label="Merchant ID"
                          footer="The merchant account must already exist before operator signup."
                        >
                          <FieldInput
                            type="text"
                            value={merchantId}
                            onChange={(e) => setMerchantIdInput(e.target.value)}
                            placeholder="merchant_123"
                            required
                          />
                        </Field>
                      )}

                      <ErrorMessage error={error} />

                      <button
                        type="submit"
                        disabled={loading}
                        className="group inline-flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-slate-950 px-5 text-sm font-semibold text-white transition-all duration-200 hover:bg-brand-600 disabled:cursor-not-allowed disabled:bg-slate-400"
                      >
                        {loading ? (
                          <>
                            <Loader2 size={16} className="animate-spin" />
                            Authenticating
                          </>
                        ) : (
                          <>
                            {tab === 'login' ? 'Enter workspace' : 'Create account'}
                            <ArrowRight
                              size={16}
                              className="transition-transform duration-200 group-hover:translate-x-0.5"
                            />
                          </>
                        )}
                      </button>
                    </form>

                    <div className="mt-7 flex flex-col gap-3 border-t border-slate-200 pt-5 text-xs text-slate-500 sm:flex-row sm:items-center sm:justify-between">
                      <p>
                        By continuing you agree to internal access policy and audit logging for operator actions.
                      </p>
                      <p className="font-medium text-slate-700">Juspay Decision Engine</p>
                    </div>
                  </div>
                </div>
              </div>
            </section>
          </div>
        </div>
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
          ? 'bg-white text-slate-950 shadow-[0_8px_24px_-18px_rgba(15,23,42,0.45)]'
          : 'text-slate-500 hover:text-slate-900'
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
      <span className="mb-2 block text-[13px] font-semibold uppercase tracking-[0.18em] text-slate-500">
        {label}
      </span>
      {children}
      {footer ? <p className="mt-2 text-xs leading-5 text-slate-500">{footer}</p> : null}
    </label>
  )
}

function FieldInput({
  icon,
  className = '',
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & {
  icon?: React.ReactNode
}) {
  return (
    <div className="relative">
      {icon ? (
        <span className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-400">
          {icon}
        </span>
      ) : null}
      <input
        {...props}
        className={`h-14 w-full rounded-2xl border border-slate-200 bg-slate-50 px-4 text-sm text-slate-900 placeholder:text-slate-400 focus:border-brand-500 focus:bg-white focus:ring-4 focus:ring-brand-500/10 ${icon ? 'pl-12' : ''} ${className}`}
      />
    </div>
  )
}
