import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface MerchantStore {
  merchantId: string
  setMerchantId: (id: string) => void
}

export const useMerchantStore = create<MerchantStore>()(
  persist(
    (set) => ({
      merchantId: '',
      setMerchantId: (id) => set({ merchantId: id }),
    }),
    { name: 'merchant-store' }
  )
)
