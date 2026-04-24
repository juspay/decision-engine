import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore, MerchantInfo } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'
import { apiFetch } from '../lib/api'
import { Loader2, Building2 } from 'lucide-react'

interface CreateMerchantResponse {
  token: string
  merchant_id: string
  merchant_name: string
  merchants: MerchantInfo[]
}

export function OnboardingPage() {
  const navigate = useNavigate()
  const { updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()

  const [merchantName, setMerchantName] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const res = await apiFetch<CreateMerchantResponse>('/onboarding/merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_name: merchantName }),
      })

      updateMerchant(res.token, res.merchant_id, res.merchants)
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
    <div
      className="min-h-screen flex flex-col items-center justify-center p-4"
      style={{
        backgroundColor: '#212e46',
        backgroundImage: 'url(/dashboard/images/auth_bg.svg)',
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundAttachment: 'fixed',
        fontFamily: "'Inter', sans-serif",
      }}
    >
      <div
        className="w-full bg-white rounded-lg overflow-hidden"
        style={{ maxWidth: '442px', border: '1px solid rgb(229, 231, 235)' }}
      >
        {/* Header */}
        <div
          className="flex items-center gap-3 px-7 py-6"
          style={{ borderBottom: '1px solid rgb(229, 231, 235)' }}
        >
          <div className="shrink-0" style={{ width: 32, height: 32 }}>
            <svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect width="32" height="32" rx="7" fill="#006df9"/>
              <rect x="6" y="9" width="20" height="14" rx="3" fill="white"/>
              <circle cx="11" cy="16" r="1.8" fill="#006df9"/>
              <circle cx="16" cy="16" r="1.8" fill="#006df9"/>
              <circle cx="21" cy="16" r="1.8" fill="#006df9"/>
            </svg>
          </div>
          <span style={{ fontFamily: "'Inter', sans-serif", fontSize: 16, fontWeight: 500, color: '#1a1a1a', letterSpacing: '-0.01em' }}>
            Decision Engine
          </span>
        </div>

        {/* Body */}
        <div className="px-7 py-7">
          <h1 style={{ fontFamily: "'Inter', sans-serif", fontSize: 24, fontWeight: 600, lineHeight: '32px', color: '#000000', marginBottom: 4, display: 'flex', alignItems: 'center', gap: 10 }}>
            <div className="flex items-center justify-center w-8 h-8 rounded-lg shrink-0" style={{ backgroundColor: '#eff6ff' }}>
              <Building2 size={16} style={{ color: '#006df9' }} />
            </div>
            Create your merchant
          </h1>
          <p style={{ fontFamily: "'Inter', sans-serif", fontSize: 14, color: 'rgb(55, 65, 81)', marginBottom: 28, marginTop: 4 }}>
            Set up your merchant account to start using Decision Engine.
          </p>

          <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div>
              <label style={{ display: 'block', fontFamily: "'Inter', sans-serif", fontSize: 13, fontWeight: 500, color: '#000000', marginBottom: 6 }}>
                Merchant Name
              </label>
              <input
                type="text"
                required
                autoFocus
                value={merchantName}
                onChange={(e) => setMerchantName(e.target.value)}
                placeholder="e.g. Acme Corp"
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

            {error && (
              <div style={{ borderRadius: 4, backgroundColor: '#fef2f2', border: '1px solid #fecaca', padding: '10px 12px', fontSize: 13, color: '#dc2626', fontFamily: "'Inter', sans-serif" }}>
                {error}
              </div>
            )}

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
              onMouseEnter={(e) => { if (!loading) (e.target as HTMLButtonElement).style.backgroundColor = 'rgb(0, 94, 220)' }}
              onMouseLeave={(e) => { if (!loading) (e.target as HTMLButtonElement).style.backgroundColor = 'rgb(18, 114, 249)' }}
            >
              {loading && <Loader2 size={15} className="animate-spin" />}
              Create Merchant
            </button>
          </form>
        </div>
      </div>
    </div>
  )
}
