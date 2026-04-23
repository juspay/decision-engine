import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import { tokenRef } from '../lib/tokenRef'

export interface AuthUser {
  userId: string
  email: string
  merchantId: string
  role: string
}

interface AuthStore {
  token: string | null
  user: AuthUser | null
  setAuth: (token: string, user: AuthUser) => void
  clearAuth: () => void
}

export const useAuthStore = create<AuthStore>()(
  persist(
    (set) => ({
      token: null,
      user: null,
      setAuth: (token, user) => {
        tokenRef.set(token)
        set({ token, user })
      },
      clearAuth: () => {
        tokenRef.set(null)
        set({ token: null, user: null })
      },
    }),
    {
      name: 'auth-store',
      onRehydrateStorage: () => (state) => {
        // Restore token ref from persisted storage on page load
        if (state?.token) {
          tokenRef.set(state.token)
        }
      },
    }
  )
)
