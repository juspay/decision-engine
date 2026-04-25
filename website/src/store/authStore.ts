import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { tokenRef } from '../lib/tokenRef'

export interface MerchantInfo {
  merchant_id: string
  merchant_name: string
  role: string
}

export interface AuthUser {
  userId: string
  email: string
  merchantId: string
  role: string
}

interface AuthStore {
  token: string | null
  user: AuthUser | null
  merchants: MerchantInfo[]
  hasHydrated: boolean
  setAuth: (token: string, user: AuthUser, merchants?: MerchantInfo[]) => void
  updateMerchant: (token: string, merchantId: string, merchants: MerchantInfo[]) => void
  clearAuth: () => void
  setHasHydrated: (hasHydrated: boolean) => void
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      token: null,
      user: null,
      merchants: [],
      hasHydrated: false,
      setAuth: (token, user, merchants = []) => {
        tokenRef.set(token)
        set({ token, user, merchants })
      },
      updateMerchant: (token, merchantId, merchants) => {
        tokenRef.set(token)
        set((state) => ({
          token,
          merchants,
          user: state.user ? { ...state.user, merchantId } : null,
        }))
      },
      clearAuth: () => {
        tokenRef.set(null)
        set({ token: null, user: null, merchants: [] })
      },
      setHasHydrated: (hasHydrated) => set({ hasHydrated }),
    }),
    {
      name: 'auth-store',
      onRehydrateStorage: () => (state) => {
        if (state?.token) {
          tokenRef.set(state.token)
        }
        state?.setHasHydrated(true)
      },
    }
  )
)
