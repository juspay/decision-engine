import { useNavigate } from 'react-router-dom'
import { apiFetch } from '../lib/api'
import { MerchantInfo, useAuthStore } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'

/** A session as the backend issues it at the end of sign-in. */
export interface AuthResponse {
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

const PENDING_MERCHANT_NAME_KEY = 'pending_merchant_name'

/** Remember the merchant name a signup asked for, to create it once that account first signs in. */
export function rememberPendingMerchantName(email: string, merchantName: string) {
  localStorage.setItem(
    PENDING_MERCHANT_NAME_KEY,
    JSON.stringify({ email, merchant_name: merchantName }),
  )
}

function takePendingMerchantName(email: string): string {
  const raw = localStorage.getItem(PENDING_MERCHANT_NAME_KEY)
  localStorage.removeItem(PENDING_MERCHANT_NAME_KEY)
  if (!raw) return ''
  try {
    const pending = JSON.parse(raw)
    return pending?.email === email ? (pending.merchant_name ?? '') : ''
  } catch {
    return ''
  }
}

/**
 * Store a newly issued session and move into the dashboard: creating the merchant a signup named,
 * or sending a user with no merchant to onboarding. Shared by the password screen and the
 * two-factor steps, which both end with a session.
 */
export function useCompleteSignIn() {
  const navigate = useNavigate()
  const { setAuth, updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()

  return async function completeSignIn(session: AuthResponse, merchantName = '') {
    setAuth(
      session.token,
      {
        userId: session.user_id,
        email: session.email,
        merchantId: session.merchant_id,
        role: session.role,
        isRedirectSession: false,
      },
      session.merchants,
    )
    if (session.merchant_id) setMerchantId(session.merchant_id)

    const pendingMerchantName = takePendingMerchantName(session.email)
    const merchantNameToUse = merchantName.trim() || pendingMerchantName

    if (merchantNameToUse && !session.merchant_id) {
      const merchantRes = await apiFetch<CreateMerchantResponse>('/onboarding/merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_name: merchantNameToUse }),
      })
      updateMerchant(merchantRes.token, merchantRes.merchant_id, merchantRes.merchants)
      setMerchantId(merchantRes.merchant_id)
      navigate('/', { replace: true })
      return
    }

    if (!session.merchant_id || session.merchants.length === 0) {
      navigate('/onboarding', { replace: true })
    } else {
      navigate('/', { replace: true })
    }
  }
}
