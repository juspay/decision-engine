import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface MerchantStore {
  merchantId: string
  setMerchantId: (id: string) => void
}

const DEBUG_STORE = true

export const useMerchantStore = create<MerchantStore>()(
  persist(
    (set) => ({
      merchantId: '',
      setMerchantId: (id) => {
        if (DEBUG_STORE) {
          console.log(`\n[STORE] Merchant ID changed: "${id}"`)
        }
        set({ merchantId: id })
      },
    }),
    { name: 'merchant-store' }
  )
)
