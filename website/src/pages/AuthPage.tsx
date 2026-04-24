import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore, MerchantInfo } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'
import { apiFetch } from '../lib/api'
import { Loader2, Eye, EyeOff, Lock } from 'lucide-react'

interface AuthResponse {
  token: string
  user_id: string
  email: string
  merchant_id: string
  role: string
  merchants: MerchantInfo[]
}

type Tab = 'login' | 'signup'

export function AuthPage() {
  const navigate = useNavigate()
  const { setAuth } = useAuthStore()
  const { setMerchantId } = useMerchantStore()

  const [tab, setTab] = useState<Tab>('login')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  function switchTab(t: Tab) {
    setTab(t)
    setError(null)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const path = tab === 'login' ? '/auth/login' : '/auth/signup'
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

      if (!res.merchant_id || res.merchants.length === 0) {
        navigate('/onboarding', { replace: true })
      } else {
        navigate('/', { replace: true })
      }
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
    <div
      className="min-h-screen flex flex-col items-center justify-center p-4"
      style={{
        backgroundColor: '#212e46',
        backgroundImage: 'url(/dashboard/images/auth_bg.svg)',
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundAttachment: 'fixed',
        fontFamily: "'Inter', 'InterDisplay', sans-serif",
        fontOpticalSizing: 'auto' as React.CSSProperties['fontOpticalSizing'],
      }}
    >
      {/* Card */}
      <div
        className="w-full bg-white rounded-lg overflow-hidden"
        style={{ maxWidth: '442px', border: '1px solid rgb(229, 231, 235)' }}
      >
        {/* Card header — logo row */}
        <div
          className="flex items-center gap-3 px-7 py-6"
          style={{ borderBottom: '1px solid rgb(229, 231, 235)' }}
        >
          {/* Icon — chat-bubble style matching Hyperswitch */}
          <div className="shrink-0" style={{ width: 32, height: 32 }}>
            <svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect width="32" height="32" rx="7" fill="#006df9"/>
              <rect x="6" y="9" width="20" height="14" rx="3" fill="white"/>
              <circle cx="11" cy="16" r="1.8" fill="#006df9"/>
              <circle cx="16" cy="16" r="1.8" fill="#006df9"/>
              <circle cx="21" cy="16" r="1.8" fill="#006df9"/>
            </svg>
          </div>
          <span
            style={{
              fontFamily: "'Inter', sans-serif",
              fontSize: 16,
              fontWeight: 500,
              color: '#1a1a1a',
              letterSpacing: '-0.01em',
            }}
          >
            Decision Engine
          </span>
        </div>

        {/* Card body */}
        <div className="px-7 py-7">
          {/* Heading */}
          <h1
            style={{
              fontFamily: "'Inter', sans-serif",
              fontSize: 24,
              fontWeight: 600,
              lineHeight: '32px',
              color: '#000000',
              marginBottom: 4,
              fontOpticalSizing: 'auto',
            } as React.CSSProperties}
          >
            {tab === 'login' ? 'Hey there, Welcome back!' : 'Create your account'}
          </h1>

          {/* Subtitle */}
          <p
            style={{
              fontFamily: "'Inter', sans-serif",
              fontSize: 14,
              color: 'rgb(55, 65, 81)',
              marginBottom: 28,
            }}
          >
            {tab === 'login' ? (
              <>
                New to Decision Engine?{' '}
                <button
                  type="button"
                  onClick={() => switchTab('signup')}
                  style={{
                    color: 'rgb(0, 109, 249)',
                    fontWeight: 600,
                    fontSize: 14,
                    background: 'none',
                    border: 'none',
                    padding: 0,
                    cursor: 'pointer',
                  }}
                >
                  Sign up
                </button>
              </>
            ) : (
              <>
                Already have an account?{' '}
                <button
                  type="button"
                  onClick={() => switchTab('login')}
                  style={{
                    color: 'rgb(0, 109, 249)',
                    fontWeight: 600,
                    fontSize: 14,
                    background: 'none',
                    border: 'none',
                    padding: 0,
                    cursor: 'pointer',
                  }}
                >
                  Sign in
                </button>
              </>
            )}
          </p>

          <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            {/* Email */}
            <div>
              <label
                style={{
                  display: 'block',
                  fontFamily: "'Inter', sans-serif",
                  fontSize: 13,
                  fontWeight: 500,
                  color: '#000000',
                  marginBottom: 6,
                  lineHeight: '19.5px',
                }}
              >
                Email
              </label>
              <input
                type="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="Enter your Email"
                style={{
                  width: '100%',
                  height: 40,
                  backgroundColor: 'rgb(255, 255, 255)',
                  border: '1px solid rgba(204, 210, 226, 0.75)',
                  borderRadius: 4,
                  padding: '0 8px',
                  fontSize: 14,
                  color: 'rgba(51, 51, 51, 0.75)',
                  outline: 'none',
                  boxSizing: 'border-box',
                  fontFamily: "'Inter', sans-serif",
                }}
                onFocus={(e) => {
                  e.target.style.border = '1px solid rgb(0, 109, 249)'
                  e.target.style.boxShadow = '0 0 0 2px rgba(0, 109, 249, 0.1)'
                }}
                onBlur={(e) => {
                  e.target.style.border = '1px solid rgba(204, 210, 226, 0.75)'
                  e.target.style.boxShadow = 'none'
                }}
              />
            </div>

            {/* Password */}
            <div>
              <label
                style={{
                  display: 'block',
                  fontFamily: "'Inter', sans-serif",
                  fontSize: 13,
                  fontWeight: 500,
                  color: '#000000',
                  marginBottom: 6,
                  lineHeight: '19.5px',
                }}
              >
                Password
              </label>
              <div style={{ position: 'relative' }}>
                <Lock
                  size={14}
                  style={{
                    position: 'absolute',
                    left: 10,
                    top: '50%',
                    transform: 'translateY(-50%)',
                    color: 'rgba(51,51,51,0.4)',
                    pointerEvents: 'none',
                  }}
                />
                <input
                  type={showPassword ? 'text' : 'password'}
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="Enter your Password"
                  style={{
                    width: '100%',
                    height: 40,
                    backgroundColor: 'rgb(255, 255, 255)',
                    border: '1px solid rgba(204, 210, 226, 0.75)',
                    borderRadius: 4,
                    padding: '0 36px 0 30px',
                    fontSize: 14,
                    color: 'rgba(51, 51, 51, 0.75)',
                    outline: 'none',
                    boxSizing: 'border-box',
                    fontFamily: "'Inter', sans-serif",
                  }}
                  onFocus={(e) => {
                    e.target.style.border = '1px solid rgb(0, 109, 249)'
                    e.target.style.boxShadow = '0 0 0 2px rgba(0, 109, 249, 0.1)'
                  }}
                  onBlur={(e) => {
                    e.target.style.border = '1px solid rgba(204, 210, 226, 0.75)'
                    e.target.style.boxShadow = 'none'
                  }}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((v) => !v)}
                  style={{
                    position: 'absolute',
                    right: 10,
                    top: '50%',
                    transform: 'translateY(-50%)',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    color: 'rgba(51,51,51,0.5)',
                    display: 'flex',
                    alignItems: 'center',
                    padding: 0,
                  }}
                >
                  {showPassword ? <Eye size={16} /> : <EyeOff size={16} />}
                </button>
              </div>

              {tab === 'login' && (
                <div style={{ marginTop: 6 }}>
                  <button
                    type="button"
                    style={{
                      color: 'rgb(0, 109, 249)',
                      fontSize: 12,
                      fontWeight: 600,
                      background: 'none',
                      border: 'none',
                      padding: 0,
                      cursor: 'pointer',
                      fontFamily: "'Inter', sans-serif",
                    }}
                  >
                    Forgot Password?
                  </button>
                </div>
              )}
            </div>


            {/* Error */}
            {error && (
              <div
                style={{
                  borderRadius: 4,
                  backgroundColor: '#fef2f2',
                  border: '1px solid #fecaca',
                  padding: '10px 12px',
                  fontSize: 13,
                  color: '#dc2626',
                  fontFamily: "'Inter', sans-serif",
                }}
              >
                {error}
              </div>
            )}

            {/* Continue button */}
            <button
              type="submit"
              disabled={loading}
              style={{
                width: '100%',
                height: 36,
                backgroundColor: loading ? 'rgba(18,114,249,0.7)' : 'rgb(18, 114, 249)',
                color: 'white',
                border: 'none',
                borderRadius: 10,
                fontSize: 16,
                fontWeight: 400,
                fontFamily: "'Inter', sans-serif",
                cursor: loading ? 'not-allowed' : 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: 6,
                transition: 'background-color 0.15s',
                marginTop: 4,
              }}
              onMouseEnter={(e) => {
                if (!loading) (e.target as HTMLButtonElement).style.backgroundColor = 'rgb(0, 94, 220)'
              }}
              onMouseLeave={(e) => {
                if (!loading) (e.target as HTMLButtonElement).style.backgroundColor = 'rgb(18, 114, 249)'
              }}
            >
              {loading && <Loader2 size={15} className="animate-spin" />}
              Continue
            </button>
          </form>
        </div>
      </div>

      {/* Footer */}
      <p
        style={{
          marginTop: 24,
          fontSize: 14,
          color: 'rgb(254, 254, 254)',
          textAlign: 'center',
          fontFamily: "'Inter', sans-serif",
        }}
      >
        By continuing, you agree to our{' '}
        <span style={{ color: 'rgb(209, 213, 219)', textDecoration: 'underline', cursor: 'pointer' }}>
          Terms of Service
        </span>{' '}
        &{' '}
        <span style={{ color: 'rgb(209, 213, 219)', textDecoration: 'underline', cursor: 'pointer' }}>
          Privacy Policy
        </span>
      </p>
      <p
        style={{
          marginTop: 16,
          fontSize: 14,
          color: 'rgb(254, 254, 254)',
          textAlign: 'center',
          fontFamily: "'Inter', sans-serif",
          display: 'flex',
          alignItems: 'center',
          gap: 6,
        }}
      >
        An open-source initiative by{' '}
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 5 }}>
          <svg width="18" height="18" viewBox="0 0 18 18" fill="none">
            <circle cx="9" cy="9" r="9" fill="#006df9" />
            <circle cx="9" cy="9" r="3.5" fill="white" />
          </svg>
          <span style={{ fontWeight: 700, letterSpacing: '0.08em', fontSize: 13, color: 'rgb(254,254,254)' }}>JUSPAY</span>
        </span>
      </p>
    </div>
  )
}
