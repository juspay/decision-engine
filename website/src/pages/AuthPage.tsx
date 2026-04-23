import { FormEvent, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { apiFetch } from '../lib/api'
import { useMerchantStore } from '../store/merchantStore'
import { useAuthStore } from '../store/authStore'

interface AuthResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
}

type AuthTab = 'login' | 'signup'

export function AuthPage() {
  const navigate = useNavigate()
  const setAuth = useAuthStore((state) => state.setAuth)
  const setMerchantId = useMerchantStore((state) => state.setMerchantId)

  const [tab, setTab] = useState<AuthTab>('login')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [merchantId, setMerchantIdInput] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const response = await apiFetch<AuthResponse>(tab === 'login' ? '/auth/login' : '/auth/signup', {
        method: 'POST',
        body: JSON.stringify(
          tab === 'login'
            ? { email, password }
            : { email, password, merchant_id: merchantId },
        ),
      })

      setAuth(response.token, {
        userId: response.user_id,
        email: response.email,
        merchantId: response.merchant_id,
        role: response.role,
      })
      setMerchantId(response.merchant_id)
      navigate('/', { replace: true })
    } catch (requestError) {
      const message = requestError instanceof Error ? requestError.message : 'Authentication failed'
      const payload = message.match(/API error \d+: (.+)/)?.[1]

      if (payload) {
        try {
          const parsed = JSON.parse(payload)
          setError(parsed.message ?? message)
        } catch {
          setError(payload)
        }
      } else {
        setError(message)
      }
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-slate-950 text-white">
      <div className="mx-auto flex min-h-screen max-w-md items-center px-6 py-12">
        <div className="w-full rounded-3xl border border-white/10 bg-white/5 p-8 shadow-2xl backdrop-blur">
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-sky-300">
            Decision Engine
          </p>
          <h1 className="mt-4 text-3xl font-semibold tracking-tight">
            {tab === 'login' ? 'Sign in' : 'Create your account'}
          </h1>
          <p className="mt-2 text-sm text-slate-300">
            {tab === 'login'
              ? 'Use your dashboard account to access merchant-scoped analytics and routing tools.'
              : 'Create an account bound to a merchant so the dashboard can derive analytics scope from your session.'}
          </p>

          <div className="mt-6 inline-flex rounded-2xl border border-white/10 bg-black/20 p-1">
            <button
              type="button"
              className={`rounded-xl px-4 py-2 text-sm font-medium transition ${
                tab === 'login' ? 'bg-white text-slate-950' : 'text-slate-300'
              }`}
              onClick={() => setTab('login')}
            >
              Login
            </button>
            <button
              type="button"
              className={`rounded-xl px-4 py-2 text-sm font-medium transition ${
                tab === 'signup' ? 'bg-white text-slate-950' : 'text-slate-300'
              }`}
              onClick={() => setTab('signup')}
            >
              Signup
            </button>
          </div>

          <form className="mt-6 space-y-4" onSubmit={handleSubmit}>
            <label className="block">
              <span className="mb-2 block text-sm text-slate-300">Email</span>
              <input
                className="w-full rounded-2xl border border-white/10 bg-black/20 px-4 py-3 text-white outline-none placeholder:text-slate-500 focus:border-sky-400"
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                required
                placeholder="you@example.com"
              />
            </label>

            <label className="block">
              <span className="mb-2 block text-sm text-slate-300">Password</span>
              <input
                className="w-full rounded-2xl border border-white/10 bg-black/20 px-4 py-3 text-white outline-none placeholder:text-slate-500 focus:border-sky-400"
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                required
                placeholder="Enter your password"
              />
            </label>

            {tab === 'signup' ? (
              <label className="block">
                <span className="mb-2 block text-sm text-slate-300">Merchant ID</span>
                <input
                  className="w-full rounded-2xl border border-white/10 bg-black/20 px-4 py-3 text-white outline-none placeholder:text-slate-500 focus:border-sky-400"
                  type="text"
                  value={merchantId}
                  onChange={(event) => setMerchantIdInput(event.target.value)}
                  required
                  placeholder="merchant_123"
                />
              </label>
            ) : null}

            {error ? (
              <div className="rounded-2xl border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-100">
                {error}
              </div>
            ) : null}

            <button
              type="submit"
              disabled={loading}
              className="w-full rounded-2xl bg-sky-400 px-4 py-3 text-sm font-semibold text-slate-950 transition hover:bg-sky-300 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {loading ? 'Please wait…' : tab === 'login' ? 'Sign in' : 'Create account'}
            </button>
          </form>
        </div>
      </div>
    </div>
  )
}
